//! The voice chat's running half, off the game thread: one thread on the voice server's UDP
//! socket (it authenticates, answers keep-alives and pings, encodes our microphone and decodes
//! the others' voices), the microphone's input stream and the voices' output stream (cpal).
//!
//! The game thread only writes what it knows (where the listener and the players are, whether
//! push-to-talk is held, the settings) into `Shared`, and reads who is talking back.

use super::proto::{self, FRAME, Packet, RATE, Secret};
use std::collections::{HashMap, VecDeque};
use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long another's voice shows as talking after their last packet.
const TALK_SHOWN: Duration = Duration::from_millis(300);
/// Voice activation keeps sending this long after the voice drops under the level.
const HANG: Duration = Duration::from_millis(500);
/// A voice starts playing once this much of it is buffered (smooths the network's jitter), and
/// never holds more than `MAX_QUEUED` (it drops the oldest).
const PRIME: usize = FRAME * 2;
const MAX_QUEUED: usize = RATE as usize / 2;
/// Opus's bitrate for our voice (Simple Voice Chat's VoIP codec is about this).
const BITRATE: i32 = 24_000;

/// What the game sets, read by the voice thread.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Voice activation (else push-to-talk).
    pub activation: bool,
    /// Voice activation's level, in dB below full scale.
    pub threshold: f32,
    /// The microphone's and the voices' gains (1 is as they are).
    pub mic: f32,
    pub volume: f32,
    /// Our microphone sends nothing.
    pub muted: bool,
    /// The others aren't heard (Simple Voice Chat's "disabled").
    pub deafened: bool,
    /// The microphone plays back to us (the settings' test).
    pub loopback: bool,
    /// Voices placed where the players are (else all in the middle).
    pub spatial: bool,
    /// The background noise taken out of our microphone.
    pub noise: bool,
}

/// The connection's state, for the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    /// The microphone or the speakers couldn't open; the connection goes on.
    NoAudio,
}

#[derive(Default)]
pub struct World {
    /// The listener's eyes and yaw (degrees, Minecraft's).
    pub listener: ([f64; 3], f32),
    /// Where each player's voice comes from.
    pub players: HashMap<u128, [f64; 3]>,
    pub push_to_talk: bool,
    pub settings: Option<Settings>,
    /// Each player's own volume (1 is as they are).
    pub volumes: HashMap<u128, f32>,
}

/// What both sides share.
pub struct Shared {
    pub world: Mutex<World>,
    /// Who was heard last, and when.
    pub heard: Mutex<HashMap<u128, Instant>>,
    pub talking: AtomicBool,
    /// Our microphone's last level, in dB (an `f32`'s bits), for the settings' meter.
    level: AtomicU32,
    pub status: Mutex<Status>,
    stop: AtomicBool,
    /// Our microphone at 48 kHz mono, waiting to be encoded.
    mic: Mutex<VecDeque<f32>>,
    mixer: Mutex<Mixer>,
}

impl Shared {
    /// Our microphone's level, in dB below full scale (-127 is silence).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
    /// The players talking now (heard within `TALK_SHOWN`).
    pub fn talking_players(&self) -> Vec<u128> {
        let now = Instant::now();
        let mut heard = self.heard.lock().unwrap();
        heard.retain(|_, at| now.duration_since(*at) < Duration::from_secs(5));
        let mut out: Vec<u128> = heard
            .iter()
            .filter(|(_, at)| now.duration_since(**at) < TALK_SHOWN)
            .map(|(uuid, _)| *uuid)
            .collect();
        out.sort_unstable();
        out
    }
}

/// The voices, each a queue of stereo frames at 48 kHz, mixed as the output pulls them.
#[derive(Default)]
struct Mixer {
    voices: HashMap<u128, Voice>,
}
#[derive(Default)]
struct Voice {
    frames: VecDeque<[f32; 2]>,
    /// Playing (else filling up to `PRIME` first).
    primed: bool,
}
impl Mixer {
    fn push(&mut self, who: u128, mono: &[f32], [left, right]: [f32; 2]) {
        let v = self.voices.entry(who).or_default();
        v.frames.extend(mono.iter().map(|s| [s * left, s * right]));
        while v.frames.len() > MAX_QUEUED {
            v.frames.pop_front();
        }
        if v.frames.len() >= PRIME {
            v.primed = true;
        }
    }
    /// The next frame of every playing voice, added up and softly limited.
    fn next(&mut self) -> [f32; 2] {
        let mut out = [0.0f32; 2];
        for v in self.voices.values_mut() {
            if !v.primed {
                continue;
            }
            match v.frames.pop_front() {
                Some([l, r]) => {
                    out[0] += l;
                    out[1] += r;
                }
                // Ran dry: wait for a buffer's worth again.
                None => v.primed = false,
            }
        }
        out.map(|s| s.tanh())
    }
}

/// A rate converter that carries its phase from one block to the next (linear: voice needs no
/// better).
struct Resampler {
    step: f64,
    t: f64,
    prev: [f32; 2],
    next: [f32; 2],
}
impl Resampler {
    fn new(from: u32, to: u32) -> Self {
        Self {
            step: f64::from(from) / f64::from(to),
            t: 0.0,
            prev: [0.0; 2],
            next: [0.0; 2],
        }
    }
    /// One output frame, pulling input frames from `source` as needed.
    fn frame(&mut self, mut source: impl FnMut() -> [f32; 2]) -> [f32; 2] {
        while self.t >= 1.0 {
            self.prev = self.next;
            self.next = source();
            self.t -= 1.0;
        }
        let t = self.t as f32;
        self.t += self.step;
        [0, 1].map(|c| self.prev[c] + (self.next[c] - self.prev[c]) * t)
    }
}

/// The microphone's converter: input samples pushed one by one, 48 kHz samples out.
struct Converter {
    /// Input samples per output sample.
    step: f64,
    t: f64,
    prev: f32,
}
impl Converter {
    fn new(from: u32, to: u32) -> Self {
        Self { step: f64::from(from) / f64::from(to), t: 0.0, prev: 0.0 }
    }
    fn push(&mut self, x: f32, out: &mut Vec<f32>) {
        while self.t < 1.0 {
            out.push(self.prev + (x - self.prev) * self.t as f32);
            self.t += self.step;
        }
        self.t -= 1.0;
        self.prev = x;
    }
}

/// The microphones and the speakers there are, by name.
pub fn devices() -> (Vec<String>, Vec<String>) {
    #[cfg(feature = "audio")]
    {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let inputs = host
            .input_devices()
            .map(|l| l.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default();
        let outputs = host
            .output_devices()
            .map(|l| l.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default();
        (inputs, outputs)
    }
    #[cfg(not(feature = "audio"))]
    {
        (Vec::new(), Vec::new())
    }
}

/// A noise gate that follows the room: it learns the background's level and lets through what
/// rises well above it, easing in and out so words aren't cut.
struct Gate {
    floor: f32,
    gain: f32,
}
impl Default for Gate {
    fn default() -> Self {
        Self { floor: 0.01, gain: 1.0 }
    }
}
impl Gate {
    fn apply(&mut self, frame: &mut [f32]) {
        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len().max(1) as f32).sqrt();
        // The floor falls at once to a quieter frame and rises slowly.
        self.floor = if rms < self.floor { rms.max(1e-5) } else { self.floor * 1.002 + 1e-6 };
        let open = ((rms / (self.floor * 3.0)) - 1.0).clamp(0.0, 1.0);
        let start = self.gain;
        self.gain += (open - self.gain) * if open > self.gain { 0.6 } else { 0.15 };
        let n = frame.len() as f32;
        for (i, s) in frame.iter_mut().enumerate() {
            *s *= start + (self.gain - start) * i as f32 / n;
        }
    }
}

/// A running voice connection; dropping it stops everything.
pub struct Session {
    pub shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
    #[cfg(feature = "audio")]
    _streams: Vec<cpal::Stream>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Session {
    /// Connects to the voice server at `host`:`port` with `secret`, and opens the audio on the
    /// named microphone and speakers (empty: the system's).
    pub fn start(host: String, port: u16, secret: Secret, mic: &str, speaker: &str) -> Self {
        let shared = Arc::new(Shared {
            world: Mutex::new(World::default()),
            heard: Mutex::new(HashMap::new()),
            talking: AtomicBool::new(false),
            level: AtomicU32::new((-127.0f32).to_bits()),
            status: Mutex::new(Status::Connecting),
            stop: AtomicBool::new(false),
            mic: Mutex::new(VecDeque::new()),
            mixer: Mutex::new(Mixer::default()),
        });
        #[cfg(feature = "audio")]
        let streams = match audio::open(&shared, mic, speaker) {
            Ok(streams) => streams,
            Err(e) => {
                log::warn!("Oteria voice: no audio ({e})");
                *shared.status.lock().unwrap() = Status::NoAudio;
                Vec::new()
            }
        };
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("oteria-voice".into())
                .spawn(move || {
                    if let Err(e) = run(&shared, &host, port, &secret) {
                        log::warn!("Oteria voice: the connection ended: {e}");
                    }
                })
                .ok()
        };
        Self {
            shared,
            thread,
            #[cfg(feature = "audio")]
            _streams: streams,
        }
    }

    /// The audio opened again on other devices (the settings' choice).
    pub fn reopen_audio(&mut self, mic: &str, speaker: &str) {
        #[cfg(feature = "audio")]
        {
            self._streams.clear();
            match audio::open(&self.shared, mic, speaker) {
                Ok(streams) => {
                    self._streams = streams;
                    let mut status = self.shared.status.lock().unwrap();
                    if *status == Status::NoAudio {
                        *status = Status::Connecting;
                    }
                }
                Err(e) => {
                    log::warn!("Oteria voice: no audio ({e})");
                    *self.shared.status.lock().unwrap() = Status::NoAudio;
                }
            }
        }
        #[cfg(not(feature = "audio"))]
        let _ = (mic, speaker);
    }
}

/// The left and right gains of a voice from `at`, heard at `listener` facing `yaw`, fading out
/// to nothing at `distance` blocks; `None` (a group, or a player not in view) plays centred.
pub fn place(at: Option<[f64; 3]>, listener: [f64; 3], yaw: f32, distance: f32) -> [f32; 2] {
    let Some(at) = at else {
        return [std::f32::consts::FRAC_1_SQRT_2; 2];
    };
    let d = [at[0] - listener[0], at[1] - listener[1], at[2] - listener[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() as f32;
    let gain = if distance > 0.0 { (1.0 - len / distance).clamp(0.0, 1.0) } else { 1.0 };
    // Facing south (yaw 0) the west (-x) is on the right.
    let yaw = yaw.to_radians();
    let right = [-yaw.cos(), -yaw.sin()];
    let flat = (d[0] * d[0] + d[2] * d[2]).sqrt() as f32;
    let pan = if flat > 0.01 { (d[0] as f32 * right[0] + d[2] as f32 * right[1]) / flat } else { 0.0 };
    // Equal power, never fully in one ear.
    let angle = (pan.clamp(-1.0, 1.0) * 0.8 + 1.0) * std::f32::consts::FRAC_PI_4;
    [angle.cos() * gain, angle.sin() * gain]
}

/// The voice thread: the socket, the handshake, and audio both ways until told to stop.
fn run(shared: &Shared, host: &str, port: u16, secret: &Secret) -> std::io::Result<()> {
    let address = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other("no address for the voice server"))?;
    let bind = if address.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
    let socket = UdpSocket::bind(bind)?;
    socket.connect(address)?;
    socket.set_read_timeout(Some(Duration::from_millis(5)))?;
    log::info!("Oteria voice: connecting to the voice server at {address}");
    let (player, key) = (secret.player, secret.secret);
    let send = |p: &Packet| {
        if let Some(datagram) = proto::to_server(player, &key, p) {
            let _ = socket.send(&datagram);
        }
    };
    let (mut authenticated, mut connected) = (false, false);
    let mut last_try = Instant::now() - Duration::from_secs(2);
    let mut last_keep_alive = Instant::now();
    let keep_alive = Duration::from_millis(u64::try_from(secret.keep_alive.max(100)).unwrap_or(1000));
    let mut encoder = new_encoder();
    let mut decoders: HashMap<u128, opus_pure::OpusDecoder> = HashMap::new();
    let (mut sequence, mut was_talking, mut loud_until) = (0i64, false, Instant::now());
    let mut buf = [0u8; 4096];
    let mut pcm = vec![0f32; FRAME * 6];
    let mut packet = vec![0u8; proto::MAX_OPUS];
    let mut gate = Gate::default();
    while !shared.stop.load(Ordering::Relaxed) {
        // The handshake, once a second until it holds.
        if !connected && last_try.elapsed() >= Duration::from_secs(1) {
            last_try = Instant::now();
            if authenticated {
                send(&Packet::ConnectionCheck);
            } else {
                send(&Packet::Authenticate { player, secret: key });
            }
        }
        // The server went quiet: start over.
        if connected && last_keep_alive.elapsed() > keep_alive * 10 {
            log::info!("Oteria voice: the voice server timed out; connecting again");
            (authenticated, connected) = (false, false);
            *shared.status.lock().unwrap() = Status::Connecting;
        }
        match socket.recv(&mut buf) {
            Ok(n) => {
                let Some(p) = proto::from_server(&key, &buf[..n]) else {
                    continue;
                };
                match p {
                    Packet::AuthenticateAck => authenticated = true,
                    Packet::ConnectionCheckAck if authenticated => {
                        if !connected {
                            log::info!("Oteria voice: connected");
                        }
                        connected = true;
                        last_keep_alive = Instant::now();
                        let mut status = shared.status.lock().unwrap();
                        if *status == Status::Connecting {
                            *status = Status::Connected;
                        }
                    }
                    Packet::KeepAlive => {
                        last_keep_alive = Instant::now();
                        send(&Packet::KeepAlive);
                    }
                    ping @ Packet::Ping { .. } => send(&ping),
                    Packet::Player { sender, data, distance, .. } => {
                        hear(shared, &mut decoders, &mut pcm, sender, &data, Some(distance));
                    }
                    Packet::Group { sender, data, .. } => {
                        hear(shared, &mut decoders, &mut pcm, sender, &data, None);
                    }
                    Packet::Location { sender, data, at, distance, .. } => {
                        // A voice from a place: placed there, not at its sender.
                        shared.world.lock().unwrap().players.insert(sender, at);
                        hear(shared, &mut decoders, &mut pcm, sender, &data, Some(distance));
                    }
                    _ => {}
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            // A port nothing listens on yet (ICMP back on Windows): try again later.
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }

        // Our microphone, 20 ms at a time.
        let (settings, push) = {
            let w = shared.world.lock().unwrap();
            (w.settings.clone(), w.push_to_talk)
        };
        let Some(settings) = settings else { continue };
        loop {
            let mut frame: Vec<f32> = {
                let mut mic = shared.mic.lock().unwrap();
                if mic.len() < FRAME {
                    break;
                }
                mic.drain(..FRAME).map(|s| (s * settings.mic).clamp(-1.0, 1.0)).collect()
            };
            if settings.noise {
                gate.apply(&mut frame);
            }
            let rms = (frame.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt();
            let db = (20.0 * rms.max(1e-9).log10()).max(-127.0);
            shared.level.store(db.to_bits(), Ordering::Relaxed);
            if settings.loopback {
                // The test: ourselves, in the middle.
                let g = std::f32::consts::FRAC_1_SQRT_2 * settings.volume;
                shared.mixer.lock().unwrap().push(0, &frame, [g, g]);
            }
            let talking = !settings.muted
                && if settings.activation {
                    if db > -settings.threshold {
                        loud_until = Instant::now() + HANG;
                    }
                    Instant::now() < loud_until
                } else {
                    push
                };
            shared.talking.store(talking && connected, Ordering::Relaxed);
            if talking && connected {
                if let Ok(n) = encoder.encode(&frame, FRAME, &mut packet) {
                    send(&Packet::Mic { data: packet[..n].to_vec(), sequence, whispering: false });
                    sequence += 1;
                }
                was_talking = true;
            } else if was_talking {
                // An empty frame: we stopped (the others' buffers end here).
                send(&Packet::Mic { data: Vec::new(), sequence, whispering: false });
                sequence += 1;
                was_talking = false;
                encoder = new_encoder();
            }
        }
    }
    if was_talking {
        send(&Packet::Mic { data: Vec::new(), sequence, whispering: false });
    }
    Ok(())
}

fn new_encoder() -> opus_pure::OpusEncoder {
    let mut e = opus_pure::OpusEncoder::new(RATE as i32, 1, opus_pure::Application::Voip)
        .expect("a 48 kHz mono Opus encoder");
    e.bitrate_bps = BITRATE;
    e
}

/// Another player's frame: decoded, placed and queued (an empty one ends what they said).
fn hear(
    shared: &Shared,
    decoders: &mut HashMap<u128, opus_pure::OpusDecoder>,
    pcm: &mut [f32],
    sender: u128,
    data: &[u8],
    distance: Option<f32>,
) {
    if data.is_empty() {
        decoders.remove(&sender);
        return;
    }
    let deafened = shared.world.lock().unwrap().settings.as_ref().is_some_and(|s| s.deafened);
    if deafened {
        return;
    }
    shared.heard.lock().unwrap().insert(sender, Instant::now());
    let decoder = match decoders.entry(sender) {
        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
        std::collections::hash_map::Entry::Vacant(e) => match opus_pure::OpusDecoder::new(RATE as i32, 1) {
            Ok(d) => e.insert(d),
            Err(_) => return,
        },
    };
    let Ok(n) = decoder.decode(data, pcm.len(), pcm) else {
        return;
    };
    let gains = {
        let w = shared.world.lock().unwrap();
        let volume = w.settings.as_ref().map_or(1.0, |s| s.volume)
            * w.volumes.get(&sender).copied().unwrap_or(1.0);
        let spatial = w.settings.as_ref().is_none_or(|s| s.spatial);
        let (listener, yaw) = w.listener;
        let at = distance.filter(|_| spatial).and_then(|_| w.players.get(&sender).copied());
        place(at, listener, yaw, distance.unwrap_or(0.0)).map(|g| g * volume)
    };
    shared.mixer.lock().unwrap().push(sender, &pcm[..n], gains);
}

#[cfg(feature = "audio")]
mod audio {
    //! The microphone and the speakers, through cpal: 48 kHz when the device offers it,
    //! converted otherwise.
    use super::{Converter, Resampler, Shared};
    use crate::game::oteria_voice::proto::RATE;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::Arc;

    /// A config at 48 kHz if the device has one, else its own.
    fn pick(
        configs: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
        default: cpal::SupportedStreamConfig,
    ) -> cpal::SupportedStreamConfig {
        let want = cpal::SampleRate(RATE);
        configs
            .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
            .find(|c| c.min_sample_rate() <= want && want <= c.max_sample_rate())
            .map(|c| c.with_sample_rate(want))
            .unwrap_or(default)
    }

    /// The device named `name` among `list`, else the default one.
    fn named(
        list: Result<impl Iterator<Item = cpal::Device>, cpal::DevicesError>,
        name: &str,
        default: Option<cpal::Device>,
    ) -> Option<cpal::Device> {
        if !name.is_empty()
            && let Ok(mut list) = list
            && let Some(d) = list.find(|d| d.name().is_ok_and(|n| n == name))
        {
            return Some(d);
        }
        default
    }

    pub(super) fn open(shared: &Arc<Shared>, mic: &str, speaker: &str) -> Result<Vec<cpal::Stream>, String> {
        let host = cpal::default_host();
        let mut streams = Vec::new();
        let err = |e: cpal::StreamError| log::warn!("Oteria voice: audio stream: {e}");

        // The speakers: the mixed voices.
        let out = named(host.output_devices(), speaker, host.default_output_device()).ok_or("no output device")?;
        let default = out.default_output_config().map_err(|e| e.to_string())?;
        let config = pick(out.supported_output_configs().map_err(|e| e.to_string())?, default);
        if config.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!("output samples are {:?}", config.sample_format()));
        }
        let channels = usize::from(config.channels());
        let mut resampler = Resampler::new(RATE, config.sample_rate().0);
        let mixer = shared.clone();
        let stream = out
            .build_output_stream(
                &config.config(),
                move |data: &mut [f32], _| {
                    let mut mixer = mixer.mixer.lock().unwrap();
                    for frame in data.chunks_mut(channels) {
                        let [l, r] = resampler.frame(|| mixer.next());
                        for (c, s) in frame.iter_mut().enumerate() {
                            *s = match c {
                                0 => l,
                                1 => r,
                                _ => 0.0,
                            };
                        }
                        if channels == 1 {
                            frame[0] = (l + r) / 2.0;
                        }
                    }
                },
                err,
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        streams.push(stream);

        // The microphone: mono at 48 kHz into `mic`.
        let input = named(host.input_devices(), mic, host.default_input_device()).ok_or("no microphone")?;
        let default = input.default_input_config().map_err(|e| e.to_string())?;
        let config = pick(input.supported_input_configs().map_err(|e| e.to_string())?, default);
        if config.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!("microphone samples are {:?}", config.sample_format()));
        }
        let channels = usize::from(config.channels());
        let mut converter = Converter::new(config.sample_rate().0, RATE);
        let mic = shared.clone();
        let stream = input
            .build_input_stream(
                &config.config(),
                move |data: &[f32], _| {
                    let mut mono = Vec::with_capacity(data.len() / channels + 2);
                    for f in data.chunks(channels) {
                        converter.push(f.iter().sum::<f32>() / channels as f32, &mut mono);
                    }
                    let mut queue = mic.mic.lock().unwrap();
                    queue.extend(mono);
                    // A second at most (nothing reads it while not connected).
                    let len = queue.len();
                    if len > RATE as usize {
                        queue.drain(..len - RATE as usize);
                    }
                },
                err,
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        streams.push(stream);
        Ok(streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voices_come_from_where_the_players_are() {
        // Facing south (+z): a player to the west (-x) is on the right.
        let [l, r] = place(Some([-5.0, 0.0, 0.0]), [0.0; 3], 0.0, 48.0);
        assert!(r > l, "{l} {r}");
        // In front: both ears alike; past the distance: silent.
        let [l, r] = place(Some([0.0, 0.0, 5.0]), [0.0; 3], 0.0, 48.0);
        assert!((l - r).abs() < 1e-5);
        assert_eq!(place(Some([0.0, 0.0, 60.0]), [0.0; 3], 0.0, 48.0), [0.0, 0.0]);
        // Turned to face west (yaw 90), the same player is in front.
        let [l, r] = place(Some([-5.0, 0.0, 0.0]), [0.0; 3], 90.0, 48.0);
        assert!((l - r).abs() < 1e-4, "{l} {r}");
    }

    #[test]
    fn a_voice_plays_once_buffered_and_waits_again_when_dry() {
        let mut m = Mixer::default();
        m.push(1, &[0.5; FRAME], [1.0, 1.0]);
        assert_eq!(m.next(), [0.0, 0.0], "one frame is not enough yet");
        m.push(1, &[0.5; FRAME], [1.0, 1.0]);
        assert!(m.next()[0] > 0.4);
        for _ in 0..PRIME {
            m.next();
        }
        assert!(!m.voices[&1].primed);
    }

    #[test]
    fn the_gate_quiets_the_background_and_lets_speech_through() {
        let mut g = Gate::default();
        let hum = |k: usize| ((k as f32) * 0.3).sin() * 0.002;
        let mut last = Vec::new();
        for _ in 0..200 {
            let mut f: Vec<f32> = (0..FRAME).map(hum).collect();
            g.apply(&mut f);
            last = f;
        }
        let quiet = last.iter().map(|s| s.abs()).fold(0.0, f32::max);
        assert!(quiet < 0.001, "the hum is gated: {quiet}");
        let mut speech: Vec<f32> = (0..FRAME).map(|k| ((k as f32) * 0.05).sin() * 0.3).collect();
        for _ in 0..3 {
            speech = (0..FRAME).map(|k| ((k as f32) * 0.05).sin() * 0.3).collect();
            g.apply(&mut speech);
        }
        assert!(speech.iter().map(|s| s.abs()).fold(0.0, f32::max) > 0.25, "speech passes");
    }

    #[test]
    fn the_resampler_keeps_the_rate_ratio() {
        let mut r = Resampler::new(48_000, 44_100);
        let mut pulled = 0;
        for _ in 0..44_100 {
            r.frame(|| {
                pulled += 1;
                [0.0; 2]
            });
        }
        assert!((pulled as i32 - 48_000).abs() <= 2, "{pulled}");
    }

    #[test]
    fn the_microphone_converter_keeps_the_rate_ratio() {
        let mut c = Converter::new(44_100, 48_000);
        let mut out = Vec::new();
        for _ in 0..44_100 {
            c.push(0.25, &mut out);
        }
        assert!((out.len() as i32 - 48_000).abs() <= 2, "{}", out.len());
    }

    #[test]
    fn opus_round_trips_a_tone() {
        let mut e = new_encoder();
        let mut d = opus_pure::OpusDecoder::new(RATE as i32, 1).unwrap();
        let mut packet = vec![0u8; proto::MAX_OPUS];
        let mut out = vec![0f32; FRAME];
        let mut energy = 0.0;
        for k in 0..10 {
            let tone: Vec<f32> = (0..FRAME)
                .map(|i| ((k * FRAME + i) as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin() * 0.5)
                .collect();
            let n = e.encode(&tone, FRAME, &mut packet).unwrap();
            assert!(n > 0 && n <= proto::MAX_OPUS);
            let got = d.decode(&packet[..n], FRAME, &mut out).unwrap();
            assert_eq!(got, FRAME);
            energy = out.iter().map(|s| s * s).sum::<f32>() / FRAME as f32;
        }
        assert!(energy > 0.05, "the tone comes back: {energy}");
    }
}
