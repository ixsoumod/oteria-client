//! Oteria's AppleSkin, after the Fabric mod AppleSkin 3.0.10 (public domain:
//! `THIRD_PARTY_NOTICES.md`), drawn as its `HUDOverlayHandler` draws:
//! - behind the hunger icons, the exhaustion bar (`drawExhaustionOverlay`);
//! - over them, the saturation as gold outlines, a quarter to a whole icon (`drawSaturationOverlay`);
//! - holding food the player can eat: the hunger it gives as flashing icons, the saturation it
//!   gives as flashing outlines, and the health natural regeneration would then give back as
//!   flashing hearts (`drawHungerOverlay`, `drawHealthOverlay`, `FoodHelper`).
//!
//! The icons sit exactly on vanilla's: their places are read from the health rows just laid out
//! (`hud_layout`), shakes included. Vanilla never sends the exhaustion, and its client never spends
//! what it counts itself, so the client's count is moved here and spent each time the server's
//! saturation or hunger drops: exactly when the server's own count passes 4.
use super::Game;
use horizon_client::hud::{HudBatch, Rect, rgba};
use horizon_client::hud_layout::GuiDraw;
use horizon_client::items::ConsumeEffect;
use horizon_client::oteria_art::{APPLESKIN, APPLESKIN_SIZE};

/// `FoodHelper.MAX_EXHAUSTION` and `REGEN_EXHAUSTION_INCREMENT`.
const MAX_EXHAUSTION: f32 = 4.0;
const REGEN_EXHAUSTION: f32 = 6.0;
/// `ModConfig.maxHudOverlayFlashAlpha`'s default.
const FLASH_MAX: f32 = 0.65;
/// The flash moves by 0.125 a tick (`onClientTick`), at 20 ticks a second.
const FLASH_SPEED: f32 = 2.5;

/// What AppleSkin keeps between frames.
#[derive(Debug)]
pub(super) struct AppleSkin {
    /// The exhaustion this client counted and the server has not spent yet.
    exhaustion: f32,
    /// The hunger and saturation last seen, to see the server spend.
    seen: Option<(i32, f32)>,
    /// The flash: unclamped, its direction, and when it last moved (s).
    unclamped: f32,
    rising: bool,
    last: Option<f64>,
}
impl Default for AppleSkin {
    fn default() -> Self {
        Self {
            exhaustion: 0.0,
            seen: None,
            unclamped: 0.0,
            rising: true,
            last: None,
        }
    }
}
impl AppleSkin {
    /// The flashing icons' opacity.
    fn flash(&self) -> f32 {
        self.unclamped.clamp(0.0, 1.0) * FLASH_MAX
    }
}

/// The food in hand (`HeldFoodCache`): main hand first, then the off hand.
struct Held {
    nutrition: i32,
    saturation: f32,
    /// Gives Hunger (`FoodHelper.isRotten`): drawn with the hunger icons.
    rotten: bool,
    /// Each Regeneration it gives: amplifier and duration (ticks).
    regeneration: Vec<(i32, i32)>,
}

/// `FoodHelper.getEstimatedHealthIncrement(int, float, float)`: the health natural regeneration
/// gives back from `food`, `saturation` and `exhaustion` before hunger falls under 18.
fn estimated_health(mut food: i32, mut saturation: f32, mut exhaustion: f32) -> f32 {
    if !exhaustion.is_finite() || !saturation.is_finite() {
        return 0.0;
    }
    let mut health = 0.0;
    while food >= 18 {
        while exhaustion > MAX_EXHAUSTION {
            exhaustion -= MAX_EXHAUSTION;
            if saturation > 0.0 {
                saturation = (saturation - 1.0).max(0.0);
            } else {
                food -= 1;
            }
        }
        if food >= 20 && saturation > f32::MIN_POSITIVE {
            // The fast regeneration of a full bar: a heal every 10 ticks, from saturation.
            let limited = saturation.min(REGEN_EXHAUSTION);
            let next_up = f32::from_bits(MAX_EXHAUSTION.to_bits() + 1);
            let times = ((next_up - exhaustion) / limited).ceil().max(1.0);
            health += limited / REGEN_EXHAUSTION * times;
            exhaustion += limited * times;
        } else if food >= 18 {
            health += 1.0;
            exhaustion += REGEN_EXHAUSTION;
        }
    }
    health
}

/// The `[x, y, w, h]` texels of AppleSkin's icons, as a region of the atlas.
fn sheet(r: Rect, [x, y, w, h]: [u32; 4]) -> Rect {
    let (tw, th) = APPLESKIN_SIZE;
    let du = (r.u1 - r.u0) / tw as f32;
    let dv = (r.v1 - r.v0) / th as f32;
    Rect {
        u0: r.u0 + du * x as f32,
        v0: r.v0 + dv * y as f32,
        u1: r.u0 + du * (x + w) as f32,
        v1: r.v0 + dv * (y + h) as f32,
        w,
        h,
    }
}
/// Where the laid-out rows put the sprites `keep` picks, in drawing order.
fn places(draws: &[GuiDraw], keep: impl Fn(&str) -> bool) -> Vec<(f32, f32)> {
    draws
        .iter()
        .filter_map(|d| match d {
            GuiDraw::Sprite { sprite, x, y, .. } if keep(sprite) => Some((*x as f32, *y as f32)),
            _ => None,
        })
        .collect()
}
fn white(alpha: f32) -> u32 {
    rgba(255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

impl Game<'_> {
    /// Once a frame: the exhaustion counted and spent, and the flash moved.
    pub(super) fn appleskin_frame(&mut self) {
        let s = self.settings.extras.pvp.saturation;
        if !s.enabled {
            return;
        }
        let now = self.started.elapsed().as_secs_f64();
        let held = s.held && self.held_food().is_some();
        let a = &mut self.pvp.appleskin;
        let dt = a.last.map_or(0.0, |t| (now - t) as f32).min(0.25);
        a.last = Some(now);
        if held {
            a.unclamped += if a.rising { 1.0 } else { -1.0 } * FLASH_SPEED * dt;
            if a.unclamped >= 1.5 {
                a.rising = false;
            } else if a.unclamped <= -0.5 {
                a.rising = true;
            }
        } else {
            (a.unclamped, a.rising) = (0.0, true);
        }
        let Some(p) = self.player.as_mut() else {
            a.seen = None;
            return;
        };
        // Only the server spends the exhaustion: the client's own count is moved here, where
        // it can't stall at its cap.
        a.exhaustion = (a.exhaustion + p.health.food.exhaustion).min(40.0);
        p.health.food.exhaustion = 0.0;
        let now = (p.health.food.food, p.health.food.saturation);
        if let Some((food, saturation)) = a.seen
            && (now.0 < food || now.0 == food && now.1 < saturation - 1e-3)
        {
            a.exhaustion = (a.exhaustion - MAX_EXHAUSTION).max(0.0);
        }
        a.seen = Some(now);
    }

    /// The food in hand the player could eat now (`HeldFoodCache.query`).
    fn held_food(&self) -> Option<Held> {
        let p = self.player.as_ref()?;
        let hands = [self.inventory.get(self.selected)?.as_ref(), self.offhand.as_ref()];
        hands.into_iter().flatten().find_map(|stack| {
            let info = self.items().get(&stack.item)?;
            let (nutrition, saturation) = info.food?;
            // Player.canEat: invulnerable, always edible, or hungry.
            if !(self.creative || info.can_always_eat || p.health.food.food < 20) {
                return None;
            }
            let mut rotten = false;
            let mut regeneration = Vec::new();
            let effects = info.consumable.iter().flat_map(|c| &c.effects);
            for effect in effects {
                let ConsumeEffect::ApplyEffects { effects, .. } = effect else {
                    continue;
                };
                for e in effects {
                    match e.id.trim_start_matches("minecraft:") {
                        "hunger" => rotten = true,
                        "regeneration" => regeneration.push((e.amplifier, e.duration)),
                        _ => {}
                    }
                }
            }
            Some(Held {
                nutrition: nutrition as i32,
                saturation,
                rotten,
                regeneration,
            })
        })
    }

    /// Behind the hunger icons (`onPreRenderFood`): the exhaustion bar, from the right.
    pub(super) fn draw_appleskin_under(&self, batch: &mut HudBatch, w: i32, h: i32) {
        let s = self.settings.extras.pvp.saturation;
        if !s.enabled || !s.exhaustion {
            return;
        }
        let Some(r) = self.gui.atlas.sprites.get(APPLESKIN).copied() else {
            return;
        };
        let full = (self.pvp.appleskin.exhaustion / MAX_EXHAUSTION).clamp(0.0, 1.0);
        let width = (full * 81.0) as u32;
        if width == 0 {
            return;
        }
        let (right, top) = (w / 2 + 91, h - 39);
        let part = sheet(r, [81 - width, 18, width, 9]);
        let x = (right - width as i32) as f32;
        batch.rect_uv(x, top as f32, width as f32, 9.0, part, white(0.75));
    }

    /// Over the health rows just painted (`onRenderFood`, `onRenderHealth`), for a player of
    /// `max` health.
    pub(super) fn draw_appleskin_over(&self, batch: &mut HudBatch, draws: &[GuiDraw], max: f32) {
        let s = self.settings.extras.pvp.saturation;
        let Some(p) = self.player.as_ref().filter(|_| s.enabled) else {
            return;
        };
        let atlas = &self.gui.atlas;
        let Some(icons) = atlas.sprites.get(APPLESKIN).copied() else {
            return;
        };
        let food_at = places(draws, |n| n == "hud/food_empty" || n == "hud/food_empty_hunger");
        let (food, saturation) = (p.health.food.food, p.health.food.saturation);
        // The gold outlines, from `start`: a quarter, a half, three quarters or a whole icon.
        let outlines = |batch: &mut HudBatch, gained: f32, alpha: f32| {
            if saturation + gained < 0.0 {
                return;
            }
            let total = (saturation + gained).clamp(0.0, 20.0);
            let start = if gained != 0.0 {
                (saturation / 2.0).max(0.0) as usize
            } else {
                0
            };
            let end = (total / 2.0).ceil() as usize;
            for (i, &(x, y)) in food_at.iter().enumerate().take(end).skip(start) {
                let left = total / 2.0 - i as f32;
                let u = match left {
                    l if l >= 1.0 => 27,
                    l if l > 0.5 => 18,
                    l if l > 0.25 => 9,
                    _ => 0,
                };
                batch.rect_uv(x, y, 9.0, 9.0, sheet(icons, [u, 0, 9, 9]), white(alpha));
            }
        };
        let sprite = |name: &str| atlas.sprites.get(&format!("gui/sprites/{name}")).copied();
        outlines(batch, 0.0, 1.0);
        let Some(held) = s.held.then(|| self.held_food()).flatten() else {
            return;
        };
        let flash = self.pvp.appleskin.flash();

        // The hunger the food gives, flashing (`drawHungerOverlay`).
        if held.nutrition > 0 {
            let total = (food + held.nutrition).clamp(0, 20);
            let (start, end) = ((food / 2).max(0) as usize, (total as f32 / 2.0).ceil() as usize);
            let kind = if held.rotten { "_hunger" } else { "" };
            for (i, &(x, y)) in food_at.iter().enumerate().take(end).skip(start) {
                if let Some(r) = sprite(&format!("hud/food_empty{kind}")) {
                    batch.rect_uv(x, y, 9.0, 9.0, r, white(flash * 0.25));
                }
                let half = i as i32 * 2 + 1 == total;
                let part = if half { "half" } else { "full" };
                if let Some(r) = sprite(&format!("hud/food_{part}{kind}")) {
                    batch.rect_uv(x, y, 9.0, 9.0, r, white(flash));
                }
            }
        }
        // The saturation it gives, flashing: no more than the hunger it will have.
        let new_food = (food + held.nutrition) as f32;
        let gained = if saturation + held.saturation > new_food {
            new_food - saturation
        } else {
            held.saturation
        };
        outlines(batch, gained, flash);

        // The health natural regeneration would give back, flashing (`drawHealthOverlay`).
        let effects = &self.potions.effects;
        let blocked = ["poison", "wither", "regeneration"]
            .iter()
            .any(|e| effects.get(e).is_some());
        if self.ui.difficulty == 0 || food >= 18 || blocked {
            return;
        }
        let health = p.health.health();
        let mut gain = 0.0;
        if health < max {
            let new_food = (food + held.nutrition).min(20);
            if new_food >= 18 {
                let sat = (saturation + held.saturation).min(new_food as f32);
                gain = estimated_health(new_food, sat, self.pvp.appleskin.exhaustion);
            }
            for &(amplifier, duration) in &held.regeneration {
                let every = (50 >> amplifier.clamp(0, 31)).max(1);
                gain += (duration / every) as f32;
            }
        }
        let modified = (health + gain).min(max);
        if modified <= health {
            return;
        }
        // The hearts' places, the containers drawn last to first.
        let mut hearts = places(draws, |n| n.starts_with("hud/heart/container"));
        hearts.reverse();
        let hardcore = if self.level.hardcore { "hardcore_" } else { "" };
        let container = sprite(if self.level.hardcore {
            "hud/heart/container_hardcore"
        } else {
            "hud/heart/container"
        });
        let whole = modified.ceil() as i32;
        let start = (health.ceil() / 2.0).max(0.0) as usize;
        let end = (modified / 2.0).ceil().max(0.0) as usize;
        for (i, &(x, y)) in hearts.iter().enumerate().take(end).skip(start) {
            if let Some(r) = container {
                batch.rect_uv(x, y, 9.0, 9.0, r, white(flash * 0.25));
            }
            let part = if i as i32 * 2 + 1 == whole { "half" } else { "full" };
            if let Some(r) = sprite(&format!("hud/heart/{hardcore}{part}")) {
                batch.rect_uv(x, y, 9.0, 9.0, r, white(flash));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regeneration_stops_once_hunger_falls_under_18() {
        // Under 18, natural regeneration doesn't heal.
        assert_eq!(estimated_health(17, 5.0, 0.0), 0.0);
        // At 18 with no saturation: one heal (6 exhaustion) takes hunger to 17 (4 + 2 spent).
        assert_eq!(estimated_health(18, 0.0, 0.0), 1.0);
        // Broken numbers give nothing rather than looping.
        assert_eq!(estimated_health(20, f32::NAN, 0.0), 0.0);
    }

    #[test]
    fn a_full_bar_heals_more_with_more_saturation() {
        let little = estimated_health(20, 1.0, 0.0);
        let more = estimated_health(20, 10.0, 0.0);
        assert!(more > little && little > 0.0, "{little} {more}");
    }
}
