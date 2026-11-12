//! Simple Voice Chat's wire format, as its Fabric and Paper versions speak it (protocol 20):
//! the plugin channel messages over the game's connection, and the UDP packets of the voice
//! server, AES-128-GCM sealed with the secret the server hands out.
//!
//! Plugin channels (`voicechat:*`, Minecraft's `FriendlyByteBuf` layout: big-endian numbers,
//! VarInt-prefixed byte arrays and UTF-8 strings, a UUID as two longs):
//! - `request_secret` (to the server): the compatibility version, an int.
//! - `secret` (from the server): the 16-byte secret, the voice port (int), the player's UUID,
//!   the codec (byte), the MTU (int), the voice distance (double), the keep-alive period in
//!   milliseconds (int), groups enabled (bool), the voice host (string) and recording allowed
//!   (bool).
//! - `update_state` (to the server): whether this player's voice chat is disabled.
//! - `state`, `states`, `remove_state` (from the server): who is connected or disabled.
//!
//! UDP: to the server `0xFF`, the player's UUID, then the sealed message as a byte array; from
//! it `0xFF` and the sealed message. A sealed message is a 12-byte nonce, then the ciphertext
//! and its 16-byte tag of: the packet's type (a byte) and its fields.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};

/// The protocol version this client speaks (`BuildConstants.COMPATIBILITY_VERSION`).
pub const COMPATIBILITY: i32 = 20;
/// The plugin channels, as the server knows them.
pub const REQUEST_SECRET: &str = "voicechat:request_secret";
pub const SECRET: &str = "voicechat:secret";
pub const UPDATE_STATE: &str = "voicechat:update_state";
pub const STATE: &str = "voicechat:state";
pub const STATES: &str = "voicechat:states";
pub const REMOVE_STATE: &str = "voicechat:remove_state";
/// Every channel this client takes or sends, for `minecraft:register`.
pub const CHANNELS: [&str; 6] = [REQUEST_SECRET, SECRET, UPDATE_STATE, STATE, STATES, REMOVE_STATE];
/// The longest Opus frame a packet may carry (`readByteArray(1275)`).
pub const MAX_OPUS: usize = 1275;
/// The audio's rate and its frames: 20 ms of mono at 48 kHz.
pub const RATE: u32 = 48_000;
pub const FRAME: usize = 960;

/// A reader over a message, Minecraft's `FriendlyByteBuf` way.
pub struct Reader<'a> {
    data: &'a [u8],
}
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.data.len() < n {
            return None;
        }
        let (head, rest) = self.data.split_at(n);
        self.data = rest;
        Some(head)
    }
    pub fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Option<bool> {
        Some(self.u8()? != 0)
    }
    pub fn i32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
    pub fn i64(&mut self) -> Option<i64> {
        Some(i64::from_be_bytes(self.take(8)?.try_into().ok()?))
    }
    pub fn f32(&mut self) -> Option<f32> {
        Some(f32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
    pub fn f64(&mut self) -> Option<f64> {
        Some(f64::from_be_bytes(self.take(8)?.try_into().ok()?))
    }
    pub fn uuid(&mut self) -> Option<u128> {
        Some(u128::from_be_bytes(self.take(16)?.try_into().ok()?))
    }
    pub fn var_int(&mut self) -> Option<i32> {
        let mut value = 0u32;
        for k in 0..5 {
            let b = self.u8()?;
            value |= u32::from(b & 0x7F) << (7 * k);
            if b & 0x80 == 0 {
                return Some(value as i32);
            }
        }
        None
    }
    /// A VarInt-prefixed byte array of at most `max` bytes.
    pub fn bytes(&mut self, max: usize) -> Option<&'a [u8]> {
        let n = usize::try_from(self.var_int()?).ok()?;
        if n > max {
            return None;
        }
        self.take(n)
    }
    pub fn string(&mut self, max_chars: usize) -> Option<String> {
        let raw = self.bytes(max_chars * 3)?;
        String::from_utf8(raw.to_vec()).ok()
    }
}

/// A writer, the same way.
#[derive(Default)]
pub struct Writer {
    pub out: Vec<u8>,
}
impl Writer {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.out.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.out.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.out.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn uuid(&mut self, v: u128) -> &mut Self {
        self.out.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn var_int(&mut self, v: i32) -> &mut Self {
        let mut v = v as u32;
        loop {
            if v & !0x7F == 0 {
                self.out.push(v as u8);
                return self;
            }
            self.out.push((v as u8 & 0x7F) | 0x80);
            v >>= 7;
        }
    }
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.var_int(v.len() as i32);
        self.out.extend_from_slice(v);
        self
    }
}

/// What the server tells a client it may join with (`voicechat:secret`).
#[derive(Clone, Debug, PartialEq)]
pub struct Secret {
    pub secret: [u8; 16],
    pub port: u16,
    pub player: u128,
    pub codec: u8,
    pub mtu: i32,
    pub distance: f64,
    /// Milliseconds between the server's keep-alives.
    pub keep_alive: i32,
    pub groups: bool,
    /// Where the voice server is when not where the game is: empty, a port, or `host[:port]`.
    pub host: String,
    pub recording: bool,
}
impl Secret {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let mut r = Reader::new(data);
        let secret = r.take(16)?.try_into().ok()?;
        let port = u16::try_from(r.i32()?).ok()?;
        Some(Self {
            secret,
            port,
            player: r.uuid()?,
            codec: r.u8()?,
            mtu: r.i32()?,
            distance: r.f64()?,
            keep_alive: r.i32()?,
            groups: r.bool()?,
            host: r.string(32_767)?,
            recording: r.bool()?,
        })
    }
    /// The voice server's host and port, from the game server's host (`InitializationData`'s
    /// `parseAddress`): the game's host and the secret's port, unless the server names another
    /// port or host.
    pub fn address(&self, game_host: &str) -> (String, u16) {
        let host = self.host.trim();
        if host.is_empty() {
            return (game_host.to_owned(), self.port);
        }
        if let Ok(port) = host.parse::<u16>() {
            return (game_host.to_owned(), port);
        }
        match split_host_port(host) {
            (h, Some(p)) => (h.to_owned(), p),
            (h, None) => (h.to_owned(), self.port),
        }
    }
}

/// `host[:port]` (an IPv6 address in brackets) apart.
pub fn split_host_port(address: &str) -> (&str, Option<u16>) {
    if let Some(rest) = address.strip_prefix('[')
        && let Some((host, tail)) = rest.split_once(']')
    {
        return (host, tail.strip_prefix(':').and_then(|p| p.parse().ok()));
    }
    match address.rsplit_once(':') {
        // One colon: host and port; more is a bare IPv6 address.
        Some((host, port)) if !host.contains(':') => (host, port.parse().ok()),
        _ => (address, None),
    }
}

/// A player's voice chat state (`voicechat:state(s)`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub disabled: bool,
    pub disconnected: bool,
    pub uuid: u128,
    pub name: String,
    pub group: Option<u128>,
}
impl PlayerState {
    pub fn read(r: &mut Reader) -> Option<Self> {
        let disabled = r.bool()?;
        let disconnected = r.bool()?;
        let uuid = r.uuid()?;
        let name = r.string(32_767)?;
        let group = if r.bool()? { Some(r.uuid()?) } else { None };
        Some(Self {
            disabled,
            disconnected,
            uuid,
            name,
            group,
        })
    }
    /// A `voicechat:states` message: an int count, then the states.
    pub fn read_all(data: &[u8]) -> Option<Vec<Self>> {
        let mut r = Reader::new(data);
        let n = r.i32()?;
        (0..n.clamp(0, 10_000)).map(|_| Self::read(&mut r)).collect()
    }
}

/// `minecraft:register`'s payload: the channels, NUL-separated.
pub fn register_payload() -> Vec<u8> {
    CHANNELS.join("\0").into_bytes()
}
pub fn request_secret() -> Vec<u8> {
    let mut w = Writer::default();
    w.i32(COMPATIBILITY);
    w.out
}
pub fn update_state(disabled: bool) -> Vec<u8> {
    let mut w = Writer::default();
    w.bool(disabled);
    w.out
}

/// The UDP packets, by their type byte.
#[derive(Clone, Debug, PartialEq)]
pub enum Packet {
    /// 1: our voice, an Opus frame (empty: we stopped talking).
    Mic { data: Vec<u8>, sequence: i64, whispering: bool },
    /// 2: a player's voice, heard from where they are.
    Player { sender: u128, data: Vec<u8>, sequence: i64, distance: f32, whispering: bool },
    /// 3: a group member's voice, heard everywhere.
    Group { sender: u128, data: Vec<u8>, sequence: i64 },
    /// 4: a voice from a place.
    Location { sender: u128, at: [f64; 3], data: Vec<u8>, sequence: i64, distance: f32 },
    /// 5: who we are, until 6 says the server knows.
    Authenticate { player: u128, secret: [u8; 16] },
    AuthenticateAck,
    /// 7: the server measuring the way back: sent back as it came.
    Ping { id: u128, timestamp: i64 },
    KeepAlive,
    /// 9: checks the way works both ways, until 10 comes back.
    ConnectionCheck,
    ConnectionCheckAck,
}
const WHISPER: u8 = 1;
const CATEGORY: u8 = 2;

impl Packet {
    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Packet::Mic { data, sequence, whispering } => {
                w.u8(1).bytes(data).i64(*sequence).bool(*whispering);
            }
            Packet::Player { sender, data, sequence, distance, whispering } => {
                w.u8(2).uuid(*sender).uuid(*sender).bytes(data).i64(*sequence);
                w.out.extend_from_slice(&distance.to_be_bytes());
                w.u8(if *whispering { WHISPER } else { 0 });
            }
            Packet::Group { sender, data, sequence } => {
                w.u8(3).uuid(*sender).uuid(*sender).bytes(data).i64(*sequence).u8(0);
            }
            Packet::Location { sender, at, data, sequence, distance } => {
                w.u8(4).uuid(*sender).uuid(*sender);
                for v in at {
                    w.out.extend_from_slice(&v.to_be_bytes());
                }
                w.bytes(data).i64(*sequence);
                w.out.extend_from_slice(&distance.to_be_bytes());
                w.u8(0);
            }
            Packet::Authenticate { player, secret } => {
                w.u8(5).uuid(*player);
                w.out.extend_from_slice(secret);
            }
            Packet::AuthenticateAck => {
                w.u8(6);
            }
            Packet::Ping { id, timestamp } => {
                w.u8(7).uuid(*id).i64(*timestamp);
            }
            Packet::KeepAlive => {
                w.u8(8);
            }
            Packet::ConnectionCheck => {
                w.u8(9);
            }
            Packet::ConnectionCheckAck => {
                w.u8(10);
            }
        }
        w.out
    }

    pub fn read(data: &[u8]) -> Option<Self> {
        let mut r = Reader::new(data);
        // A sound's flags: whispering, then a category name to skip.
        let flags = |r: &mut Reader| -> Option<u8> {
            let flags = r.u8()?;
            if flags & CATEGORY != 0 {
                r.string(16)?;
            }
            Some(flags)
        };
        Some(match r.u8()? {
            1 => Packet::Mic {
                data: r.bytes(MAX_OPUS)?.to_vec(),
                sequence: r.i64()?,
                whispering: r.bool()?,
            },
            2 => {
                let _channel = r.uuid()?;
                let sender = r.uuid()?;
                let data = r.bytes(MAX_OPUS)?.to_vec();
                let sequence = r.i64()?;
                let distance = r.f32()?;
                let whispering = flags(&mut r)? & WHISPER != 0;
                Packet::Player { sender, data, sequence, distance, whispering }
            }
            3 => {
                let _channel = r.uuid()?;
                let sender = r.uuid()?;
                let data = r.bytes(MAX_OPUS)?.to_vec();
                let sequence = r.i64()?;
                flags(&mut r)?;
                Packet::Group { sender, data, sequence }
            }
            4 => {
                let _channel = r.uuid()?;
                let sender = r.uuid()?;
                let at = [r.f64()?, r.f64()?, r.f64()?];
                let data = r.bytes(MAX_OPUS)?.to_vec();
                let sequence = r.i64()?;
                let distance = r.f32()?;
                flags(&mut r)?;
                Packet::Location { sender, at, data, sequence, distance }
            }
            5 => Packet::Authenticate {
                player: r.uuid()?,
                secret: r.take(16)?.try_into().ok()?,
            },
            6 => Packet::AuthenticateAck,
            7 => Packet::Ping { id: r.uuid()?, timestamp: r.i64()? },
            8 => Packet::KeepAlive,
            9 => Packet::ConnectionCheck,
            10 => Packet::ConnectionCheckAck,
            _ => return None,
        })
    }
}

/// Seals `plain` with `key`: a fresh nonce, then the ciphertext and its tag (`Secret.encrypt`).
pub fn seal(key: &[u8; 16], plain: &[u8]) -> Option<Vec<u8>> {
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut nonce).ok()?;
    let cipher = Aes128Gcm::new(key.into());
    let sealed = cipher.encrypt(Nonce::from_slice(&nonce), plain).ok()?;
    let mut out = Vec::with_capacity(12 + sealed.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Some(out)
}
/// `seal`'s way back; None when the tag doesn't match (another key, or a forged packet).
pub fn open(key: &[u8; 16], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < 12 + 16 {
        return None;
    }
    let (nonce, rest) = sealed.split_at(12);
    Aes128Gcm::new(key.into()).decrypt(Nonce::from_slice(nonce), rest).ok()
}

/// A datagram to the server: `0xFF`, our UUID, the sealed packet as a byte array.
pub fn to_server(player: u128, key: &[u8; 16], packet: &Packet) -> Option<Vec<u8>> {
    let sealed = seal(key, &packet.write())?;
    let mut w = Writer::default();
    w.u8(0xFF).uuid(player).bytes(&sealed);
    Some(w.out)
}
/// A datagram from the server: `0xFF`, then the sealed packet as a byte array.
pub fn from_server(key: &[u8; 16], datagram: &[u8]) -> Option<Packet> {
    let mut r = Reader::new(datagram);
    if r.u8()? != 0xFF {
        return None;
    }
    let sealed = r.bytes(4096)?;
    Packet::read(&open(key, sealed)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_secret_reads_as_the_server_writes_it() {
        let mut w = Writer::default();
        w.out.extend_from_slice(&[7; 16]);
        w.i32(24454).uuid(0x0123_4567_89AB_CDEF_0011_2233_4455_6677).u8(0).i32(1024);
        w.out.extend_from_slice(&48.0f64.to_be_bytes());
        w.i32(1000).bool(true);
        w.bytes(b"voice.example.org:25000").bool(false);
        let s = Secret::parse(&w.out).unwrap();
        assert_eq!((s.port, s.mtu, s.keep_alive, s.distance), (24454, 1024, 1000, 48.0));
        assert_eq!(s.player, 0x0123_4567_89AB_CDEF_0011_2233_4455_6677);
        assert_eq!(s.address("play.example.org"), ("voice.example.org".into(), 25000));
        let plain = Secret { host: String::new(), ..s.clone() };
        assert_eq!(plain.address("play.example.org"), ("play.example.org".into(), 24454));
        let port = Secret { host: "25565".into(), ..s };
        assert_eq!(port.address("1.2.3.4"), ("1.2.3.4".into(), 25565));
        assert_eq!(split_host_port("[::1]:5"), ("::1", Some(5)));
        assert_eq!(split_host_port("::1"), ("::1", None));
    }

    #[test]
    fn packets_round_trip_through_the_seal() {
        let key = [3u8; 16];
        for p in [
            Packet::Mic { data: vec![1, 2, 3], sequence: 9, whispering: false },
            Packet::Player { sender: 5, data: vec![4; 60], sequence: -1, distance: 48.0, whispering: true },
            Packet::Group { sender: 6, data: vec![], sequence: 2 },
            Packet::Location { sender: 7, at: [1.0, 2.0, 3.5], data: vec![9], sequence: 4, distance: 8.0 },
            Packet::Authenticate { player: 11, secret: key },
            Packet::Ping { id: 12, timestamp: 1_700_000 },
            Packet::KeepAlive,
            Packet::ConnectionCheckAck,
        ] {
            let sealed = seal(&key, &p.write()).unwrap();
            assert_eq!(Packet::read(&open(&key, &sealed).unwrap()), Some(p.clone()));
            assert!(open(&[4; 16], &sealed).is_none(), "another key can't open it");
        }
        // The server's framing: no UUID, the sealed bytes as an array.
        let mut w = Writer::default();
        w.u8(0xFF).bytes(&seal(&key, &Packet::KeepAlive.write()).unwrap());
        assert_eq!(from_server(&key, &w.out), Some(Packet::KeepAlive));
        // Ours: our UUID before the sealed bytes.
        let out = to_server(0xAB, &key, &Packet::KeepAlive).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!((r.u8(), r.uuid()), (Some(0xFF), Some(0xAB)));
    }

    #[test]
    fn states_and_var_ints() {
        let mut w = Writer::default();
        w.i32(2);
        for (uuid, group) in [(1u128, None), (2, Some(9u128))] {
            w.bool(false).bool(true).uuid(uuid).bytes(b"Steve").bool(group.is_some());
            if let Some(g) = group {
                w.uuid(g);
            }
        }
        let states = PlayerState::read_all(&w.out).unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!((states[1].group, states[0].disconnected), (Some(9), true));
        for v in [0, 1, 127, 128, 300, 2_097_151, i32::MAX, -1] {
            let mut w = Writer::default();
            w.var_int(v);
            assert_eq!(Reader::new(&w.out).var_int(), Some(v));
        }
        assert_eq!(register_payload().split(|b| *b == 0).count(), CHANNELS.len());
    }
}
