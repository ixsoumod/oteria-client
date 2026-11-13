//! Oteria's voice chat in the game: Simple Voice Chat's protocol (`proto`), so a player on this
//! client and players on Fabric (or any client with that mod) hear each other on a server that
//! runs it (its Fabric mod or its Paper plugin). As that mod: push-to-talk or voice activation,
//! voices placed where the players are, groups, muting the microphone or the sound, an icon by
//! each player's name, and its menu on V (`oteria_voice_ui`), in Oteria's style.
//!
//! On a server, once joined with the Voice chat mod on: the voice channels are registered and
//! the secret asked for; the server's answer starts the voice connection (`engine`). Nothing is
//! sent to a server that doesn't run it (it never answers).

mod engine;
pub(super) mod proto;

use super::oteria_voice_ui::{VAction, VoiceView};
use super::{Game, Screen};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::oteria_icons as icon;
use horizon_protocol::packets::{configuration::CustomPayload, play::Serverbound as S};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;
use std::time::Instant;
use winit::keyboard::KeyCode;

/// The small icons drawn by names and on the HUD (`oteria_icons::small` of these).
pub(super) const TALKING: &str = "gui/sprites/oteria_icons_small/volume";
pub(super) const SOUND_OFF: &str = "gui/sprites/oteria_icons_small/volume-off";
pub(super) const DISCONNECTED: &str = "gui/sprites/oteria_icons_small/wifi-off";

#[derive(Default)]
struct Voice {
    /// The connection (`Session::serial`) the secret was asked on.
    asked: Option<u64>,
    session: Option<engine::Session>,
    /// What the server says of each player: name, group, sound off, not connected.
    states: HashMap<u128, proto::PlayerState>,
    /// The groups the server lists, and ours.
    groups: BTreeMap<u128, proto::Group>,
    group: Option<u128>,
    /// The last group join refused for its password.
    wrong_password: bool,
    /// The devices the audio was opened on, to open it again when they change.
    devices: (String, String),
    /// The microphone test, on from the settings page.
    loopback: bool,
    /// Faces by UUID (skin textures), refreshed now and then.
    faces: HashMap<u128, String>,
    faces_at: Option<Instant>,
    /// The microphones and speakers there are.
    device_list: Option<(Vec<String>, Vec<String>)>,
}

thread_local! {
    static VOICE: RefCell<Voice> = RefCell::new(Voice::default());
}

/// The key named `name` (`key.keyboard.v`) as a key code.
fn code_of(name: &str, held: &std::collections::HashSet<KeyCode>) -> Vec<KeyCode> {
    held.iter()
        .copied()
        .filter(|k| horizon_client::keybinds::key_name(*k) == Some(name))
        .collect()
}

impl Game<'_> {
    /// Once a frame: asks for the secret on a new server, stops on leaving or turning it off,
    /// and tells the voice thread where everyone is and what is set.
    pub(super) fn voice_frame(&mut self) {
        let v = self.settings.extras.pvp.voice.clone();
        let joined = self
            .remote
            .as_ref()
            .filter(|r| r.joined && r.reconfiguring.is_none())
            .map(|r| r.session.serial());
        let joined = joined.filter(|_| !self.has_singleplayer_server());
        let Some(serial) = joined.filter(|_| v.enabled) else {
            VOICE.with(|voice| {
                let mut voice = voice.borrow_mut();
                if voice.session.is_some() || voice.asked.is_some() {
                    *voice = Voice::default();
                }
            });
            return;
        };
        let ask = VOICE.with(|voice| {
            let mut voice = voice.borrow_mut();
            if voice.asked == Some(serial) {
                return false;
            }
            *voice = Voice { asked: Some(serial), ..Voice::default() };
            true
        });
        if ask {
            self.send_voice("minecraft:register", proto::register_payload());
            self.send_voice(proto::REQUEST_SECRET, proto::request_secret());
        }

        // The world as the voice thread needs it.
        let held = self.screen == Screen::Playing && !code_of(&v.key, &self.physical_keys).is_empty();
        let listener = self.player.as_ref().map(|p| {
            let at = p.position;
            [at.x, at.y + f64::from(p.dimensions.eye_height), at.z]
        });
        let players: HashMap<u128, [f64; 3]> = self
            .remote
            .as_ref()
            .map(|r| {
                self.mobs
                    .iter()
                    .filter(|m| &*m.kind.id == "minecraft:player")
                    .filter_map(|m| {
                        let uuid = r.player_uuid(m.id)?;
                        Some((uuid, [m.position.x, m.position.y + 1.5, m.position.z]))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let yaw = self.yaw;
        let faces_due = VOICE.with(|voice| {
            let voice = voice.borrow();
            voice.session.is_some()
                && voice.faces_at.is_none_or(|t| t.elapsed().as_secs_f32() > 1.0)
        });
        let faces = if faces_due { self.voice_faces() } else { None };
        VOICE.with(|voice| {
            let mut voice = voice.borrow_mut();
            if let Some(faces) = faces {
                voice.faces = faces;
                voice.faces_at = Some(Instant::now());
            }
            let devices = (v.mic_device.clone(), v.speaker_device.clone());
            let loopback = voice.loopback;
            let reopen = voice.devices != devices;
            voice.devices = devices.clone();
            let Some(session) = voice.session.as_mut() else {
                return;
            };
            if reopen {
                session.reopen_audio(&devices.0, &devices.1);
            }
            let mut w = session.shared.world.lock().unwrap();
            if let Some(listener) = listener {
                w.listener = (listener, yaw);
            }
            // Places sent by the server for a location's voice stay until a player has it.
            w.players.extend(players);
            w.push_to_talk = held;
            w.volumes = v.volumes.iter().map(|(id, pct)| (*id, f32::from(*pct) / 100.0)).collect();
            w.settings = Some(engine::Settings {
                activation: v.activation,
                threshold: f32::from(v.threshold),
                mic: f32::from(v.mic) / 100.0,
                volume: f32::from(v.volume) / 100.0,
                muted: v.muted,
                deafened: v.deafened,
                loopback,
                spatial: v.spatial,
                noise: v.noise,
            });
        });
    }

    /// A key pressed in play: the voice chat's menu (V, or the key chosen), the microphone (M),
    /// the sound (N), the group (G), as Simple Voice Chat's. True when taken.
    pub(super) fn voice_key_press(&mut self, name: &str) -> bool {
        let v = &self.settings.extras.pvp.voice;
        if !v.enabled || self.screen != Screen::Playing {
            return false;
        }
        match name {
            n if n == v.menu_key => self.open_voice_menu(false),
            "key.keyboard.m" => self.voice_action(VAction::ToggleMute),
            "key.keyboard.n" => self.voice_action(VAction::ToggleDeafen),
            "key.keyboard.g" => self.open_voice_menu(true),
            _ => return false,
        }
        true
    }

    /// The players' faces (their skins), by UUID.
    fn voice_faces(&mut self) -> Option<HashMap<u128, String>> {
        let (players, _, _) = self.remote_tab_players()?;
        let mut faces = HashMap::new();
        for p in players {
            let skin = self.face_skin(p.uuid, &p.skin);
            faces.insert(p.uuid, skin);
        }
        Some(faces)
    }

    fn send_voice(&self, channel: &str, data: Vec<u8>) {
        self.send(S::CustomPayload(CustomPayload { channel: channel.into(), data }));
    }

    /// A `voicechat:*` message from the server. True when taken.
    pub(super) fn voice_payload(&mut self, channel: &str, data: &[u8]) -> bool {
        if !channel.starts_with("voicechat:") {
            return false;
        }
        match channel {
            proto::SECRET => {
                let Some(secret) = proto::Secret::parse(data) else {
                    log::warn!("Oteria voice: a secret this client can't read");
                    return true;
                };
                let game_host = self
                    .remote
                    .as_ref()
                    .map(|r| proto::split_host_port(&r.session.address).0.to_owned())
                    .unwrap_or_default();
                let (host, port) = secret.address(&game_host);
                let v = &self.settings.extras.pvp.voice;
                let devices = (v.mic_device.clone(), v.speaker_device.clone());
                let session = engine::Session::start(host, port, secret, &devices.0, &devices.1);
                VOICE.with(|voice| {
                    let mut voice = voice.borrow_mut();
                    voice.session = Some(session);
                    voice.devices = devices;
                });
                // Our sound is on unless turned off: the others see it.
                let deafened = self.settings.extras.pvp.voice.deafened;
                self.send_voice(proto::UPDATE_STATE, proto::update_state(deafened));
            }
            proto::STATES => {
                if let Some(states) = proto::PlayerState::read_all(data) {
                    VOICE.with(|voice| {
                        voice.borrow_mut().states = states.into_iter().map(|s| (s.uuid, s)).collect();
                    });
                }
            }
            proto::STATE => {
                if let Some(s) = proto::PlayerState::read(&mut proto::Reader::new(data)) {
                    VOICE.with(|voice| voice.borrow_mut().states.insert(s.uuid, s));
                }
            }
            proto::REMOVE_STATE => {
                if let Some(id) = proto::removed_group(data) {
                    VOICE.with(|voice| voice.borrow_mut().states.remove(&id));
                }
            }
            proto::ADD_GROUP => {
                if let Some(g) = proto::Group::parse(data) {
                    VOICE.with(|voice| voice.borrow_mut().groups.insert(g.id, g));
                }
            }
            proto::REMOVE_GROUP => {
                if let Some(id) = proto::removed_group(data) {
                    VOICE.with(|voice| voice.borrow_mut().groups.remove(&id));
                }
            }
            proto::JOINED_GROUP => {
                if let Some((group, wrong)) = proto::joined_group(data) {
                    VOICE.with(|voice| {
                        let mut voice = voice.borrow_mut();
                        voice.group = group;
                        voice.wrong_password = wrong;
                    });
                }
            }
            _ => {}
        }
        true
    }

    /// What the voice menu shows, now.
    pub(super) fn voice_view(&self) -> VoiceView {
        let v = &self.settings.extras.pvp.voice;
        let own = self.remote.as_ref().map(|r| r.profile());
        VOICE.with(|voice| {
            let mut voice = voice.borrow_mut();
            if voice.device_list.is_none() {
                voice.device_list = Some(engine::devices());
            }
            let session = voice.session.as_ref();
            let talking: Vec<u128> = session.map(|s| s.shared.talking_players()).unwrap_or_default();
            let status = match session.map(|s| *s.shared.status.lock().unwrap()) {
                None => super::oteria_voice_ui::Status::Unavailable,
                Some(engine::Status::Connecting) => super::oteria_voice_ui::Status::Connecting,
                Some(engine::Status::Connected) => super::oteria_voice_ui::Status::Connected,
                Some(engine::Status::NoAudio) => super::oteria_voice_ui::Status::NoAudio,
            };
            let player = |uuid: u128| super::oteria_voice_ui::Player {
                uuid,
                name: voice.states.get(&uuid).map_or_else(|| "?".into(), |s| s.name.clone()),
                face: voice.faces.get(&uuid).cloned(),
                talking: talking.contains(&uuid)
                    || (Some(uuid) == own
                        && session.is_some_and(|s| s.shared.talking.load(Ordering::Relaxed))),
                sound_off: voice.states.get(&uuid).is_some_and(|s| s.disabled),
                volume: v.volumes.get(&uuid).copied().unwrap_or(100),
            };
            let mut members: Vec<_> = voice
                .states
                .values()
                .filter(|s| voice.group.is_some() && s.group == voice.group)
                .map(|s| player(s.uuid))
                .collect();
            members.sort_by(|a, b| a.name.cmp(&b.name));
            let mut others: Vec<_> = voice
                .states
                .values()
                .filter(|s| Some(s.uuid) != own)
                .map(|s| player(s.uuid))
                .collect();
            others.sort_by(|a, b| a.name.cmp(&b.name));
            let (mics, speakers) = voice.device_list.clone().unwrap_or_default();
            VoiceView {
                status,
                muted: v.muted,
                deafened: v.deafened,
                hide_icons: v.hide_icons,
                groups: voice.groups.values().filter(|g| !g.hidden).cloned().collect(),
                group: voice.group.and_then(|g| voice.groups.get(&g).cloned()),
                wrong_password: voice.wrong_password,
                members,
                others,
                level: session.map_or(-127.0, |s| s.shared.level()),
                loopback: voice.loopback,
                mics,
                speakers,
            }
        })
    }

    /// Something the voice menu or a voice key asked for.
    pub(super) fn voice_action(&mut self, action: VAction) {
        let v = &mut self.settings.extras.pvp.voice;
        let mut save = true;
        match action {
            VAction::ToggleMute => v.muted = !v.muted,
            VAction::ToggleDeafen => {
                v.deafened = !v.deafened;
                let deafened = v.deafened;
                if VOICE.with(|voice| voice.borrow().session.is_some()) {
                    self.send_voice(proto::UPDATE_STATE, proto::update_state(deafened));
                }
            }
            VAction::ToggleIcons => v.hide_icons = !v.hide_icons,
            VAction::Cycle(opt, back) => opt.cycle(&mut self.settings.extras.pvp, back),
            VAction::Set(opt, value) => {
                let v = &mut self.settings.extras.pvp.voice;
                match opt {
                    horizon_client::pvp::Opt::VoiceVolume => v.volume = value.min(200),
                    horizon_client::pvp::Opt::VoiceMic => v.mic = value.min(200),
                    horizon_client::pvp::Opt::VoiceThreshold => v.threshold = value.clamp(20, 70),
                    _ => {}
                }
            }
            VAction::PlayerVolume(uuid, pct) => {
                if pct == 100 {
                    v.volumes.remove(&uuid);
                } else {
                    v.volumes.insert(uuid, pct.min(200));
                }
            }
            VAction::Device { mic, name } => {
                if mic {
                    v.mic_device = name;
                } else {
                    v.speaker_device = name;
                }
                VOICE.with(|voice| voice.borrow_mut().device_list = None);
            }
            VAction::MicTest(on) => {
                save = false;
                VOICE.with(|voice| voice.borrow_mut().loopback = on);
            }
            VAction::CreateGroup { name, password, kind } => {
                save = false;
                let password = (!password.is_empty()).then_some(password.as_str());
                self.send_voice(proto::CREATE_GROUP, proto::create_group(&name, password, kind));
            }
            VAction::JoinGroup(id, password) => {
                save = false;
                self.send_voice(proto::SET_GROUP, proto::set_group(id, password.as_deref()));
            }
            VAction::LeaveGroup => {
                save = false;
                self.send_voice(proto::LEAVE_GROUP, Vec::new());
            }
        }
        if save {
            self.settings.save(&Self::options_path());
        }
    }

    /// The icon by a player's name: talking, their sound off, or not connected to the voice
    /// chat (only for players the server says have it).
    pub(super) fn voice_name_icon(&self, uuid: Option<u128>) -> Option<&'static str> {
        let uuid = uuid?;
        if self.settings.extras.pvp.voice.hide_icons {
            return None;
        }
        VOICE.with(|voice| {
            let voice = voice.borrow();
            let session = voice.session.as_ref()?;
            let state = voice.states.get(&uuid)?;
            if state.disconnected {
                Some(DISCONNECTED)
            } else if state.disabled {
                Some(SOUND_OFF)
            } else if session.shared.talking_players().contains(&uuid) {
                Some(TALKING)
            } else {
                None
            }
        })
    }

    /// The voice chat on the HUD, as Simple Voice Chat shows it: our microphone's state at the
    /// bottom left, and our group's faces down the left, lit while they talk.
    pub(super) fn draw_voice_hud(&self, batch: &mut HudBatch) {
        let v = &self.settings.extras.pvp.voice;
        let (on, talking, status, members) = VOICE.with(|voice| {
            let voice = voice.borrow();
            let Some(session) = voice.session.as_ref() else {
                return (false, false, None, Vec::new());
            };
            let shared = &session.shared;
            let speaking = shared.talking_players();
            let members: Vec<(Option<String>, bool, bool)> = voice
                .states
                .values()
                .filter(|s| voice.group.is_some() && s.group == voice.group)
                .map(|s| (voice.faces.get(&s.uuid).cloned(), speaking.contains(&s.uuid), s.disabled))
                .collect();
            (true, shared.talking.load(Ordering::Relaxed), Some(*shared.status.lock().unwrap()), members)
        });
        if !on {
            return;
        }
        let atlas = &self.gui.atlas;
        let sprite = |name: &str| atlas.sprites.get(icon::small(name).as_str()).copied();
        // Our microphone, bottom left.
        let (name, colour) = if v.deafened {
            (icon::VOLUME_OFF, rgba(255, 90, 90, 255))
        } else if v.muted {
            (icon::MICROPHONE_OFF, rgba(255, 90, 90, 255))
        } else if status == Some(engine::Status::NoAudio) {
            (icon::MICROPHONE_OFF, rgba(255, 180, 80, 255))
        } else if talking {
            (icon::MICROPHONE, rgba(90, 230, 120, 255))
        } else if !v.activation {
            // Push-to-talk, not held: nothing to show, as the mod.
            ("", 0)
        } else {
            (icon::MICROPHONE, rgba(255, 255, 255, 180))
        };
        if !name.is_empty()
            && let Some(r) = sprite(name)
        {
            batch.rect_uv(4.0, batch.height - 20.0, 16.0, 16.0, r, colour);
        }
        if v.hide_icons {
            return;
        }
        // Our group, down the left: faces, outlined while talking.
        let mut draws = Vec::new();
        for (k, (face, speaking, _)) in members.iter().enumerate() {
            let (x, y) = (4.0, 4.0 + k as f32 * 20.0);
            if *speaking {
                batch.fill(atlas, x - 1.0, y - 1.0, 18.0, 18.0, rgba(255, 255, 255, 230));
            }
            let skin = face.as_deref().unwrap_or("entity/player/wide/steve");
            horizon_client::tab_list::face(&mut draws, skin, x as i32, y as i32, 16, true, false, u32::MAX);
        }
        horizon_client::hud_layout::paint(batch, atlas, &draws);
        // Over the faces: who has their sound off.
        for (k, (_, _, off)) in members.iter().enumerate() {
            if *off && let Some(r) = sprite(icon::VOLUME_OFF) {
                batch.rect_uv(13.0, 13.0 + k as f32 * 20.0, 8.0, 8.0, r, rgba(255, 90, 90, 255));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_name_icons_are_atlas_sprites() {
        use horizon_client::oteria_icons as icon;
        for (sprite, of) in [
            (super::TALKING, icon::VOLUME),
            (super::SOUND_OFF, icon::VOLUME_OFF),
            (super::DISCONNECTED, icon::WIFI_OFF),
        ] {
            assert_eq!(sprite, icon::small(of));
        }
    }
}
