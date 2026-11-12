//! Oteria's voice chat in the game: Simple Voice Chat's protocol (`proto`), so a player on this
//! client and players on Fabric (or any client with that mod) hear each other on a server that
//! runs it (its Fabric mod or its Paper plugin). Push-to-talk or voice activation, voices placed
//! where the players are, and a list of who is talking on the HUD.
//!
//! On a server, once joined with the Voice chat mod on: the voice channels are registered and
//! the secret asked for; the server's answer starts the voice connection (`engine`). Nothing is
//! sent to a server that doesn't run it (it never answers).

mod engine;
mod proto;

use super::{Game, Screen};
use horizon_client::hud::{HudBatch, rgba};
use horizon_protocol::packets::{configuration::CustomPayload, play::Serverbound as S};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

#[derive(Default)]
struct Voice {
    /// The connection (`Session::serial`) the secret was asked on.
    asked: Option<u64>,
    session: Option<engine::Session>,
    /// Player names from the server's states, for the HUD.
    names: HashMap<u128, String>,
}

thread_local! {
    static VOICE: RefCell<Voice> = RefCell::new(Voice::default());
}

impl Game<'_> {
    /// Once a frame: asks for the secret on a new server, stops on leaving or turning it off,
    /// and tells the voice thread where everyone is and whether push-to-talk is held.
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
        let held = self.screen == Screen::Playing
            && self.physical_keys.iter().any(|k| {
                horizon_client::keybinds::key_name(*k) == Some(v.key.as_str())
            });
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
        VOICE.with(|voice| {
            let voice = voice.borrow();
            let Some(session) = voice.session.as_ref() else {
                return;
            };
            let mut w = session.shared.world.lock().unwrap();
            if let Some(listener) = listener {
                w.listener = (listener, yaw);
            }
            // Places sent by the server for a location's voice stay until a player has it.
            w.players.extend(players);
            w.push_to_talk = held;
            w.settings = Some(engine::Settings {
                activation: v.activation,
                threshold: f32::from(v.threshold),
                mic: f32::from(v.mic) / 100.0,
                volume: f32::from(v.volume) / 100.0,
            });
        });
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
                let session = engine::Session::start(host, port, secret);
                VOICE.with(|voice| voice.borrow_mut().session = Some(session));
                // Our voice chat is on: the others see it.
                self.send_voice(proto::UPDATE_STATE, proto::update_state(false));
            }
            proto::STATES => {
                if let Some(states) = proto::PlayerState::read_all(data) {
                    VOICE.with(|voice| {
                        let mut voice = voice.borrow_mut();
                        for s in states {
                            voice.names.insert(s.uuid, s.name);
                        }
                    });
                }
            }
            proto::STATE => {
                if let Some(s) = proto::PlayerState::read(&mut proto::Reader::new(data)) {
                    VOICE.with(|voice| voice.borrow_mut().names.insert(s.uuid, s.name));
                }
            }
            _ => {}
        }
        true
    }

    /// Who is talking, at the left of the screen: our own microphone first, then the others.
    pub(super) fn draw_voice_hud(&self, batch: &mut HudBatch) {
        let (own, others, status) = VOICE.with(|voice| {
            let voice = voice.borrow();
            let Some(session) = voice.session.as_ref() else {
                return (false, Vec::new(), None);
            };
            let shared = &session.shared;
            let others: Vec<String> = shared
                .talking_players()
                .into_iter()
                .map(|uuid| voice.names.get(&uuid).cloned().unwrap_or_else(|| "?".into()))
                .collect();
            (shared.talking.load(Ordering::Relaxed), others, Some(*shared.status.lock().unwrap()))
        });
        let Some(status) = status else {
            return;
        };
        let atlas = &self.gui.atlas;
        let mic = horizon_client::oteria_icons::small(horizon_client::oteria_icons::MICROPHONE);
        let (x, mut y) = (4.0, batch.height / 2.0 - 20.0);
        let mut line = |batch: &mut HudBatch, text: &str, colour: u32| {
            let w = HudBatch::text_width(atlas, text) + 16.0;
            batch.fill(atlas, x, y, w, 12.0, rgba(0, 0, 0, 110));
            if let Some(r) = atlas.sprites.get(mic.as_str()).copied() {
                batch.rect_uv(x + 2.0, y + 2.0, 8.0, 8.0, r, colour);
            }
            batch.text(atlas, text, x + 13.0, y + 2.0, colour, true);
            y += 14.0;
        };
        if status == engine::Status::NoAudio {
            line(batch, "Voice: no microphone", rgba(255, 120, 120, 255));
        }
        if own {
            let name = self.player_name();
            line(batch, &name, rgba(120, 230, 140, 255));
        }
        for name in others {
            line(batch, &name, rgba(255, 255, 255, 255));
        }
    }

    fn player_name(&self) -> String {
        self.remote
            .as_ref()
            .map(|r| r.session.name.clone())
            .unwrap_or_default()
    }
}
