//! A butterfly knife rigged with the player's arms and animated, as gun packs ship them (a
//! Bedrock geometry and its Bedrock animations: `oteria_art::KNIFE_RIG_FILE`, copied on this
//! computer, never in git). When it is there, it is the knife drawn in place of Oteria's own
//! (`oteria_knife.rs`): taken in hand it plays one of its draws, the inspect key one of its
//! inspects, each swing one of its slashes (in turn), and in between its idle, over and over.
//!
//! The rig is drawn as the packs' own mod draws it in first person: from its `idle_view` bone,
//! looking along -z, in the file's space mirrored in x (the Bedrock engine's), a bone turning
//! about its pivot by z, then y, then x, with x and y turned the other way. The arms' bones
//! (`righthand_pos`, `lefthand_pos`) hold a stand-in box each: the player's own arms are drawn
//! there instead. The packs' sounds are not in them: the knife's clicks (a handle or the blade
//! hitting its stop, found in the animations) and its slashes are the game's own sounds.

use super::{Game, Hand, Screen};
use glam::{EulerRot, Mat4, Quat, Vec3};
use horizon_client::mesher::Vertex;
use horizon_client::mobs::{Part, Polygon};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

/// The knife `name`'s rig (`oteria_art::KNIFE_RIGS`), when its files are on this computer and
/// read.
pub(super) fn rig(name: &str) -> Option<&'static Rig> {
    use horizon_client::oteria_art::{KNIFE_RIGS, knife_rig_bytes};
    static RIGS: [OnceLock<Option<Rig>>; KNIFE_RIGS.len()] =
        [const { OnceLock::new() }; KNIFE_RIGS.len()];
    let i = KNIFE_RIGS.iter().position(|k| *k == name)?;
    RIGS[i]
        .get_or_init(|| {
            // On disk beside the game, else built into it.
            let (geo, anims) = (
                knife_rig_bytes(name, "geo.json")?,
                knife_rig_bytes(name, "animation.json")?,
            );
            match Rig::parse(&geo, &anims, name) {
                Ok(rig) => {
                    log::info!("Oteria: the {name} is drawn from its rig");
                    Some(rig)
                }
                Err(e) => {
                    log::warn!("Oteria: the {name}'s rig doesn't read: {e}");
                    None
                }
            }
        })
        .as_ref()
}

/// The file's space to the drawn one: mirrored in x.
fn shown(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.y, v.z)
}
/// A rotation of the file (degrees) in the drawn space.
fn turn(r: Vec3) -> Quat {
    Quat::from_euler(
        EulerRot::ZYX,
        r.z.to_radians(),
        -r.y.to_radians(),
        -r.x.to_radians(),
    )
}

struct Bone {
    name: String,
    parent: Option<usize>,
    /// Its pivot, drawn space (pixels).
    pivot: Vec3,
    /// Its rotation at rest (degrees, as in the file).
    rotation: Vec3,
    /// Its boxes, drawn space (pixels); a box turned on its own is a child part.
    part: Part,
    /// The corners of its boxes (drawn space), for the arms' stand-ins.
    bounds: Option<(Vec3, Vec3)>,
}

#[derive(Clone, Copy, Debug)]
struct Key {
    time: f32,
    pre: Vec3,
    post: Vec3,
    catmull: bool,
}
#[derive(Debug)]
struct Track(Vec<Key>);

fn number(v: &Value) -> Option<f32> {
    v.as_f64()
        .map(|x| x as f32)
        .or_else(|| v.as_str()?.trim().parse().ok())
}
fn vec3(v: &Value) -> Option<Vec3> {
    match v {
        Value::Array(a) => {
            let n = |i: usize| a.get(i).and_then(number).unwrap_or(0.0);
            Some(Vec3::new(n(0), n(1), n(2)))
        }
        _ => number(v).map(Vec3::splat),
    }
}

impl Track {
    /// A channel: a value, or keys by time, each a value or `pre`/`post` with a `lerp_mode`.
    fn parse(v: &Value) -> Option<Track> {
        let key = |time: f32, k: &Value| {
            if let Some(o) = k.as_object() {
                let pre = o.get("pre").and_then(vec3);
                let post = o.get("post").and_then(vec3);
                let catmull = o.get("lerp_mode").and_then(Value::as_str) == Some("catmullrom");
                let (pre, post) = (pre.or(post)?, post.or(pre)?);
                Some(Key {
                    time,
                    pre,
                    post,
                    catmull,
                })
            } else {
                let v = vec3(k)?;
                Some(Key {
                    time,
                    pre: v,
                    post: v,
                    catmull: false,
                })
            }
        };
        let mut keys: Vec<Key> = match v.as_object() {
            Some(o) if !o.contains_key("pre") && !o.contains_key("post") => o
                .iter()
                .filter_map(|(t, k)| key(t.trim().parse().ok()?, k))
                .collect(),
            _ => vec![key(0.0, v)?],
        };
        keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        (!keys.is_empty()).then_some(Track(keys))
    }

    fn at(&self, t: f32) -> Vec3 {
        let k = &self.0;
        if t <= k[0].time {
            return k[0].post;
        }
        let i = k.partition_point(|x| x.time <= t);
        if i >= k.len() {
            return k[k.len() - 1].post;
        }
        let (a, b) = (k[i - 1], k[i]);
        let u = (t - a.time) / (b.time - a.time).max(1e-6);
        if a.catmull || b.catmull {
            let p0 = if i >= 2 { k[i - 2].post } else { a.post };
            let p3 = k.get(i + 1).map_or(b.pre, |k| k.pre);
            let (p1, p2) = (a.post, b.pre);
            let (u2, u3) = (u * u, u * u * u);
            0.5 * (2.0 * p1
                + (p2 - p0) * u
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2
                + (3.0 * p1 - p0 - 3.0 * p2 + p3) * u3)
        } else {
            a.post.lerp(b.pre, u)
        }
    }
}

#[derive(Default, Debug)]
struct Channels {
    position: Option<Track>,
    rotation: Option<Track>,
    scale: Option<Track>,
}

/// What a moment of an animation sounds like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Sound {
    /// A handle or the blade hits its stop.
    Click,
    /// A slash.
    Slash,
    /// Another of the pack's sounds (a draw, an inspect): a soft swish.
    Swish,
}

pub(super) struct Anim {
    pub length: f32,
    /// By bone (`Rig::bones`).
    bones: Vec<Option<Channels>>,
    /// In order of time.
    pub sounds: Vec<(f32, Sound)>,
    /// A stretch where the knife turns whole turns round the hand and comes back to the same
    /// pose (from, to: s): played over and over while the inspect key is held.
    pub spin: Option<(f32, f32)>,
}

/// A bone's animated values: its move (pixels, file axes), its turn (rest and animation,
/// drawn space) and its size. Blends go the short way round, whole turns apart or not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BonePose {
    pos: Vec3,
    rot: Quat,
    scale: Vec3,
}
impl Default for BonePose {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}
/// Every bone's (`Rig::bones`).
pub(super) type Pose = Vec<BonePose>;

pub(super) fn blend(a: &Pose, b: &Pose, u: f32) -> Pose {
    a.iter()
        .zip(b)
        .map(|(a, b)| BonePose {
            pos: a.pos.lerp(b.pos, u),
            rot: a.rot.slerp(b.rot, u),
            scale: a.scale.lerp(b.scale, u),
        })
        .collect()
}

pub(super) struct Rig {
    bones: Vec<Bone>,
    /// Where the eye is (`idle_view`'s pivot, drawn space).
    camera: Vec3,
    /// The right and the left arms' stand-ins.
    hands: [Option<usize>; 2],
    anims: HashMap<String, Anim>,
    /// The knife's bone.
    knife: String,
    /// Its texture's sprite.
    pub sprite: String,
    /// The bones not drawn: the eye's, the camera's and the placements' (`view`, `camera`,
    /// `positioning` and theirs), whose boxes only mark places.
    hidden: Vec<bool>,
}

/// The bones a stop is listened for on: the two handles and the blade.
const MOVING: [&str; 3] = ["up", "down", "blade2"];

impl Rig {
    /// `knife`: the knife's name, its bone's too (the one turned round the hand).
    pub(super) fn parse(geo: &[u8], anims: &[u8], knife: &str) -> Result<Rig, String> {
        let geo: Value = serde_json::from_slice(geo).map_err(|e| format!("geometry: {e}"))?;
        let anims: Value = serde_json::from_slice(anims).map_err(|e| format!("animations: {e}"))?;
        let model = geo["minecraft:geometry"]
            .get(0)
            .ok_or("no minecraft:geometry")?;
        let desc = &model["description"];
        let texture = [
            desc["texture_width"].as_f64().unwrap_or(64.0) as f32,
            desc["texture_height"].as_f64().unwrap_or(64.0) as f32,
        ];
        let list = model["bones"].as_array().ok_or("no bones")?;
        let names: Vec<String> = list
            .iter()
            .map(|b| b["name"].as_str().unwrap_or("").to_owned())
            .collect();
        let bones: Vec<Bone> = list
            .iter()
            .map(|b| {
                let pivot = shown(b.get("pivot").and_then(vec3).unwrap_or(Vec3::ZERO));
                let mut part = Part {
                    name: b["name"].as_str().unwrap_or("").to_owned(),
                    pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                    polygons: Vec::new(),
                    children: Vec::new(),
                };
                let mut bounds: Option<(Vec3, Vec3)> = None;
                for c in b["cubes"].as_array().into_iter().flatten() {
                    let origin = c.get("origin").and_then(vec3).unwrap_or(Vec3::ZERO);
                    let size = c.get("size").and_then(vec3).unwrap_or(Vec3::ZERO);
                    let inflate = c.get("inflate").and_then(number).unwrap_or(0.0);
                    let (lo, hi) = (origin - inflate, origin + size + inflate);
                    let (lo, hi) = (Vec3::new(-hi.x, lo.y, lo.z), Vec3::new(-lo.x, hi.y, hi.z));
                    bounds = Some(bounds.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
                    match c.get("rotation").and_then(vec3) {
                        Some(r) if r != Vec3::ZERO => {
                            let at = shown(c.get("pivot").and_then(vec3).unwrap_or(Vec3::ZERO));
                            // `render_model` turns by z, y, then x, as `turn` does.
                            let r = r * Vec3::new(-1.0, -1.0, 1.0) * std::f32::consts::PI / 180.0;
                            part.children.push(Part {
                                name: String::new(),
                                pose: [at.x, at.y, at.z, r.x, r.y, r.z, 1.0, 1.0, 1.0],
                                polygons: faces(c, lo - at, hi - at, texture),
                                children: Vec::new(),
                            });
                        }
                        _ => part.polygons.extend(faces(c, lo, hi, texture)),
                    }
                }
                Bone {
                    name: part.name.clone(),
                    parent: b["parent"]
                        .as_str()
                        .and_then(|p| names.iter().position(|n| n == p)),
                    pivot,
                    rotation: b.get("rotation").and_then(vec3).unwrap_or(Vec3::ZERO),
                    part,
                    bounds,
                }
            })
            .collect();
        let find = |name: &str| bones.iter().position(|b| b.name == name);
        let camera = find("idle_view")
            .map(|i| bones[i].pivot)
            .ok_or("no idle_view bone")?;
        let mut hidden: Vec<bool> = Vec::with_capacity(bones.len());
        for b in &bones {
            let marker = ["view", "camera", "positioning"].contains(&b.name.as_str());
            hidden.push(
                marker
                    || b.parent
                        .is_some_and(|p| hidden.get(p).copied().unwrap_or(false)),
            );
        }
        let mut rig = Rig {
            knife: knife.to_owned(),
            sprite: horizon_client::oteria_art::knife_rig_sprite(knife),
            hidden,
            hands: [find("righthand_pos"), find("lefthand_pos")],
            camera,
            anims: HashMap::new(),
            bones,
        };
        for (name, a) in anims["animations"].as_object().ok_or("no animations")? {
            let mut channels: Vec<Option<Channels>> = rig.bones.iter().map(|_| None).collect();
            for (bone, ch) in a["bones"].as_object().into_iter().flatten() {
                if let Some(i) = rig.bones.iter().position(|b| &b.name == bone) {
                    channels[i] = Some(Channels {
                        position: ch.get("position").and_then(Track::parse),
                        rotation: ch.get("rotation").and_then(Track::parse),
                        scale: ch.get("scale").and_then(Track::parse),
                    });
                }
            }
            let length = a["animation_length"].as_f64().map_or_else(
                || {
                    let ends = channels
                        .iter()
                        .flatten()
                        .flat_map(|c| [&c.position, &c.rotation, &c.scale]);
                    ends.flatten()
                        .filter_map(|t| t.0.last())
                        .map(|k| k.time)
                        .fold(0.0, f32::max)
                },
                |l| l as f32,
            );
            let mut anim = Anim {
                spin: None,
                length,
                bones: channels,
                sounds: Vec::new(),
            };
            anim.sounds = rig.sounds(&anim, a);
            anim.spin = rig.spin(&anim);
            rig.anims.insert(name.clone(), anim);
        }
        if !rig.anims.contains_key("static_idle") {
            return Err("no static_idle animation".into());
        }
        Ok(rig)
    }

    /// Its slashes (the file's sounds named so) and its clicks: a handle or the blade, turning
    /// fast, stopping.
    fn sounds(&self, anim: &Anim, file: &Value) -> Vec<(f32, Sound)> {
        let mut out: Vec<(f32, Sound)> = file["sound_effects"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(t, e)| {
                let slash = e["effect"].as_str()?.contains("slash");
                Some((
                    t.trim().parse().ok()?,
                    if slash { Sound::Slash } else { Sound::Swish },
                ))
            })
            .collect();
        const STEP: f32 = 1.0 / 48.0;
        let mut clicks: Vec<f32> = Vec::new();
        for (i, bone) in self.bones.iter().enumerate() {
            let Some(track) = MOVING
                .contains(&bone.name.as_str())
                .then(|| anim.bones[i].as_ref()?.rotation.as_ref())
                .flatten()
            else {
                continue;
            };
            let steps = (anim.length / STEP) as usize;
            let angle: Vec<f32> = (0..=steps).map(|k| track.at(k as f32 * STEP).x).collect();
            let speed: Vec<f32> = angle
                .windows(2)
                .map(|w| (w[1] - w[0]).abs() / STEP)
                .collect();
            for k in 3..speed.len() {
                let was_fast = speed[k.saturating_sub(4)..k].iter().any(|&s| s > 700.0);
                if speed[k] < 150.0 && was_fast {
                    clicks.push(k as f32 * STEP);
                }
            }
        }
        clicks.sort_by(f32::total_cmp);
        let mut last = f32::MIN;
        for t in clicks {
            if t - last > 0.12 {
                out.push((t, Sound::Click));
                last = t;
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }

    /// Where `anim` turns the knife (`butterfly`) round the hand whole turns and is back in the
    /// same pose, every bone within a few degrees and a fraction of a pixel: the stretch that
    /// fits best, from a quarter of a second to a second long, in its first two thirds.
    fn spin(&self, anim: &Anim) -> Option<(f32, f32)> {
        const STEP: f32 = 1.0 / 96.0;
        let knife = self.bones.iter().position(|b| b.name == self.knife)?;
        let turning = anim.bones[knife].as_ref()?.rotation.as_ref()?;
        let steps = (anim.length / STEP) as usize;
        let at = |k: usize| k as f32 * STEP;
        // Every channel sampled: (rotation?, values).
        let samples: Vec<(bool, Vec<Vec3>)> = anim
            .bones
            .iter()
            .flatten()
            .flat_map(|c| [(true, &c.rotation), (false, &c.position)])
            .filter_map(|(rot, t)| {
                let t = t.as_ref()?;
                Some((rot, (0..=steps).map(|k| t.at(at(k))).collect()))
            })
            .collect();
        let wrap = |d: f32| (d + 180.0).rem_euclid(360.0) - 180.0;
        let mut best: Option<(f32, usize, usize)> = None;
        for a in 0..steps * 2 / 3 {
            for b in a + (0.25 / STEP) as usize..(a + (1.0 / STEP) as usize).min(steps) {
                if (turning.at(at(b)) - turning.at(at(a))).abs().max_element() < 300.0 {
                    continue;
                }
                let error = samples
                    .iter()
                    .map(|(rot, v)| {
                        let d = v[b] - v[a];
                        if *rot {
                            d.to_array()
                                .map(wrap)
                                .iter()
                                .fold(0.0f32, |m, x| m.max(x.abs()))
                        } else {
                            d.abs().max_element() * 10.0
                        }
                    })
                    .fold(0.0f32, f32::max);
                if best.is_none_or(|(e, _, _)| error < e) {
                    best = Some((error, a, b));
                }
            }
        }
        best.filter(|(e, _, _)| *e < 6.0)
            .map(|(_, a, b)| (at(a), at(b)))
    }

    pub(super) fn anim(&self, name: &str) -> Option<&Anim> {
        self.anims.get(name)
    }

    /// The pose `anim` is in at `t` (s), the idle at `idle` (s) for the bones it doesn't move.
    pub(super) fn pose(&self, anim: Option<(&Anim, f32)>, idle: f32) -> Pose {
        let base = &self.anims["static_idle"];
        let idle = if base.length > 0.0 {
            idle % base.length
        } else {
            0.0
        };
        (0..self.bones.len())
            .map(|i| {
                let (ch, t) = match anim.and_then(|(a, t)| Some((a.bones[i].as_ref()?, t))) {
                    Some(found) => found,
                    None => match base.bones[i].as_ref() {
                        Some(c) => (c, idle),
                        None => {
                            return BonePose {
                                rot: turn(self.bones[i].rotation),
                                ..BonePose::default()
                            };
                        }
                    },
                };
                let turned = ch.rotation.as_ref().map_or(Vec3::ZERO, |k| k.at(t));
                BonePose {
                    pos: ch.position.as_ref().map_or(Vec3::ZERO, |k| k.at(t)),
                    rot: turn(self.bones[i].rotation + turned),
                    scale: ch.scale.as_ref().map_or(Vec3::ONE, |k| k.at(t)),
                }
            })
            .collect()
    }

    /// Each bone's matrix (blocks, from the eye) in `pose`.
    pub(super) fn matrices(&self, pose: &Pose) -> Vec<Mat4> {
        let eye = Mat4::from_translation(-self.camera / 16.0);
        let mut out: Vec<Mat4> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            // The file lists a parent before its children; one listed after is drawn from the eye.
            let parent = b.parent.filter(|&p| p < i).map_or(eye, |p| out[p]);
            let p = pose.get(i).copied().unwrap_or_default();
            out.push(
                parent
                    * Mat4::from_translation((b.pivot + shown(p.pos)) / 16.0)
                    * Mat4::from_quat(p.rot)
                    * Mat4::from_scale(p.scale)
                    * Mat4::from_translation(-b.pivot / 16.0),
            );
        }
        out
    }

    /// The knife (every bone but the arms' stand-ins), `base` taking the eye's space to the world.
    pub(super) fn render(
        &self,
        base: Mat4,
        mats: &[Mat4],
        uv: [f32; 4],
        light: u32,
        out: &mut Vec<Vertex>,
    ) {
        for (i, b) in self.bones.iter().enumerate() {
            if self.hands.contains(&Some(i))
                || self.hidden[i]
                || (b.part.polygons.is_empty() && b.part.children.is_empty())
            {
                continue;
            }
            horizon_client::mobs::render_model(
                &b.part,
                base * mats[i],
                &|p: &Part| p.pose,
                uv,
                light,
                0,
                out,
            );
        }
    }

    /// An arm's stand-in (the right's or the left's): its bone and its box's corners (drawn
    /// space, pixels).
    pub(super) fn hand(&self, left: bool) -> Option<(usize, (Vec3, Vec3))> {
        let i = self.hands[usize::from(left)]?;
        Some((i, self.bones[i].bounds?))
    }
}

/// A box's six faces (corners `lo`, `hi`, drawn space), each with its own UV (`uv` and
/// `uv_size`, in the texture's pixels; a negative size flips it).
fn faces(cube: &Value, lo: Vec3, hi: Vec3, texture: [f32; 2]) -> Vec<Polygon> {
    let ([x0, y0, z0], [x1, y1, z1]) = (lo.to_array(), hi.to_array());
    // Each face's corners seen from outside: top left, top right, bottom right, bottom left.
    let all: [(&str, [f32; 3], [[f32; 3]; 4]); 6] = [
        (
            "north",
            [0.0, 0.0, -1.0],
            [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]],
        ),
        (
            "south",
            [0.0, 0.0, 1.0],
            [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]],
        ),
        (
            "east",
            [1.0, 0.0, 0.0],
            [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]],
        ),
        (
            "west",
            [-1.0, 0.0, 0.0],
            [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]],
        ),
        (
            "up",
            [0.0, 1.0, 0.0],
            [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
        ),
        (
            "down",
            [0.0, -1.0, 0.0],
            [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
        ),
    ];
    all.into_iter()
        .filter_map(|(name, normal, corners)| {
            let face = cube.get("uv")?.get(name)?;
            let at = face.get("uv").and_then(vec3)?;
            let size = face.get("uv_size").and_then(vec3)?;
            let (u0, v0) = (at.x / texture[0], at.y / texture[1]);
            let (u1, v1) = ((at.x + size.x) / texture[0], (at.y + size.y) / texture[1]);
            let uv = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
            let vertices = std::array::from_fn(|k| {
                let [x, y, z] = corners[k];
                [x, y, z, uv[k][0], uv[k][1]]
            });
            Some(Polygon {
                normal,
                vertices,
                cull_back: false,
            })
        })
        .collect()
}

/// The animation playing over the idle, and the pose it started from.
struct Playing {
    anim: &'static Anim,
    /// When its time was 0.
    started: Instant,
    played: usize,
    /// The pose it blends from, since when, over how long (s).
    from: Option<(Pose, Instant, f32)>,
    /// The inspect key that started it is held: its spin plays over and over.
    holding: bool,
    /// The time spent spinning, once the key is let go (s).
    skipped: f32,
    /// Its time last frame (a smaller one: it went round again).
    last: f32,
}
impl Playing {
    fn time(&self) -> f32 {
        let t = self.started.elapsed().as_secs_f32() - self.skipped;
        match self.anim.spin {
            Some((from, to)) if self.holding && t > to => from + (t - from) % (to - from),
            _ => t,
        }
    }
    /// The key is let go: the turn under way ends, then the rest plays.
    fn let_go(&mut self) {
        let now = self.time();
        self.holding = false;
        self.skipped = self.started.elapsed().as_secs_f32() - now;
    }
}
struct State {
    playing: Option<Playing>,
    /// The pose the last animation ended in, blended back into the idle.
    ended: Option<(Pose, Instant)>,
    idle: Instant,
    /// The slot and item last shown as the knife: another one, and it is drawn again.
    held: Option<(usize, String)>,
    /// The inspect key is down (its repeats start nothing).
    inspect_down: bool,
    /// The swings counted so far, and the next slash (0 or 1).
    swings: u32,
    slash: usize,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        playing: None,
        ended: None,
        idle: Instant::now(),
        held: None,
        inspect_down: false,
        swings: 0,
        slash: 0,
    });
}

/// How long the idle takes back from an animation that ends (s).
const BACK: f32 = 0.12;
/// The inspects; a press of the key plays the one that spins, a tap (let go sooner than
/// `TAP`) any of them.
const INSPECTS: [&str; 3] = ["inspect", "inspect_1", "inspect_2"];
const TAP: f32 = 0.2;

/// One of `names`, by chance.
fn any_of<'a>(names: &[&'a str]) -> &'a str {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    names[nanos as usize % names.len()]
}

impl State {
    /// The pose now.
    fn pose(&self, rig: &Rig) -> Pose {
        let idle = self.idle.elapsed().as_secs_f32();
        let Some(p) = &self.playing else {
            let now = rig.pose(None, idle);
            return match &self.ended {
                Some((from, at)) if at.elapsed().as_secs_f32() < BACK => {
                    blend(from, &now, at.elapsed().as_secs_f32() / BACK)
                }
                _ => now,
            };
        };
        let t = p.time();
        let now = rig.pose(Some((p.anim, t.min(p.anim.length))), idle);
        match &p.from {
            Some((from, since, over)) if since.elapsed().as_secs_f32() < *over => {
                blend(from, &now, since.elapsed().as_secs_f32() / over)
            }
            _ => now,
        }
    }

    fn play(&mut self, rig: &'static Rig, names: &[&str], over: f32) {
        if let Some(anim) = rig
            .anim(any_of(names))
            .or_else(|| names.iter().find_map(|n| rig.anim(n)))
        {
            self.play_at(rig, anim, 0.0, over, false);
        }
    }

    /// `anim` from its time `at` (s), blended in from the pose now over `over` (s).
    fn play_at(&mut self, rig: &Rig, anim: &'static Anim, at: f32, over: f32, holding: bool) {
        let now = Instant::now();
        let from = (over > 0.0).then(|| (self.pose(rig), now, over));
        let started = now
            .checked_sub(std::time::Duration::from_secs_f32(at))
            .unwrap_or(now);
        let played = anim.sounds.iter().take_while(|s| s.0 < at).count();
        self.playing = Some(Playing {
            anim,
            started,
            played,
            from,
            holding,
            skipped: 0.0,
            last: at,
        });
        self.ended = None;
    }
}

impl Game<'_> {
    /// The inspect key with the rig: the inspect that spins the knife (not again while the key
    /// is held, its spin over and over meanwhile).
    pub(super) fn knife_rig_inspect(&mut self, rig: &'static Rig) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.inspect_down {
                return;
            }
            s.inspect_down = true;
            let spinning = INSPECTS
                .iter()
                .filter_map(|n| rig.anim(n))
                .find(|a| a.spin.is_some());
            if let Some(anim) = spinning.or_else(|| rig.anim(INSPECTS[0])) {
                s.play_at(rig, anim, 0.0, 0.15, true);
            }
        });
    }

    /// Once a frame with the rig: a draw when the knife comes into the hand, a slash for each
    /// swing of it, the playing animation's sounds as it reaches them.
    pub(super) fn knife_rig_frame(
        &mut self,
        rig: &'static Rig,
        held: Option<(usize, String)>,
        key_down: bool,
    ) {
        let swings = self.swings_started;
        let main_swing = self.swing.attack_hand() == Hand::Main && self.screen == Screen::Playing;
        let due: Vec<Sound> = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.inspect_down &= key_down;
            // The inspect key let go: a tap plays any inspect, a hold finishes the spin.
            let released = s.playing.as_ref().filter(|p| p.holding && !key_down);
            if let Some((t, current)) = released.map(|p| (p.time(), p.anim)) {
                let anim = rig.anim(any_of(&INSPECTS));
                match anim.filter(|a| t < TAP && !std::ptr::eq(*a, current)) {
                    Some(other) => s.play_at(rig, other, t, 0.1, false),
                    None => {
                        if let Some(p) = s.playing.as_mut() {
                            p.let_go();
                        }
                    }
                }
            }
            let swung = swings != s.swings;
            s.swings = swings;
            if held != s.held {
                s.held = held.clone();
                s.playing = None;
                s.ended = None;
                if held.is_some() {
                    s.play(rig, &["draw", "draw_1"], 0.0);
                }
                return Vec::new();
            }
            if held.is_none() {
                return Vec::new();
            }
            // A swing while a slash has only just started doesn't start another.
            let slashing = s.playing.as_ref().is_some_and(|p| {
                rig.anims
                    .iter()
                    .any(|(n, a)| n.starts_with("melee") && std::ptr::eq(a, p.anim))
                    && p.started.elapsed().as_secs_f32() < 0.25
            });
            if swung && main_swing && !slashing {
                let name = ["melee_1", "melee_2"][s.slash];
                s.slash = (s.slash + 1) % 2;
                s.play(rig, &[name], 0.06);
            }
            let Some(p) = s.playing.as_mut() else {
                return Vec::new();
            };
            let t = p.time();
            if t < p.last {
                // Round again: the spin's sounds again.
                let from = p.anim.spin.map_or(0.0, |r| r.0);
                p.played = p.anim.sounds.iter().take_while(|s| s.0 < from).count();
            }
            p.last = t;
            let mut due = Vec::new();
            while p.played < p.anim.sounds.len() && p.anim.sounds[p.played].0 <= t {
                due.push(p.anim.sounds[p.played].1);
                p.played += 1;
            }
            if t >= p.anim.length {
                let last = rig.pose(
                    Some((p.anim, p.anim.length)),
                    s.idle.elapsed().as_secs_f32(),
                );
                s.playing = None;
                s.ended = Some((last, Instant::now()));
                // The idle starts over, from the pose the animations end in.
                s.idle = Instant::now();
            }
            due
        });
        for sound in due {
            let (event, volume, pitch) = match sound {
                Sound::Click => ("minecraft:block.tripwire.click_on", 0.6, 1.8),
                Sound::Slash => ("minecraft:entity.player.attack.sweep", 0.25, 1.5),
                Sound::Swish => ("minecraft:entity.player.attack.sweep", 0.1, 1.9),
            };
            self.audio.play(event, None, volume, pitch);
        }
    }

    /// The rig in first person, the player's arms in its stand-ins: `to_world` is the hands'
    /// view space, `inverse_height` the hand's equip height (it lowers the knife as another
    /// item comes in). The vertices where the knife starts are returned (the arms before them).
    pub(super) fn draw_knife_rig(
        &self,
        rig: &'static Rig,
        to_world: Mat4,
        left: bool,
        inverse_height: f32,
        light: u32,
        out: &mut Vec<Vertex>,
    ) -> usize {
        let (pose, drawing) = STATE.with(|s| {
            let s = s.borrow();
            let drawing = s.playing.as_ref().is_some_and(|p| {
                ["draw", "draw_1"]
                    .iter()
                    .any(|n| rig.anim(n).is_some_and(|a| std::ptr::eq(a, p.anim)))
            });
            (s.pose(rig), drawing)
        });
        // The draw brings the knife up itself.
        let lower = if drawing { 0.0 } else { inverse_height * -0.6 };
        self.draw_knife_rig_posed(rig, &pose, to_world, left, lower, light, out)
    }

    /// The rig in `pose`, the player's arms in its stand-ins, lowered by `lower` (blocks).
    #[allow(clippy::too_many_arguments)]
    fn draw_knife_rig_posed(
        &self,
        rig: &'static Rig,
        pose: &Pose,
        to_world: Mat4,
        left: bool,
        lower: f32,
        light: u32,
        out: &mut Vec<Vertex>,
    ) -> usize {
        let mats = rig.matrices(pose);
        // A left main hand: the rig mirrored.
        let side = if left { -1.0 } else { 1.0 };
        let base = to_world
            * Mat4::from_translation(Vec3::new(0.0, lower, 0.0))
            * Mat4::from_scale(Vec3::new(side, 1.0, 1.0));
        if !self.own_invisible() {
            let slim = self.player_model.as_ref().is_some_and(|m| m.slim);
            for stand_in in [false, true] {
                let Some((bone, (lo, hi))) = rig.hand(stand_in) else {
                    continue;
                };
                // Mirrored, the right stand-in is the left arm's.
                let arm_left = stand_in != left;
                let Some(arm) = self.res.player_arm(slim, arm_left).as_ref() else {
                    continue;
                };
                // The arm's box (its part's own space, its hand end at +y) on the stand-in's,
                // its hand end at the top.
                let corners = arm
                    .polygons
                    .iter()
                    .flat_map(|p| p.vertices.iter().map(|v| Vec3::new(v[0], v[1], v[2])));
                let (alo, ahi) =
                    corners.fold((Vec3::MAX, Vec3::MIN), |(a, b), v| (a.min(v), b.max(v)));
                if alo.x > ahi.x {
                    continue;
                }
                let at = Vec3::new(
                    (lo.x + hi.x - alo.x - ahi.x) / 2.0,
                    hi.y - ahi.y,
                    (lo.z + hi.z - alo.z - ahi.z) / 2.0,
                );
                let pivot = Vec3::new(arm.pose[0], arm.pose[1], arm.pose[2]);
                // `first_person_arm` turns the arm 0.1 rad outwards about its pivot: undone.
                let z_rot = if arm_left { -0.1 } else { 0.1 };
                let m = base
                    * mats[bone]
                    * Mat4::from_translation(at / 16.0)
                    * Mat4::from_rotation_z(-z_rot)
                    * Mat4::from_translation(-pivot / 16.0);
                self.first_person_arm(arm_left, m, light, out);
            }
        }
        let start = out.len();
        let Some(r) = self.gui.atlas.sprites.get(&rig.sprite).copied() else {
            return start;
        };
        rig.render(base, &mats, [r.u0, r.v0, r.u1, r.v1], light, out);
        start
    }

    /// The knife `knife` alone (no arms), at rest: its vertices in the eye's space (blocks),
    /// for its card.
    pub(super) fn knife_alone(&self, knife: &str) -> Option<Vec<Vertex>> {
        let rig = rig(knife)?;
        let r = self.gui.atlas.sprites.get(&rig.sprite).copied()?;
        let mats = rig.matrices(&rig.pose(None, 0.0));
        let mut out = Vec::new();
        rig.render(
            Mat4::IDENTITY,
            &mats,
            [r.u0, r.v0, r.u1, r.v1],
            240 | 240 << 8,
            &mut out,
        );
        (!out.is_empty()).then_some(out)
    }

    /// The Cosmetics page's preview of `knife` in `well` (GUI pixels): the knife in the
    /// player's arms as first person shows it, its draw and inspects on a loop (`clock`, s),
    /// seen through the game's hand camera (`HAND_FOV` up and down, centred), so it is framed as
    /// in the game.
    pub(super) fn draw_knife_preview(
        &self,
        batch: &mut horizon_client::hud::HudBatch,
        well: [f32; 4],
        knife: &str,
        clock: f32,
    ) {
        let Some(rig) = rig(knife) else {
            return;
        };
        let pose = preview_pose(rig, clock);
        let mut verts = Vec::new();
        self.draw_knife_rig_posed(
            rig,
            &pose,
            Mat4::IDENTITY,
            false,
            0.0,
            240 | 240 << 8,
            &mut verts,
        );
        let [x, y, w, h] = well;
        let k = (h / 2.0) / (HAND_FOV.to_radians() / 2.0).tan();
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let mut seen = Vec::with_capacity(verts.len());
        for q in verts.as_chunks::<4>().0 {
            // Nothing that reaches behind the eye.
            if q.iter().any(|v| v.pos[2] > -0.02) {
                continue;
            }
            for v in q {
                let [px, py, pz] = v.pos;
                let mut v = *v;
                v.pos = [cx + px / -pz * k, cy - py / -pz * k, pz];
                seen.push(v);
            }
        }
        let n = seen.len();
        let quads = super::perspective::player_preview_quads_no_cull(
            seen,
            Mat4::IDENTITY,
            None,
            &self.gui.atlas.sprites,
            0..n,
        );
        super::perspective::emit_preview(batch, quads, Vec::new(), [x, y, x + w, y + h]);
    }
}

/// The hand camera's field of view up and down (degrees): `camera::hand_projection`'s.
const HAND_FOV: f32 = 70.0;
/// The rest between the preview's animations (s).
const PREVIEW_REST: f32 = 0.7;

/// The preview's pose at `clock` (s): the draw and each inspect in turn, a rest of idle after
/// each, blended back into it.
fn preview_pose(rig: &Rig, clock: f32) -> Pose {
    let anims: Vec<&Anim> = ["draw", "inspect", "inspect_1", "inspect_2"]
        .iter()
        .filter_map(|n| rig.anim(n))
        .filter(|a| a.length > 0.0)
        .collect();
    let total: f32 = anims.iter().map(|a| a.length + PREVIEW_REST).sum();
    if total <= 0.0 {
        return rig.pose(None, clock);
    }
    let mut t = clock % total;
    for a in anims {
        if t < a.length {
            return rig.pose(Some((a, t)), clock);
        }
        t -= a.length;
        if t < PREVIEW_REST {
            let idle = rig.pose(None, clock);
            return if t < BACK {
                blend(&rig.pose(Some((a, a.length)), clock), &idle, t / BACK)
            } else {
                idle
            };
        }
        t -= PREVIEW_REST;
    }
    rig.pose(None, clock)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rigged knives copied on this computer (`private/cosmetics`, not in git) read, each
    /// with a draw, an inspect and an idle: `cargo test -p horizon-client --lib
    /// the_rigged_knives_on_this_computer_read -- --ignored`.
    #[test]
    #[ignore]
    fn the_rigged_knives_on_this_computer_read() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../private/cosmetics");
        for knife in horizon_client::oteria_art::KNIFE_RIGS {
            let read = |ext: &str| std::fs::read(dir.join(format!("{knife}_rig.{ext}")));
            let (Ok(geo), Ok(anims)) = (read("geo.json"), read("animation.json")) else {
                println!("{knife}: not on this computer");
                continue;
            };
            let rig = Rig::parse(&geo, &anims, knife).unwrap_or_else(|e| panic!("{knife}: {e}"));
            for anim in ["draw", "inspect", "idle"] {
                assert!(rig.anim(anim).is_some(), "{knife} has no {anim}");
            }
            println!("{knife}: read");
        }
    }

    const GEO: &str = r#"{"format_version":"1.12.0","minecraft:geometry":[{
        "description":{"texture_width":64,"texture_height":64},
        "bones":[
            {"name":"root","pivot":[0,0,0]},
            {"name":"arm","parent":"root","pivot":[2,0,0]},
            {"name":"righthand_pos","parent":"arm","pivot":[0,8,0],
             "cubes":[{"origin":[4,8,-2],"size":[3,12,4],"uv":{"north":{"uv":[0,0],"uv_size":[4,12]}}}]},
            {"name":"blade2","parent":"arm","pivot":[1,0,0],
             "cubes":[{"origin":[0,0,-4],"size":[1,1,4],"rotation":[10,0,0],"pivot":[1,0,0],
                       "uv":{"north":{"uv":[0,0],"uv_size":[1,1]},"down":{"uv":[2,8],"uv_size":[4,-4]}}}]},
            {"name":"idle_view","pivot":[0,2,1]}
        ]}]}"#;
    const ANIMS: &str = r#"{"format_version":"1.8.0","animations":{
        "static_idle":{"loop":true,"animation_length":1.0,"bones":{"arm":{"rotation":[0,0,0]}}},
        "draw":{"animation_length":1.0,"bones":{
            "arm":{"position":{"0.0":[2,0,0],"1.0":[4,0,0]}},
            "blade2":{"rotation":{"0.0":[0,0,0],"0.25":[180,0,0],"0.5":[180,0,0],"1.0":{"post":[200,0,0],"lerp_mode":"catmullrom"}},
                      "scale":2}},
         "sound_effects":{"0.0":{"effect":"x:draw"}}},
        "melee_1":{"animation_length":0.5,"bones":{},"sound_effects":{"0.1":{"effect":"x:knife_slash1"}}}
    }}"#;

    fn rig() -> Rig {
        Rig::parse(GEO.as_bytes(), ANIMS.as_bytes(), "butterfly").expect("the rig reads")
    }

    #[test]
    fn the_rig_reads_mirrored_in_x() {
        let rig = rig();
        assert_eq!(rig.camera, Vec3::new(0.0, 2.0, 1.0));
        let (bone, (lo, hi)) = rig.hand(false).expect("the right arm's stand-in");
        assert_eq!(rig.bones[bone].name, "righthand_pos");
        assert_eq!(
            (lo, hi),
            (Vec3::new(-7.0, 8.0, -2.0), Vec3::new(-4.0, 20.0, 2.0))
        );
        assert!(rig.hand(true).is_none());
        // The turned box is a part of its own about its pivot, mirrored.
        let blade = &rig.bones[3].part;
        assert_eq!(blade.children.len(), 1);
        assert_eq!(&blade.children[0].pose[..3], &[-1.0, 0.0, 0.0]);
        assert!((blade.children[0].pose[3] + 10f32.to_radians()).abs() < 1e-5);
        // Its down face's UV flips.
        let down = &blade.children[0].polygons[1];
        assert_eq!((down.vertices[0][4], down.vertices[3][4]), (0.125, 0.0625));
    }

    #[test]
    fn an_animation_moves_its_bones_and_the_idle_the_rest() {
        let rig = rig();
        let draw = rig.anim("draw").unwrap();
        let pose = rig.pose(Some((draw, 0.5)), 0.0);
        assert_eq!(pose[1].pos, Vec3::new(3.0, 0.0, 0.0));
        assert!(pose[3].rot.angle_between(turn(Vec3::new(180.0, 0.0, 0.0))) < 1e-4);
        assert_eq!(pose[3].scale, Vec3::splat(2.0));
        assert_eq!(
            rig.pose(Some((draw, 9.0)), 0.0)[1].pos,
            Vec3::new(4.0, 0.0, 0.0)
        );
        // Toward a catmull-rom key, it eases (not a straight line).
        let blade = draw.bones[3]
            .as_ref()
            .and_then(|c| c.rotation.as_ref())
            .unwrap();
        let eased = blade.at(0.6).x;
        assert!(
            eased > 180.0 && eased < 200.0 && (eased - 184.0).abs() > 0.1,
            "{eased}"
        );
        // A bone it doesn't move stays at rest; the eye's matrix puts the eye at the origin.
        let mats = rig.matrices(&rig.pose(None, 0.0));
        assert!(
            mats[4]
                .transform_point3(Vec3::new(0.0, 2.0, 1.0) / 16.0)
                .length()
                < 1e-6
        );
        // Moved +x in the file is moved -x when drawn.
        let mats = rig.matrices(&pose);
        let p = mats[1].transform_point3(Vec3::ZERO)
            - rig.matrices(&rig.pose(None, 0.0))[1].transform_point3(Vec3::ZERO);
        assert!((p.x + 3.0 / 16.0).abs() < 1e-6, "{p}");
    }

    #[test]
    fn the_sounds_are_its_slashes_and_its_stops() {
        let rig = rig();
        assert_eq!(rig.anim("melee_1").unwrap().sounds, [(0.1, Sound::Slash)]);
        // Its own sound at the start, a swish; the blade turns half a turn in a quarter of a
        // second, then stops: a click.
        let draw = &rig.anim("draw").unwrap().sounds;
        assert_eq!(draw.len(), 2, "{draw:?}");
        assert_eq!(draw[0], (0.0, Sound::Swish));
        assert!(
            draw[1].1 == Sound::Click && (draw[1].0 - 0.25).abs() < 0.05,
            "{draw:?}"
        );
    }

    /// The rigs copied on this computer, when there are some, read with their arms and
    /// animations, and draw no marker of the eye's.
    #[test]
    fn the_copied_rigs_read_when_there_are_some() {
        for knife in horizon_client::oteria_art::KNIFE_RIGS {
            let file = format!(
                "{}/../../private/cosmetics/{knife}_rig",
                env!("CARGO_MANIFEST_DIR")
            );
            let read = |ext: &str| std::fs::read(format!("{file}.{ext}")).ok();
            let (Some(geo), Some(anims)) = (read("geo.json"), read("animation.json")) else {
                continue;
            };
            let rig = Rig::parse(&geo, &anims, knife).expect("the copied rig reads");
            assert!(rig.hand(false).is_some());
            let view = rig
                .bones
                .iter()
                .position(|b| b.name == "idle_view")
                .unwrap();
            assert!(rig.hidden[view] && !rig.hidden[rig.hands[0].unwrap()]);
            for name in ["draw", "inspect", "melee_1"] {
                let anim = rig.anim(name).expect(name);
                eprintln!(
                    "{knife} {name}: {} s, {:?}, spin {:?}",
                    anim.length, anim.sounds, anim.spin
                );
            }
        }
    }

    #[test]
    fn a_blend_takes_the_short_way_round() {
        // A whole turn apart: the same pose all the way, no turn back.
        let a = vec![BonePose {
            rot: turn(Vec3::new(0.0, 0.0, 165.7)),
            ..BonePose::default()
        }];
        let b = vec![BonePose {
            rot: turn(Vec3::new(0.0, 0.0, -194.3)),
            ..BonePose::default()
        }];
        assert!(blend(&a, &b, 0.5)[0].rot.angle_between(a[0].rot) < 1e-3);
    }

    #[test]
    fn the_spin_repeats_round_the_same_pose() {
        let rig = rig();
        // The test rig's animations don't spin the knife.
        assert!(rig.anim("draw").unwrap().spin.is_none());
        let file = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../private/cosmetics/butterfly_rig"
        );
        let read = |ext: &str| std::fs::read(format!("{file}.{ext}")).ok();
        let (Some(geo), Some(anims)) = (read("geo.json"), read("animation.json")) else {
            return;
        };
        let rig = Rig::parse(&geo, &anims, "butterfly").unwrap();
        let inspect = rig.anim("inspect").unwrap();
        let (from, to) = inspect.spin.expect("the inspect spins the knife");
        eprintln!("spin {from}..{to}");
        let (a, b) = (
            rig.matrices(&rig.pose(Some((inspect, from)), 0.0)),
            rig.matrices(&rig.pose(Some((inspect, to)), 0.0)),
        );
        for (a, b) in a.iter().zip(&b) {
            assert!(a.abs_diff_eq(*b, 0.06), "{a} {b}");
        }
    }

    #[test]
    fn held_the_spin_goes_round_and_let_go_the_rest_plays() {
        static ANIM: OnceLock<Anim> = OnceLock::new();
        let anim = ANIM.get_or_init(|| Anim {
            length: 2.0,
            bones: Vec::new(),
            sounds: Vec::new(),
            spin: Some((0.5, 0.9)),
        });
        let ago = |s: f32| Instant::now() - std::time::Duration::from_secs_f32(s);
        let mut p = Playing {
            anim,
            started: ago(3.0),
            played: 0,
            from: None,
            holding: true,
            skipped: 0.0,
            last: 0.0,
        };
        let t = p.time();
        assert!((0.5..=0.9).contains(&t), "{t}");
        p.let_go();
        assert!((p.time() - t).abs() < 0.01, "it goes on from where it was");
        // Not held, it plays straight through.
        let p = Playing {
            holding: false,
            started: ago(1.5),
            ..p
        };
        assert!((p.time() - 1.5 + p.skipped).abs() < 0.01);
    }
}
