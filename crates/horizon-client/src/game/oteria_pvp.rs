//! Oteria's PvP mods (`horizon_client::pvp`): the Crystal Optimizer (after Marlow's Crystal
//! Optimizer, MIT), the Anchor Optimizer, coloured hitboxes, the Pop Counter, what the totem and
//! pot counters remember, and Super Secret Settings (after fooxy's mod, MIT). Each does nothing
//! while it is off, and none sends the server anything vanilla wouldn't.
use super::{Dim, Game, Screen};
use horizon_client::{gizmos::Gizmos, items::ItemStack, particles::TrackingTarget, text::Text};
use std::{cell::RefCell, collections::HashMap};

/// How long a broken crystal stays gone without the server removing it: Marlow's 30 ticks.
const CRYSTAL_TICKS: u64 = 30;

#[derive(Default)]
struct State {
    /// The crystals this player broke, gone ahead of the server: the crystal, the tick it went
    /// and the dimension it was in.
    kept: Vec<(super::dragon::Crystal, u64, Dim)>,
    /// Each player that popped a totem: their name and how many they popped.
    pops: HashMap<u32, (String, u32)>,
    /// Totems this player popped since they last died.
    own_pops: u32,
    /// Healing splash potions carried last frame, and when the last one went (seconds since
    /// the game started).
    pots: u32,
    last_pot: Option<f64>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Healing splash potions: Instant Health I or II.
pub(super) fn is_healing_pot(s: &ItemStack) -> bool {
    &*s.item == "minecraft:splash_potion" && s.potion().is_some_and(|p| p.ends_with("healing"))
}

/// The sounds Super Secret Settings plays, as 1.8's button played a random one.
const SECRET_SOUNDS: [&str; 12] = [
    "minecraft:entity.villager.ambient",
    "minecraft:entity.ghast.scream",
    "minecraft:entity.cat.ambient",
    "minecraft:entity.pig.ambient",
    "minecraft:entity.creeper.primed",
    "minecraft:entity.enderman.scream",
    "minecraft:entity.goat.screaming.ambient",
    "minecraft:entity.chicken.egg",
    "minecraft:entity.llama.spit",
    "minecraft:entity.parrot.imitate.wither",
    "minecraft:block.note_block.didgeridoo",
    "minecraft:entity.player.burp",
];

impl Game<'_> {
    // --- Crystal Optimizer -------------------------------------------------------------------

    /// `MultiPlayerGameMode.attack` sent on entity `id`: an End crystal the blow breaks
    /// (`AttackDamage.breaksCrystal`: the attack damage, Strength and Weakness counted, above 0)
    /// leaves this client at once.
    pub(super) fn oteria_crystal_attacked(&mut self, id: i32) {
        if !self.settings.extras.pvp.crystal.enabled || id <= 0 {
            return;
        }
        let Some(i) = self.crystals.iter().position(|c| c.id == id as u32) else {
            return;
        };
        if self.attack_damage() <= 0.0 {
            return;
        }
        let crystal = self.crystals.remove(i);
        let (tick, dim) = (self.ticks, self.dim);
        STATE.with(|s| s.borrow_mut().kept.push((crystal, tick, dim)));
    }
    /// The server removed entity `id`: a crystal kept aside is gone for good.
    pub(super) fn oteria_crystal_removed(&mut self, id: u32) {
        STATE.with(|s| s.borrow_mut().kept.retain(|(c, _, _)| c.id != id));
    }
    /// The crystals kept aside that the server still hasn't removed after 30 ticks come back;
    /// those of another dimension are forgotten.
    fn oteria_crystal_tick(&mut self) {
        let (now, dim) = (self.ticks, self.dim);
        let back: Vec<super::dragon::Crystal> = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.kept.retain(|(_, _, d)| *d == dim);
            let (late, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut s.kept)
                .into_iter()
                .partition(|(_, at, _)| now.wrapping_sub(*at) >= CRYSTAL_TICKS);
            s.kept = keep;
            late.into_iter().map(|(c, _, _)| c).collect()
        });
        for c in back {
            if !self.crystals.iter().any(|k| k.id == c.id) {
                self.crystals.push(c);
            }
        }
    }
    /// With Keep render: the broken crystals still drawn until the server removes them.
    pub(super) fn oteria_kept_crystals(&self) -> Vec<super::dragon::Crystal> {
        let c = self.settings.extras.pvp.crystal;
        if !c.enabled || !c.keep_render {
            return Vec::new();
        }
        let dim = self.dim;
        STATE.with(|s| {
            s.borrow()
                .kept
                .iter()
                .filter(|(_, _, d)| *d == dim)
                .map(|(c, _, _)| c.clone())
                .collect()
        })
    }

    // --- Anchor Optimizer --------------------------------------------------------------------

    /// After `useItemOn` was sent for `pos` with `seq`: what `RespawnAnchorBlock` will do on the
    /// server, done here at once as a prediction (`BlockStatePredictionHandler`), which the
    /// server's answer settles. A charged anchor clicked by the main hand outside the Nether
    /// explodes (`useWithoutItem`): it goes. With Instant charge, glowstone charges it.
    pub(super) fn oteria_anchor_predict(
        &mut self,
        pos: [i32; 3],
        main_hand: bool,
        held: Option<&ItemStack>,
        seq: i32,
    ) {
        let opt = self.settings.extras.pvp.anchor;
        if !opt.enabled {
            return;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if !self.is_anchor(state) {
            return;
        }
        // A sneak with something in hand skips the block's use.
        let hands_full = self.stack_in(self.selected).is_some() || self.offhand.is_some();
        if self.remote_shift_down() && hands_full {
            return;
        }
        let charges = self.anchor_charges(state);
        let fuel = |s: Option<&ItemStack>| s.is_some_and(|s| &*s.item == "minecraft:glowstone");
        let chargeable = charges < 4;
        let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        if fuel(held) && chargeable {
            if opt.charge
                && let Some(next) = self.anchor_with_charges(state, charges + 1)
            {
                self.predict_blocks(&vec![(at, next)], seq);
            }
            return;
        }
        // The off hand's glowstone charges it instead (`PASS`); only the main hand sets it off.
        let offhand_fuel = fuel(self.offhand.as_ref());
        if !main_hand || (offhand_fuel && chargeable) || charges == 0 || self.anchor_works() {
            return;
        }
        self.predict_blocks(&vec![(at, horizon_world::registry::BlockStateId(0))], seq);
    }

    // --- Hitboxes ----------------------------------------------------------------------------

    /// Every entity's box in its kind's colour (F3+B's, which wins while it is on).
    pub(super) fn oteria_hitboxes(&self, alpha: f32, g: &mut Gizmos) {
        let h = self.settings.extras.pvp.hitboxes;
        if !h.enabled || self.title.is_some() || self.debug_entry("entity_hitboxes") {
            return;
        }
        let stroke = horizon_client::gizmos::STROKE_WIDTH;
        let (near_width, far_width) = (
            stroke * f32::from(h.width) / 10.0,
            stroke * f32::from(h.far_width) / 10.0,
        );
        // Combat Hitbox: the lines thin out (or thicken) past a distance from the eyes.
        let eye = self.eye_position();
        let far = f64::from(h.far_distance);
        let width_at = |at: [f64; 3]| match eye {
            Some(e)
                if (at[0] - e[0]).powi(2) + (at[1] - e[1]).powi(2) + (at[2] - e[2]).powi(2)
                    > far * far =>
            {
                far_width
            }
            _ => near_width,
        };
        let target = self.crosshair_target_mob();
        let a = f64::from(alpha);
        let lerp = |p: horizon_core::geometry::Vec3, n: horizon_core::geometry::Vec3| -> [f64; 3] {
            [p.x + a * (n.x - p.x), p.y + a * (n.y - p.y), p.z + a * (n.z - p.z)]
        };
        let lerpf = |p: f32, n: f32| p + alpha * (n - p);
        let draw = |g: &mut Gizmos,
                        colour: Option<horizon_client::pvp::Colour>,
                        at: [f64; 3],
                        (w, ht): (f32, f32),
                        eye: Option<f32>,
                        view: Option<(f32, f32)>| {
            let Some(colour) = colour else {
                return;
            };
            let half = f64::from(w) / 2.0;
            let min = [at[0] - half, at[1], at[2] - half];
            let max = [at[0] + half, at[1] + f64::from(ht), at[2] + half];
            let width = width_at(at);
            // The outline first, thicker, so the box's own lines sit on it.
            if let Some(outline) = h.outline {
                g.cuboid(min, max, outline.argb(), width * f32::from(h.outline_size) / 10.0);
            }
            g.cuboid(min, max, colour.argb(), width);
            if let Some(eye) = eye {
                let y = at[1] + f64::from(eye);
                if h.eyes {
                    let d = 0.01;
                    g.cuboid([min[0], y - d, min[2]], [max[0], y + d, max[2]], h.eye_colour.argb(), width);
                }
                if h.look
                    && let Some((pitch, yaw)) = view
                {
                    let (p, y2) = (pitch.to_radians(), -yaw.to_radians());
                    let v = [
                        f64::from(y2.sin() * p.cos()),
                        f64::from(-p.sin()),
                        f64::from(y2.cos() * p.cos()),
                    ];
                    let start = [at[0], y, at[2]];
                    let end = [start[0] + v[0] * 2.0, start[1] + v[1] * 2.0, start[2] + v[2] * 2.0];
                    g.line(start, end, h.look_colour.argb(), width);
                }
            }
        };
        for (i, m) in self.mobs.iter().enumerate() {
            if m.removed || m.invisible || m.has_effect("minecraft:invisibility") {
                continue;
            }
            let kind = if &*m.kind.id == "minecraft:player" {
                h.players
            } else if m.hostile() {
                h.hostile
            } else {
                h.passive
            };
            // Combat Hitbox: the targeted entity's colour, then the hurt one's, over the kind's
            // (a kind that is off stays off).
            let colour = kind.and(if target == Some(i) && h.target.is_some() {
                h.target
            } else if m.hurt_time > 0 && h.hurt.is_some() {
                h.hurt
            } else {
                kind
            });
            let view = (
                lerpf(m.previous_head_pitch, m.head_pitch),
                lerpf(m.previous_head_yaw, m.head_yaw),
            );
            draw(
                g,
                colour,
                lerp(m.previous, m.position),
                (m.width(), m.height()),
                Some(m.eye_height() as f32),
                Some(view),
            );
        }
        for item in &self.dropped {
            draw(g, h.items, lerp(item.previous, item.position), (0.25, 0.25), None, None);
        }
        for o in &self.orbs {
            draw(g, h.items, lerp(o.previous, o.position), (0.5, 0.5), None, None);
        }
        for c in &self.crystals {
            let p = c.position;
            draw(g, h.crystals, [p.x, p.y, p.z], (2.0, 2.0), None, None);
        }
        for p in &self.creatures.projectiles {
            use horizon_client::projectiles::ProjectileKind as K;
            let size = match p.kind {
                K::Arrow { .. } | K::Trident => 0.5,
                K::SmallFireball | K::ShulkerBullet { .. } | K::WitherSkull { .. } => 0.3125,
                K::WindCharge | K::BreezeWindCharge => 0.3125,
                K::LargeFireball { .. } | K::DragonFireball => 1.0,
                _ => 0.25,
            };
            draw(g, h.projectiles, lerp(p.previous, p.position), (size, size), None, None);
        }
        for t in &self.tridents {
            draw(g, h.projectiles, lerp(t.previous, t.position), (0.5, 0.5), None, None);
        }
        for r in self.rockets.iter().filter(|_| h.fireworks) {
            draw(g, h.projectiles, lerp(r.previous, r.position), (0.25, 0.25), None, None);
        }
        for a in self.armor_stands.iter().filter(|a| !a.invisible && !a.marker) {
            let size = if a.small { (0.25, 0.9875) } else { (0.5, 1.975) };
            draw(g, h.others, lerp(a.previous, a.position), size, None, None);
        }
        for b in &self.boats {
            let size = (super::boats::WIDTH as f32, super::boats::HEIGHT as f32);
            draw(g, h.others, lerp(b.previous, b.position), size, None, None);
        }
        for c in &self.carts {
            let size = (super::minecarts::WIDTH as f32, super::minecarts::HEIGHT as f32);
            draw(g, h.others, lerp(c.previous, c.position), size, None, None);
        }
        for t in &self.tnts {
            draw(g, h.others, lerp(t.previous, t.position), (0.98, 0.98), None, None);
        }
        for f in &self.falling {
            draw(g, h.others, lerp(f.previous, f.position), (0.98, 0.98), None, None);
        }
        if h.own
            && !self.camera_type.first_person()
            && let Some(p) = &self.player
        {
            let d = p.dimensions;
            draw(
                g,
                h.players,
                lerp(self.previous_position, p.position),
                (d.width, d.height),
                Some(d.eye_height),
                Some((self.pitch, self.yaw)),
            );
        }
    }

    // --- Totems and the Pop Counter ----------------------------------------------------------

    /// Entity event 35 on `target`: one more totem popped.
    pub(super) fn oteria_totem_popped(&mut self, target: TrackingTarget) {
        let id = match target {
            TrackingTarget::LocalPlayer => {
                STATE.with(|s| s.borrow_mut().own_pops += 1);
                return;
            }
            TrackingTarget::Entity(id) => id,
        };
        if !self.mobs.iter().any(|m| m.id == id as u32 && &*m.kind.id == "minecraft:player") {
            return;
        }
        let name = self.oteria_player_name(id);
        let count = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let entry = s.pops.entry(id as u32).or_insert_with(|| (name.clone(), 0));
            entry.0 = name.clone();
            entry.1 += 1;
            entry.1
        });
        if self.settings.extras.pvp.pops {
            let key = if count == 1 { "oteria.pops.popped_one" } else { "oteria.pops.popped" };
            let line = horizon_client::lang::tr(key, &[&name, &count]);
            self.system_message(Text::literal(format!("§d[Oteria] §f{line}")));
        }
    }
    fn oteria_player_name(&self, id: i32) -> String {
        self.remote
            .as_ref()
            .and_then(|r| r.level_player(id))
            .and_then(|(_, name)| name.map(str::to_owned))
            .unwrap_or_else(|| horizon_client::lang::get_or("oteria.pops.someone", "A player"))
    }
    /// Totems this player popped since they last died.
    pub(super) fn oteria_own_pops(&self) -> u32 {
        STATE.with(|s| s.borrow().own_pops)
    }
    /// Seconds since this player threw their last healing splash potion.
    pub(super) fn oteria_since_pot(&self) -> Option<f64> {
        let now = self.started.elapsed().as_secs_f64();
        STATE.with(|s| s.borrow().last_pot).map(|t| now - t)
    }

    // --- Super Secret Settings ---------------------------------------------------------------

    /// The Options button (or its key): another effect at random, and a random sound, as 1.8's.
    pub(super) fn oteria_secret_next(&mut self) {
        let n = horizon_client::pvp::SECRET_EFFECTS.len() as u64;
        let now = self.started.elapsed().as_nanos() as u64;
        let mut pick = now % (n - 1) + 1;
        if pick as u8 == self.settings.extras.pvp.secret.effect {
            pick = pick % (n - 1) + 1;
        }
        self.settings.extras.pvp.secret.effect = pick as u8;
        let sound = SECRET_SOUNDS[(now / 7 % SECRET_SOUNDS.len() as u64) as usize];
        let pitch = 0.8 + (now / 13 % 5) as f32 * 0.1;
        self.audio.play(sound, None, 1.0, pitch);
        self.settings.save(&Self::options_path());
    }
    /// The Super Secret Settings key, in play: another effect; with Shift, none. True when taken.
    pub(super) fn oteria_secret_key_press(&mut self, name: &str) -> bool {
        if self.screen != Screen::Playing
            || name == "key.keyboard.unknown"
            || name != self.settings.keys.get("key.oteria.secret")
        {
            return false;
        }
        let shift = [winit::keyboard::KeyCode::ShiftLeft, winit::keyboard::KeyCode::ShiftRight]
            .iter()
            .any(|k| self.physical_keys.contains(k));
        if shift {
            self.settings.extras.pvp.secret.effect = 0;
            self.settings.save(&Self::options_path());
        } else {
            self.oteria_secret_next();
        }
        true
    }

    // --- Each frame --------------------------------------------------------------------------

    /// Once per frame: the crystals' return, the deaths after pops, the pots thrown.
    pub(super) fn oteria_pvp_frame(&mut self) {
        self.oteria_crystal_tick();
        self.oteria_totem_frame();
        self.appleskin_frame();
        let now = self.started.elapsed().as_secs_f64();
        // Pots carried: one fewer is one thrown.
        let pots: u32 = self
            .inventory
            .iter()
            .chain(std::iter::once(&self.offhand))
            .flatten()
            .filter(|s| is_healing_pot(s))
            .map(|s| s.count)
            .sum();
        let own_dead = self.player.as_ref().is_none_or(|p| p.health.health() <= 0.0);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            // Dying empties the inventory: that is no throw.
            if pots < s.pots && !own_dead {
                s.last_pot = Some(now);
            }
            s.pots = pots;
            if own_dead {
                s.own_pops = 0;
            }
        });
        // The players who popped and died since.
        let dead: Vec<(u32, String, u32)> = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let mut dead = Vec::new();
            s.pops.retain(|id, (name, n)| {
                let alive = self.mobs.iter().find(|m| m.id == *id).map(|m| m.alive() && !m.removed);
                match alive {
                    Some(false) => {
                        dead.push((*id, name.clone(), *n));
                        false
                    }
                    _ => true,
                }
            });
            dead
        });
        if self.settings.extras.pvp.pops {
            for (_, name, n) in dead {
                let line = horizon_client::lang::tr("oteria.pops.died", &[&name, &n]);
                self.system_message(Text::literal(format!("§d[Oteria] §f{line}")));
            }
        }
    }
}
