//! Oteria's music (the Music tile, off by default): the song playing on this computer (Spotify,
//! or any player Windows' media controls know: a browser's YouTube, Deezer...) on a card at the
//! top right, its play/pause, next and previous on keys of their own, and its synced lyrics in the
//! world around the player: each word pops in on its beat at a place of its own in the air ahead,
//! nearer or further, a little turned, glowing in the line's colour, and stays there (the player
//! can walk past it); a second later it fades away, rising.
//!
//! The song comes from Windows' media controls (`GlobalSystemMediaTransportControls`): no
//! account and no login. The lyrics come from LRCLIB (`horizon_addresses::LRCLIB_API`), which
//! is asked the song's title, artist, album and length once per song. Nothing else leaves the
//! computer. Other systems have no such controls: there the tile shows nothing.

use super::Game;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

/// A song as the media controls give it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Its length, milliseconds (0: unknown).
    pub length: u64,
    pub playing: bool,
    /// Where it was, milliseconds, and when that was read.
    position: u64,
    at: Option<Instant>,
    /// Spotify's own session (else another player's).
    pub spotify: bool,
    /// Its cover, 64x64 RGBA, when the player gives one.
    pub cover: Option<std::sync::Arc<Vec<u8>>>,
    /// Shuffle on, and repeat (0 off, 1 the song, 2 the list).
    pub shuffle: bool,
    pub repeat: u8,
}
impl Track {
    /// Where the song is now, milliseconds: its last position moved on while it plays.
    pub fn now(&self) -> u64 {
        let moved = match (self.playing, self.at) {
            (true, Some(at)) => at.elapsed().as_millis() as u64,
            _ => 0,
        };
        let now = self.position + moved;
        if self.length > 0 {
            now.min(self.length)
        } else {
            now
        }
    }
    fn key(&self) -> (String, String) {
        (self.title.clone(), self.artist.clone())
    }
}

/// Synced lyrics: each line from its time, milliseconds, and its words with their own times
/// when the lyrics have them (enhanced LRC), else `None` (spread over the line).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Lyrics {
    pub lines: Vec<(u64, String)>,
    pub words: Vec<Vec<(Option<u64>, String)>>,
}
impl Lyrics {
    /// Whether the lines' times follow the song, not a regular step: lyrics only spread out
    /// (as LRCLIB has for some songs) are timed at a few round steps, the same over and over,
    /// where a song's own are nearly all different.
    pub fn timed_on_the_song(&self) -> bool {
        if self.lines.is_empty() {
            return false;
        }
        let gaps: Vec<u64> = self
            .lines
            .windows(2)
            .map(|w| w[1].0 - w[0].0)
            .filter(|&g| g > 0)
            .collect();
        if gaps.len() < 6 {
            return true;
        }
        let mut distinct = gaps.clone();
        distinct.sort_unstable();
        distinct.dedup();
        distinct.len() * 10 >= gaps.len() * 4
    }
    /// The line sung at `ms`, by index.
    pub fn at(&self, ms: u64) -> Option<usize> {
        let i = self.lines.partition_point(|(t, _)| *t <= ms);
        i.checked_sub(1)
    }
}

/// A singer's pace, for lyrics without each word's time: milliseconds a letter, and the
/// letters' worth of time between two words.
const LETTER_MS: u64 = 75;
const WORD_GAP: u64 = 2;
/// How long the words wait after the player's position, for the sound to reach the ears:
/// 180 ms was seen late with Spotify, whose position already matches what is heard.
const HEARD_AFTER_MS: u64 = 0;

/// LRC text (`[mm:ss.xx] words`, several times on one line allowed) as lines in time order.
pub(super) fn parse_lrc(text: &str) -> Lyrics {
    let mut lines: Vec<(u64, String, Vec<(Option<u64>, String)>)> = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(tail) = rest.strip_prefix('[') {
            let Some((tag, after)) = tail.split_once(']') else {
                break;
            };
            if let Some(ms) = lrc_time(tag) {
                times.push(ms);
            }
            rest = after;
        }
        let plain = strip_word_times(rest);
        let words = timed_words(rest);
        lines.extend(times.into_iter().map(|t| (t, plain.clone(), words.clone())));
    }
    lines.sort_by_key(|(t, _, _)| *t);
    let (lines, words) = lines.into_iter().map(|(t, l, w)| ((t, l), w)).unzip();
    Lyrics { lines, words }
}
/// A line's words, each with the time of the `<mm:ss.xx>` tag before it, if any.
fn timed_words(line: &str) -> Vec<(Option<u64>, String)> {
    let mut out: Vec<(Option<u64>, String)> = Vec::new();
    let mut pending = None;
    for token in line.split_whitespace() {
        let mut rest = token;
        while let Some(tail) = rest.strip_prefix('<') {
            match tail.split_once('>') {
                Some((tag, after)) if lrc_time(tag).is_some() => {
                    pending = lrc_time(tag);
                    rest = after;
                }
                _ => break,
            }
        }
        let word = strip_word_times(rest).replace('\u{2019}', "'");
        if word.is_empty() {
            continue;
        }
        // An elision ("j'", "l'", "qu'") is one word with the one after it.
        match out.last_mut() {
            Some((_, last)) if elision(last) => last.push_str(&word),
            _ => out.push((pending.take(), word)),
        }
    }
    out
}
/// A word cut short before the next one: a few letters and an apostrophe ("j'", "qu'").
fn elision(word: &str) -> bool {
    word.strip_suffix('\'')
        .is_some_and(|w| (1..=3).contains(&w.chars().count()) && w.chars().all(char::is_alphabetic))
}
impl Lyrics {
    /// Line `i`'s words with the time each pops in: its own, else when it is sung, the line
    /// sung from its start at a singer's pace, each word taking as long as its letters (and a
    /// little more), the whole within the time before the next line.
    pub fn word_times(&self, i: usize) -> Vec<(u64, String)> {
        let Some(&(start, _)) = self.lines.get(i) else {
            return Vec::new();
        };
        let words = self.words.get(i).cloned().unwrap_or_default();
        let gap = self
            .lines
            .get(i + 1)
            .map_or(4_000, |(t, _)| t.saturating_sub(start))
            .max(1);
        let weight = |w: &str| w.chars().filter(|c| c.is_alphanumeric()).count() as u64 + WORD_GAP;
        let total: u64 = words.iter().map(|(_, w)| weight(w)).sum::<u64>().max(1);
        // A line sung at its pace, unless the next one comes first: then it fills its time,
        // all but a breath at its end.
        let sung = (total * LETTER_MS).min(gap * 9 / 10);
        // Never before the word ahead of it.
        let mut last = start;
        let mut before = 0;
        words
            .into_iter()
            .map(|(t, w)| {
                let spread = start + sung * before / total;
                before += weight(&w);
                last = t.unwrap_or(spread).max(last);
                (last, w)
            })
            .collect()
    }
}
/// A line with each word's own time (`<mm:ss.xx>word`, enhanced LRC) as its words alone.
fn strip_word_times(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let (before, tail) = rest.split_at(open);
        out.push_str(before);
        match tail[1..].split_once('>') {
            Some((tag, after)) if lrc_time(tag).is_some() => rest = after,
            _ => {
                out.push('<');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
/// `mm:ss.xx` (or `mm:ss`) as milliseconds; `None` for other tags (`ar:`, `offset:`...).
fn lrc_time(tag: &str) -> Option<u64> {
    let (m, s) = tag.split_once(':')?;
    let minutes: u64 = m.trim().parse().ok()?;
    let seconds: f64 = s.trim().parse().ok()?;
    (seconds >= 0.0).then(|| minutes * 60_000 + (seconds * 1000.0).round() as u64)
}

/// What the player asks of the music.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    PlayPause,
    Next,
    Previous,
    Shuffle,
    Repeat,
}

/// A cover image (PNG or JPEG) brought to 64x64 RGBA, each texel the mean of the image's under it.
pub(super) fn cover_texels(bytes: &[u8]) -> Option<Vec<u8>> {
    let (w, h, rgba) = horizon_client::hud::decode_image_bytes(bytes).ok()?;
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return None;
    }
    // The middle square, as players show covers.
    let side = w.min(h);
    let (ox, oy) = ((w - side) / 2, (h - side) / 2);
    let mut out = vec![0u8; 64 * 64 * 4];
    for ty in 0..64 {
        for tx in 0..64 {
            let (x0, x1) = (
                ox + tx * side / 64,
                ox + ((tx + 1) * side / 64).max(tx * side / 64 + 1),
            );
            let (y0, y1) = (
                oy + ty * side / 64,
                oy + ((ty + 1) * side / 64).max(ty * side / 64 + 1),
            );
            let mut sum = [0u32; 4];
            let mut n = 0u32;
            for y in y0..y1.min(h) {
                for x in x0..x1.min(w) {
                    let i = (y * w + x) * 4;
                    for c in 0..4 {
                        sum[c] += u32::from(rgba[i + c]);
                    }
                    n += 1;
                }
            }
            let o = (ty * 64 + tx) * 4;
            for c in 0..4 {
                out[o + c] = (sum[c] / n.max(1)) as u8;
            }
        }
    }
    Some(out)
}

#[derive(Default)]
struct Shared {
    track: Option<Track>,
    /// Lyrics by song: `None` while asked, `Some(empty)` when there are none.
    lyrics: HashMap<(String, String), Option<Lyrics>>,
}

struct Music {
    shared: Mutex<Shared>,
    commands: Mutex<mpsc::Sender<Command>>,
}

fn music() -> &'static Music {
    static MUSIC: OnceLock<Music> = OnceLock::new();
    MUSIC.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("oteria music".into())
            .spawn(move || {
                // For tests and the harness: a song of its own, its lyrics given, nothing asked.
                if std::env::var_os("OTERIA_MUSIC_DEMO").is_some() {
                    demo(rx)
                } else {
                    media::watch(rx)
                }
            })
            .ok();
        Music {
            shared: Mutex::new(Shared::default()),
            commands: Mutex::new(tx),
        }
    })
}

/// `OTERIA_MUSIC_DEMO`: a song of 20 seconds that plays over and over, with its own lyrics, in
/// place of the media controls (`horizon-ctl` runs, where nothing should play for real).
fn demo(_commands: mpsc::Receiver<Command>) {
    let started = Instant::now();
    let lyrics = parse_lrc(
        "[00:00.50] Des mots qui dansent dans le ciel\n[00:04.00] On avance sans regarder derrière\n\
         [00:08.00] J'entends la nuit qui m'appelle\n[00:12.00] Tout y est, on l'a fait\n[00:16.00] Encore une fois",
    );
    {
        let mut s = music().shared.lock().unwrap_or_else(|e| e.into_inner());
        s.lyrics
            .insert(("Demo".into(), "Oteria".into()), Some(lyrics));
    }
    loop {
        let ms = started.elapsed().as_millis() as u64 % 20_000;
        publish(Some(Track {
            title: "Demo".into(),
            artist: "Oteria".into(),
            album: String::new(),
            length: 20_000,
            playing: true,
            position: ms,
            at: Some(Instant::now()),
            spotify: false,
            ..Track::default()
        }));
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The media thread's news: the song now (`None`: nothing plays).
fn publish(track: Option<Track>) {
    let m = music();
    let ask = {
        let mut s = m.shared.lock().unwrap_or_else(|e| e.into_inner());
        let new = track
            .as_ref()
            .filter(|t| !t.title.is_empty() && !s.lyrics.contains_key(&t.key()))
            .cloned();
        if let Some(t) = &new {
            s.lyrics.insert(t.key(), None);
        }
        s.track = track;
        new
    };
    if let Some(t) = ask {
        std::thread::Builder::new()
            .name("oteria lyrics".into())
            .spawn(move || {
                let lyrics = fetch_lyrics(&t).unwrap_or_default();
                let mut s = music().shared.lock().unwrap_or_else(|e| e.into_inner());
                s.lyrics.insert(t.key(), Some(lyrics));
            })
            .ok();
    }
}

/// The song's synced lyrics from LRCLIB: the exact song first, then a search.
fn fetch_lyrics(t: &Track) -> Option<Lyrics> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .user_agent("Oteria Client")
        .build();
    let base = horizon_addresses::LRCLIB_API;
    // Only lyrics really timed on the song: LRCLIB also holds plain lyrics spread at a regular
    // step from 0, which would show the words over the intro.
    let synced = |v: &serde_json::Value| {
        v.get("syncedLyrics")
            .and_then(|s| s.as_str())
            .map(parse_lrc)
            .filter(Lyrics::timed_on_the_song)
    };
    let mut get = agent
        .get(&format!("{base}/get"))
        .query("track_name", &t.title)
        .query("artist_name", &t.artist);
    if !t.album.is_empty() {
        get = get.query("album_name", &t.album);
    }
    if t.length > 0 {
        get = get.query("duration", &(t.length / 1000).to_string());
    }
    if let Ok(r) = get.call()
        && let Ok(body) = r.into_string()
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&body)
        && let Some(l) = synced(&v)
    {
        return Some(l);
    }
    // Not that exact song: the closest one found by title and artist, its length nearest.
    let body = agent
        .get(&format!("{base}/search"))
        .query("track_name", &t.title)
        .query("artist_name", &t.artist)
        .call()
        .ok()?
        .into_string()
        .ok()?;
    let found: Vec<serde_json::Value> = serde_json::from_str(&body).ok()?;
    let seconds = (t.length / 1000) as f64;
    found
        .iter()
        .filter(|v| synced(v).is_some())
        .min_by_key(|v| {
            let d = v.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
            ((d - seconds).abs() * 10.0) as u64
        })
        .and_then(synced)
}

/// Windows' media controls, polled on their thread.
#[cfg(windows)]
mod media {
    use super::{Command, Track, publish};
    use std::sync::mpsc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Media::MediaPlaybackAutoRepeatMode as RepeatMode;

    /// Now as a Windows `DateTime`: 100 ns ticks since 1601.
    fn file_time_now() -> i64 {
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        (unix.as_nanos() / 100) as i64 + 116_444_736_000_000_000
    }

    pub(super) fn watch(commands: mpsc::Receiver<Command>) {
        // No apartment set up here: windows-rs joins the process's multithreaded one itself.
        let mut manager: Option<Manager> = None;
        // The last song's cover, read once per song.
        let mut cover: (String, Option<std::sync::Arc<Vec<u8>>>) = (String::new(), None);
        loop {
            if manager.is_none() {
                manager = Manager::RequestAsync().and_then(|op| op.join()).ok();
            }
            let session = manager.as_ref().and_then(pick);
            while let Ok(c) = commands.try_recv() {
                if let Some(s) = &session {
                    let _ = match c {
                        Command::PlayPause => s.TryTogglePlayPauseAsync().and_then(|o| o.join()),
                        Command::Next => s.TrySkipNextAsync().and_then(|o| o.join()),
                        Command::Previous => s.TrySkipPreviousAsync().and_then(|o| o.join()),
                        Command::Shuffle => {
                            let on = s
                                .GetPlaybackInfo()
                                .and_then(|p| p.IsShuffleActive())
                                .and_then(|v| v.Value())
                                .unwrap_or(false);
                            s.TryChangeShuffleActiveAsync(!on).and_then(|o| o.join())
                        }
                        Command::Repeat => {
                            let now = s
                                .GetPlaybackInfo()
                                .and_then(|p| p.AutoRepeatMode())
                                .and_then(|v| v.Value())
                                .unwrap_or(RepeatMode::None);
                            // Off, the list, the song, as players step it.
                            let next = match now {
                                RepeatMode::None => RepeatMode::List,
                                RepeatMode::List => RepeatMode::Track,
                                _ => RepeatMode::None,
                            };
                            s.TryChangeAutoRepeatModeAsync(next).and_then(|o| o.join())
                        }
                    };
                }
            }
            let mut track = session.as_ref().and_then(read);
            if let (Some(t), Some(s)) = (track.as_mut(), session.as_ref()) {
                let key = format!("{}\u{1}{}", t.title, t.artist);
                if cover.0 != key {
                    cover = (key, read_cover(s).map(std::sync::Arc::new));
                }
                t.cover = cover.1.clone();
            }
            publish(track);
            std::thread::sleep(Duration::from_millis(400));
        }
    }

    #[cfg(test)]
    pub(super) fn pick_for_test(m: &Manager) -> Option<Session> {
        pick(m)
    }
    #[cfg(test)]
    pub(super) fn read_for_test(s: &Session) -> Option<Track> {
        read(s)
    }
    #[cfg(test)]
    pub(super) fn cover_for_test(s: &Session) -> Option<Vec<u8>> {
        read_cover(s)
    }

    /// Spotify's session when there is one, else the one Windows calls current.
    fn pick(m: &Manager) -> Option<Session> {
        if let Ok(all) = m.GetSessions() {
            for i in 0..all.Size().unwrap_or(0) {
                if let Ok(s) = all.GetAt(i)
                    && s.SourceAppUserModelId()
                        .is_ok_and(|id| id.to_string().to_lowercase().contains("spotify"))
                {
                    return Some(s);
                }
            }
        }
        m.GetCurrentSession().ok()
    }

    /// The song's cover as the player gives it (`Thumbnail`), 64x64.
    fn read_cover(s: &Session) -> Option<Vec<u8>> {
        use windows::Storage::Streams::{DataReader, IInputStream};
        use windows::core::Interface;
        let props = s.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
        let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
        let size = u32::try_from(stream.Size().ok()?).ok()?.min(8 << 20);
        let reader = DataReader::CreateDataReader(&stream.cast::<IInputStream>().ok()?).ok()?;
        let loaded = reader.LoadAsync(size).ok()?.join().ok()?;
        let mut bytes = vec![0u8; loaded as usize];
        reader.ReadBytes(&mut bytes).ok()?;
        super::cover_texels(&bytes)
    }

    fn read(s: &Session) -> Option<Track> {
        let props = s.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
        let title = props.Title().ok()?.to_string();
        if title.is_empty() {
            return None;
        }
        let info = s.GetPlaybackInfo().ok();
        let playing = info
            .as_ref()
            .and_then(|p| p.PlaybackStatus().ok())
            .is_some_and(|st| st == Status::Playing);
        let shuffle = info
            .as_ref()
            .and_then(|p| p.IsShuffleActive().ok())
            .and_then(|v| v.Value().ok())
            .unwrap_or(false);
        let repeat = match info
            .as_ref()
            .and_then(|p| p.AutoRepeatMode().ok())
            .and_then(|v| v.Value().ok())
        {
            Some(RepeatMode::Track) => 1,
            Some(RepeatMode::List) => 2,
            _ => 0,
        };
        let timeline = s.GetTimelineProperties().ok()?;
        let ticks = |t: i64| (t.max(0) / 10_000) as u64;
        let position = timeline.Position().map(|p| p.Duration).unwrap_or(0);
        let updated = timeline
            .LastUpdatedTime()
            .map(|d| d.UniversalTime)
            .unwrap_or(0);
        let length = timeline.EndTime().map(|e| e.Duration).unwrap_or(0);
        // The position is from when it was last updated: moved on to now while it plays.
        let since = if playing && updated > 0 {
            (file_time_now() - updated).max(0)
        } else {
            0
        };
        Some(Track {
            title,
            artist: props.Artist().map(|a| a.to_string()).unwrap_or_default(),
            album: props
                .AlbumTitle()
                .map(|a| a.to_string())
                .unwrap_or_default(),
            length: ticks(length),
            playing,
            position: ticks(position + since),
            at: Some(Instant::now()),
            spotify: s
                .SourceAppUserModelId()
                .is_ok_and(|id| id.to_string().to_lowercase().contains("spotify")),
            cover: None,
            shuffle,
            repeat,
        })
    }
}

#[cfg(not(windows))]
mod media {
    use super::Command;
    use std::sync::mpsc;

    /// No media controls to read here: nothing ever plays.
    pub(super) fn watch(commands: mpsc::Receiver<Command>) {
        while commands.recv().is_ok() {}
    }
}

/// The player widget's size at 100%, GUI pixels.
const WIDTH: f32 = 190.0;
const HEIGHT: f32 = 80.0;

/// The player's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Shuffle,
    Previous,
    PlayPause,
    Next,
    Repeat,
}
thread_local! {
    /// Where the player's buttons were drawn (GUI pixels), for clicks.
    static CONTROLS: std::cell::RefCell<Vec<([f32; 4], Button)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A cover's id: a hash of its texels.
fn cover_hash(texels: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texels.iter().step_by(7) {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A button's picture, 9 GUI pixels square at (x, y), in pixels as the font's.
#[allow(clippy::too_many_arguments)]
fn draw_button(
    batch: &mut horizon_client::hud::HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    button: Button,
    x: f32,
    y: f32,
    playing: bool,
    repeat: u8,
    c: u32,
) {
    let px = |batch: &mut horizon_client::hud::HudBatch, dx: f32, dy: f32, w: f32, h: f32| {
        batch.fill(atlas, x + dx, y + dy, w, h, c)
    };
    let tri = |batch: &mut horizon_client::hud::HudBatch, tip: f32, base: f32| {
        // A triangle from a base (x = base, 9 tall) to a tip (x = tip, middle).
        batch.fill_quad(
            atlas,
            [
                [x + base, y],
                [x + tip, y + 4.5],
                [x + tip, y + 4.5],
                [x + base, y + 9.0],
            ],
            c,
        );
    };
    match button {
        Button::PlayPause if playing => {
            px(batch, 1.0, 0.0, 2.5, 9.0);
            px(batch, 5.5, 0.0, 2.5, 9.0);
        }
        Button::PlayPause => tri(batch, 8.5, 1.0),
        Button::Previous => {
            px(batch, 0.5, 0.0, 1.5, 9.0);
            tri(batch, 2.0, 8.5);
        }
        Button::Next => {
            tri(batch, 7.0, 0.5);
            px(batch, 7.0, 0.0, 1.5, 9.0);
        }
        Button::Shuffle => {
            // Two lines crossing, each ending in an arrow to the right.
            for i in 0..7 {
                let f = i as f32;
                px(batch, f, 1.0 + f, 1.0, 1.0);
                px(batch, f, 7.0 - f, 1.0, 1.0);
            }
            px(batch, 7.0, 0.0, 2.0, 1.0);
            px(batch, 8.0, 1.0, 1.0, 2.0);
            px(batch, 7.0, 8.0, 2.0, 1.0);
            px(batch, 8.0, 6.0, 1.0, 2.0);
        }
        Button::Repeat => {
            // A loop: a box open at the top right, an arrow there.
            px(batch, 0.0, 2.0, 1.0, 6.0);
            px(batch, 0.0, 8.0, 9.0, 1.0);
            px(batch, 8.0, 2.0, 1.0, 6.0);
            px(batch, 0.0, 2.0, 5.0, 1.0);
            px(batch, 5.0, 0.0, 1.0, 5.0);
            px(batch, 6.0, 1.0, 1.0, 3.0);
            if repeat == 1 {
                // The song alone: a dot in the loop.
                px(batch, 3.5, 4.5, 2.0, 2.0);
            }
        }
    }
}

/// A word in the world: what it says, where it is and how it is turned, when it came.
struct FloatWord {
    text: String,
    at: glam::DVec3,
    right: glam::Vec3,
    up: glam::Vec3,
    born: f32,
    /// Its text pixel's size, blocks.
    size: f32,
    colour: u32,
    seed: u64,
}

/// The words in the air, those already shown (by line and place in it, for this song), the
/// effects' clock, and the last frame.
#[derive(Default)]
struct Float {
    words: Vec<FloatWord>,
    shown: std::collections::HashSet<(usize, usize)>,
    song: (String, String),
    last: Option<Instant>,
    /// The clock the effects run on, seconds.
    clock: f64,
}

/// A word's life, seconds: whole for `HOLD`, then fading until `LIFE`.
const HOLD: f32 = 1.0;
const LIFE: f32 = 1.7;

/// Word `k` of line `line`, sung now, put in the air ahead of the player looking `(yaw, pitch)`
/// from `camera`: off to a side by up to 50 degrees, 3.5 to 9 blocks away, above or below the
/// look, facing the player, turned and tilted a little. Bigger further, so far words still read.
/// `attempt` tries another place, nearer each time (where the first was in a block). `reach` is
/// the Music tile's lyrics distance, blocks: words land from 0.58 to 1.5 times it (6: 3.5 to 9).
#[allow(clippy::too_many_arguments)]
fn spawn_word(
    text: String,
    line: usize,
    k: usize,
    camera: glam::DVec3,
    (yaw, pitch): (f32, f32),
    clock: f32,
    attempt: u64,
    reach: f32,
) -> FloatWord {
    let seed = (line as u64) << 8 | k as u64;
    let r = |n: u64| hash(seed * 31 + n + attempt * 1009);
    let side = (r(1) - 0.5) * 100.0;
    let (near, far) = (reach * 0.583, reach * 1.5);
    let distance = (near + (far - near) * r(2)) * (1.0 - attempt as f32 * 0.06).max(0.45);
    let dir = (yaw + side).to_radians();
    let flat = glam::DVec3::new(-f64::from(dir.sin()), 0.0, f64::from(dir.cos()));
    let rise = -f64::from(pitch.to_radians().sin()) * f64::from(distance) * 0.8
        + f64::from(r(3) * 2.2 - 0.7);
    let at = camera + flat * f64::from(distance) + glam::DVec3::Y * rise;
    // Facing the player: its rows along the player's right as they look at it.
    let towards = (at - camera).as_vec3();
    let towards = glam::Vec3::new(towards.x, 0.0, towards.z).normalize_or(glam::Vec3::Z);
    let right = towards.cross(glam::Vec3::Y).normalize_or(glam::Vec3::X);
    let turn = glam::Quat::from_axis_angle(glam::Vec3::Y, (r(4) - 0.5) * 0.7)
        * glam::Quat::from_axis_angle(right, (r(5) - 0.5) * 0.5)
        * glam::Quat::from_axis_angle(towards, (r(6) - 0.5) * 0.45);
    let size = super::names::NAME_TAG_UNIT * (1.6 + 0.32 * distance) * (0.85 + 0.3 * r(7));
    let hue = hash(line as u64 * 7 + 3) + clock * 0.03 + k as f32 * 0.06;
    FloatWord {
        text,
        at,
        right: turn * right,
        up: turn * glam::Vec3::Y,
        born: clock,
        size,
        colour: pastel(hue),
        seed,
    }
}

/// A number in 0..1 from `seed`, the same each frame.
fn hash(seed: u64) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 24) as f32
}
/// A soft colour of hue `h` (0..1), `0xRRGGBB`.
fn pastel(h: f32) -> u32 {
    let h6 = h.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h6 % 2.0 - 1.0).abs();
    let (r, g, b) = match h6 as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    // Towards white: soft, not neon.
    let ch = |c: f32| ((0.45 + 0.55 * c) * 255.0) as u32;
    ch(r) << 16 | ch(g) << 8 | ch(b)
}
/// `c` (`0xRRGGBB`) taken `k` of the way to white.
fn mix_white(c: u32, k: f32) -> u32 {
    let ch = |s: u32| {
        let v = ((c >> s) & 0xFF) as f32;
        ((v + (255.0 - v) * k).round() as u32).min(255)
    };
    ch(16) << 16 | ch(8) << 8 | ch(0)
}
/// `t` from 0 to 1, overshooting a little before it settles (a pop).
fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    let c = 1.9;
    1.0 + (c + 1.0) * t * t * t + c * t * t
}
thread_local! {
    static FLOAT: std::cell::RefCell<Float> = std::cell::RefCell::new(Float::default());
}

impl Game<'_> {
    /// The song and its lyrics now, while the Music tile is on.
    fn music_now(&self) -> Option<(Track, Option<Lyrics>)> {
        if !self.settings.extras.pvp.music.enabled {
            return None;
        }
        let s = music().shared.lock().unwrap_or_else(|e| e.into_inner());
        let t = s.track.clone()?;
        let lyrics = s.lyrics.get(&t.key()).cloned().flatten();
        Some((t, lyrics))
    }

    /// The music's keys, in play. True when taken.
    pub(super) fn music_key_press(&mut self, name: &str) -> bool {
        if !self.settings.extras.pvp.music.enabled
            || self.screen != super::Screen::Playing
            || name == "key.keyboard.unknown"
        {
            return false;
        }
        let keys = &self.settings.keys;
        let command = if name == keys.get("key.oteria.music.play") {
            Command::PlayPause
        } else if name == keys.get("key.oteria.music.next") {
            Command::Next
        } else if name == keys.get("key.oteria.music.previous") {
            Command::Previous
        } else {
            return false;
        };
        let _ = music()
            .commands
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(command);
        true
    }

    /// The player, a HUD widget the editor moves and sizes (`WidgetId::Music`): the song's
    /// cover, its title and artist, shuffle, previous, play/pause, next and repeat, and a bar
    /// with the time played and the song's length, over a light haze. Its buttons answer clicks
    /// while the chat or Oteria's menu is open (`music_click`).
    pub(super) fn draw_music_hud(
        &mut self,
        batch: &mut horizon_client::hud::HudBatch,
        corners: &mut horizon_client::extras_hud::Corners,
    ) {
        use horizon_client::hud::rgba;
        use horizon_client::pvp_hud::WidgetId;
        CONTROLS.with(|c| c.borrow_mut().clear());
        let m = &self.settings.extras.pvp.music;
        let editing = self.hud_editing();
        let shown = matches!(
            self.screen,
            super::Screen::Playing | super::Screen::Chat | super::Screen::OteriaMenu
        );
        if !m.enabled || !m.card || !(shown || editing) {
            return;
        }
        let track = self.music_now().map(|(t, _)| t);
        // In the editor with nothing playing: a stand-in to place.
        let track = match (track, editing) {
            (Some(t), _) => t,
            (None, true) => Track {
                title: horizon_client::lang::get_or("oteria.music.none", "No music"),
                artist: "Spotify".into(),
                length: 1,
                ..Track::default()
            },
            (None, false) => return,
        };
        let cover = track.cover.as_ref().map(|c| {
            let id = format!("horizon/oteria_cover/{:016x}", cover_hash(c));
            if !self.skin_slots.contains(&id) || !self.gui.atlas.sprites.contains_key(&id) {
                self.store_skin(id.clone(), c.to_vec());
            }
            id
        });
        let (w, h) = (WIDTH, HEIGHT);
        let screen = (batch.width, batch.height);
        let place = self.settings.extras.pvp.layout.get(WidgetId::Music);
        let Some([x, y, sw, _]) = self.widget_rect(
            corners,
            place,
            horizon_client::extras::Corner::TopLeft,
            (w, h),
            screen,
        ) else {
            return;
        };
        let atlas = &self.gui.atlas;
        let start = batch.vertices.len();
        // The haze: white from the left, fading out to the right.
        let strips = 24;
        for i in 0..strips {
            let t = i as f32 / strips as f32;
            let a = (0.55 * (1.0 - t).powf(1.4) * 255.0) as u8;
            // Edge to edge, never over each other (an overlap would show as a line).
            let (x0, x1) = (x + w * t, x + w * (i + 1) as f32 / strips as f32);
            batch.fill(atlas, x0, y, x1 - x0, h, rgba(235, 238, 230, a));
        }
        // The cover, else a dark square with a note.
        let (cx, cy, cs) = (x + 6.0, y + 6.0, 46.0);
        match cover.as_ref().and_then(|c| atlas.sprites.get(c).copied()) {
            Some(r) => batch.rect_uv(cx, cy, cs, cs, r, u32::MAX),
            None => {
                batch.fill(atlas, cx, cy, cs, cs, rgba(30, 32, 38, 200));
                batch.text(
                    atlas,
                    "\u{266A}",
                    cx + cs / 2.0 - 3.0,
                    cy + cs / 2.0 - 4.0,
                    rgba(255, 255, 255, 200),
                    false,
                );
            }
        }
        // The title and the artist, cut to fit.
        let tx = cx + cs + 7.0;
        let room = x + w - 6.0 - tx;
        let fit = |text: &str| {
            if horizon_client::hud::HudBatch::text_width(atlas, text) <= room {
                return text.to_owned();
            }
            let mut cut: String = text.to_owned();
            while !cut.is_empty()
                && horizon_client::hud::HudBatch::text_width(atlas, &format!("{cut}...")) > room
            {
                cut.pop();
            }
            format!("{}...", cut.trim_end())
        };
        batch.text(
            atlas,
            &fit(&track.title),
            tx,
            cy + 2.0,
            rgba(255, 255, 255, 255),
            true,
        );
        batch.text(
            atlas,
            &fit(&track.artist),
            tx,
            cy + 13.0,
            rgba(225, 225, 225, 255),
            true,
        );
        // The buttons: shuffle, previous, play or pause, next, repeat.
        let by = cy + 31.0;
        let on = rgba(255, 255, 255, 255);
        let off = rgba(255, 255, 255, 120);
        let mut buttons = Vec::new();
        for (k, button) in [
            Button::Shuffle,
            Button::Previous,
            Button::PlayPause,
            Button::Next,
            Button::Repeat,
        ]
        .into_iter()
        .enumerate()
        {
            let bx = tx + k as f32 * 15.0;
            let colour = match button {
                Button::Shuffle if !track.shuffle => off,
                Button::Repeat if track.repeat == 0 => off,
                _ => on,
            };
            draw_button(
                batch,
                atlas,
                button,
                bx,
                by,
                track.playing,
                track.repeat,
                colour,
            );
            buttons.push(([bx - 2.0, by - 2.0, 13.0, 13.0], button));
        }
        // The bar: played, the rest, a knob where it is; the times under it.
        let (bx0, bx1, bar_y) = (x + 6.0, x + w - 6.0, y + h - 18.0);
        let k = if track.length > 0 {
            (track.now() as f32 / track.length as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        batch.fill(atlas, bx0, bar_y, bx1 - bx0, 1.0, rgba(255, 255, 255, 110));
        batch.fill(
            atlas,
            bx0,
            bar_y,
            (bx1 - bx0) * k,
            1.0,
            rgba(255, 255, 255, 255),
        );
        batch.fill(
            atlas,
            bx0 + (bx1 - bx0) * k - 1.5,
            bar_y - 1.5,
            4.0,
            4.0,
            rgba(255, 255, 255, 255),
        );
        let time = |ms: u64| format!("{:02}:{:02}", ms / 60_000, ms / 1000 % 60);
        let (played, length) = (time(track.now()), time(track.length));
        batch.text(
            atlas,
            &played,
            bx0,
            bar_y + 5.0,
            rgba(255, 255, 255, 235),
            true,
        );
        let lw = horizon_client::hud::HudBatch::text_width(atlas, &length);
        batch.text(
            atlas,
            &length,
            bx1 - lw,
            bar_y + 5.0,
            rgba(255, 255, 255, 235),
            true,
        );
        // At its place's size.
        let scale = sw / w;
        if scale != 1.0 {
            batch.scale_since(start, x, y, scale, scale);
        }
        let to_screen = |[bx, by, bw, bh]: [f32; 4]| {
            [
                x + (bx - x) * scale,
                y + (by - y) * scale,
                bw * scale,
                bh * scale,
            ]
        };
        CONTROLS.with(|c| {
            *c.borrow_mut() = buttons
                .into_iter()
                .map(|(r, b)| (to_screen(r), b))
                .collect()
        });
        self.pvp
            .boxes
            .push((WidgetId::Music, [x, y, w * scale, h * scale]));
    }

    /// A click at `cursor` (GUI pixels) on the player's buttons, while the chat or Oteria's menu
    /// is open (the mouse is free). True when it was one.
    pub(super) fn music_click(&mut self, cursor: (f32, f32)) -> bool {
        let hit = CONTROLS.with(|c| {
            c.borrow()
                .iter()
                .find(|(r, _)| {
                    cursor.0 >= r[0]
                        && cursor.0 < r[0] + r[2]
                        && cursor.1 >= r[1]
                        && cursor.1 < r[1] + r[3]
                })
                .map(|h| h.1)
        });
        let Some(button) = hit else {
            return false;
        };
        let command = match button {
            Button::Shuffle => Command::Shuffle,
            Button::Previous => Command::Previous,
            Button::PlayPause => Command::PlayPause,
            Button::Next => Command::Next,
            Button::Repeat => Command::Repeat,
        };
        self.audio.play_button_click();
        let _ = music()
            .commands
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(command);
        true
    }

    /// The words sung, in the world: each pops in on its beat at a place of its own ahead of the
    /// player (nearer or further, a little turned), stays there, and a second later fades away.
    pub(super) fn music_lyrics_geometry(
        &mut self,
        camera: glam::DVec3,
        out: &mut Vec<horizon_client::mesher::Vertex>,
    ) -> Vec<horizon_client::font::Patch> {
        let shown = self.settings.extras.pvp.music.lyrics && self.screen_shows_world();
        let song = self.music_now().filter(|_| shown);
        let Some((track, Some(lyrics))) = song else {
            FLOAT.with(|f| *f.borrow_mut() = Float::default());
            return Vec::new();
        };
        // What is heard now, not what the player has sent to the speakers.
        let now = track.now().saturating_sub(HEARD_AFTER_MS);
        let (yaw, pitch) = (self.yaw, self.pitch);
        let words = FLOAT.with(|f| {
            let mut f = f.borrow_mut();
            let t = Instant::now();
            let dt = f
                .last
                .map_or(0.0, |l| t.duration_since(l).as_secs_f64())
                .min(0.1);
            f.last = Some(t);
            f.clock += dt;
            let clock = f.clock as f32;
            // Another song: none of this one's words.
            if f.song != (track.title.clone(), track.artist.clone()) {
                f.song = (track.title.clone(), track.artist.clone());
                f.shown.clear();
                f.words.clear();
            }
            // The words just sung (this line and the one before), each once, in the air.
            if let Some(i) = lyrics.at(now) {
                for line in i.saturating_sub(1)..=i {
                    for (k, (at, word)) in lyrics.word_times(line).into_iter().enumerate() {
                        let fresh = at <= now && now - at < 1_000;
                        if fresh
                            && f.shown.insert((line, k))
                            && let Some(w) =
                                self.place_word(word, line, k, camera, (yaw, pitch), clock)
                        {
                            f.words.push(w);
                        }
                    }
                }
            }
            f.words.retain(|w| clock - w.born < LIFE);
            f.words
                .iter()
                .map(|w| {
                    (
                        w.text.clone(),
                        w.at,
                        w.right,
                        w.up,
                        clock - w.born,
                        w.size,
                        w.colour,
                        w.seed,
                    )
                })
                .collect::<Vec<_>>()
        });
        let mut patches = Vec::new();
        let light = horizon_client::block_entity_render::vertex_light(0xFF);
        for (text, at, right, up, age, size, colour, seed) in words {
            self.lyric_word(
                camera,
                &text,
                at,
                (right, up),
                age,
                size,
                colour,
                seed,
                light,
                out,
                &mut patches,
            );
        }
        patches
    }

    /// Where word `k` of line `line` goes: its first place in the air with nothing between it
    /// and the camera, out of a dozen tried, nearer each time; `None` when there is none.
    #[allow(clippy::too_many_arguments)]
    fn place_word(
        &self,
        text: String,
        line: usize,
        k: usize,
        camera: glam::DVec3,
        look: (f32, f32),
        clock: f32,
    ) -> Option<FloatWord> {
        let reach = f32::from(self.settings.extras.pvp.music.distance);
        (0..12).find_map(|attempt| {
            let w = spawn_word(text.clone(), line, k, camera, look, clock, attempt, reach);
            self.word_seen(camera, &w).then_some(w)
        })
    }

    /// Whether `w` is in the air, its whole width, and seen from `camera`: no block that hides
    /// the view in it nor on the way to its middle and its ends.
    fn word_seen(&self, camera: glam::DVec3, w: &FloatWord) -> bool {
        let facts = self.res.facts();
        let blocks = |p: glam::DVec3| {
            let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            if !self.world.chunks.contains_key(&(x >> 4, z >> 4)) {
                return false;
            }
            let f = facts.get(self.world.block(x, y, z));
            f.full_collision || f.legacy_solid
        };
        // Its ends: a text pixel is `size` blocks, a character about 6 of them.
        let half = f64::from(w.text.chars().count() as f32 * 3.0 * w.size + w.size * 4.0);
        let (right, up) = (w.right.as_dvec3(), w.up.as_dvec3());
        let tall = f64::from(w.size * 6.0);
        let points = [
            w.at,
            w.at + right * half,
            w.at - right * half,
            w.at + up * tall,
            w.at - up * tall,
        ];
        points.iter().all(|&p| {
            let way = p - camera;
            let steps = (way.length() / 0.25).ceil().max(1.0) as usize;
            // From a little ahead of the eyes (not the block the head is in) to the point.
            (2..=steps).all(|i| !blocks(camera + way * (i as f64 / steps as f64)))
        })
    }

    /// One word in the world, `age` seconds old: it pops in, glows, holds, then fades as it rises.
    #[allow(clippy::too_many_arguments)]
    fn lyric_word(
        &self,
        camera: glam::DVec3,
        word: &str,
        at: glam::DVec3,
        (right, up): (glam::Vec3, glam::Vec3),
        age: f32,
        size: f32,
        colour: u32,
        seed: u64,
        light: u32,
        out: &mut Vec<horizon_client::mesher::Vertex>,
        patches: &mut Vec<horizon_client::font::Patch>,
    ) {
        use horizon_client::text::Text;
        // In: a pop from small, overshooting. After a second: fading out over the rest, rising.
        let gone = ((age - HOLD) / (LIFE - HOLD)).clamp(0.0, 1.0);
        let pop = 0.25 + 0.75 * ease_out_back(age / 0.3);
        let alpha = (age / 0.12).min(1.0) * (1.0 - gone);
        let at = at
            + glam::DVec3::Y * f64::from(gone * 0.6 + 0.04 * (age * 2.0 + hash(seed) * 6.3).sin());
        let size = size * pop * (1.0 - 0.25 * gone);
        let o = at - camera;
        let a = |v: f32| ((v * alpha).clamp(0.0, 1.0) * 255.0) as u32;
        let (r, u) = (right.as_dvec3(), up.as_dvec3());
        // A glow in the word's colour: the word again all round it, a text pixel and two out,
        // fainter further; then the word itself, white tinted.
        // The glow a little behind the word, so the word is always over it.
        let behind = o.normalize_or_zero() * 0.03;
        for (reach, strength) in [(2.0, 0.07), (1.0, 0.2)] {
            for d in 0..8 {
                let angle = d as f64 * std::f64::consts::FRAC_PI_4;
                let shift = (r * angle.cos() + u * angle.sin()) * f64::from(size * reach) + behind;
                self.mark_translucent_entity(out.len(), "minecraft:name_tag");
                let halo = Text::colored(word, colour);
                patches.extend(self.oriented_text_quads(
                    o + shift,
                    &halo,
                    a(strength),
                    size,
                    (right, up),
                    0,
                    light,
                    out,
                ));
            }
        }
        self.mark_translucent_entity(out.len(), "minecraft:name_tag");
        let core = Text::colored(word, mix_white(colour, 0.75));
        patches.extend(self.oriented_text_quads(
            o,
            &core,
            a(1.0),
            size,
            (right, up),
            0,
            light,
            out,
        ));
        // A few sparks fly off a word as it pops.
        if age < 0.6 {
            let t = age / 0.6;
            for p in 0..5u64 {
                let angle = f64::from(hash(seed * 5 + p + 99) * std::f32::consts::TAU);
                let reach = f64::from(size) * (14.0 + 22.0 * f64::from(t));
                let spark =
                    o + r * (angle.cos() * reach) + u * (angle.sin() * reach + f64::from(0.3 * t));
                let star = Text::colored("✦", colour);
                let sa = ((1.0 - t) * 220.0) as u32;
                self.mark_translucent_entity(out.len(), "minecraft:name_tag");
                patches.extend(self.oriented_text_quads(
                    spark,
                    &star,
                    sa,
                    size * 0.5 * (1.0 - 0.5 * t),
                    (right, up),
                    0,
                    light,
                    out,
                ));
            }
        }
    }

    /// Whether the world is what the player looks at (no screen over it).
    fn screen_shows_world(&self) -> bool {
        self.screen == super::Screen::Playing && self.title.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyrics_spread_at_a_regular_step_are_not_timed_on_the_song() {
        let lrc = |times: &[u64]| {
            let text: String = times
                .iter()
                .map(|t| {
                    format!(
                        "[{:02}:{:05.2}] la\n",
                        t / 60_000,
                        (t % 60_000) as f64 / 1000.0
                    )
                })
                .collect();
            parse_lrc(&text)
        };
        // Plain lyrics spread from 0 by a few round steps (LRCLIB's for "Love Me Not").
        let steps = [3_400, 3_600, 4_200, 4_800];
        let spread: Vec<u64> = (0..30)
            .scan(0, |t, i| {
                let at = *t;
                *t += steps[i % 4];
                Some(at)
            })
            .collect();
        assert!(!lrc(&spread).timed_on_the_song());
        // The song's own: after its intro, each gap its own.
        let sung: Vec<u64> = (0..30)
            .map(|i| 16_880 + i * 2_000 + (i * i * 37) % 900)
            .collect();
        assert!(lrc(&sung).timed_on_the_song());
        // A few lines can't tell: kept.
        assert!(lrc(&[0, 5_000, 10_000]).timed_on_the_song());
    }

    #[test]
    fn lrc_lines_are_read_in_time_order() {
        let l = parse_lrc(
            "[ar: Someone]\n[00:12.50] second\n[00:01.00][01:00.00] first and again\n[00:20.00]\nno time here",
        );
        let times: Vec<u64> = l.lines.iter().map(|(t, _)| *t).collect();
        assert_eq!(times, vec![1_000, 12_500, 20_000, 60_000]);
        assert_eq!(l.lines[0].1, "first and again");
        assert_eq!(l.lines[2].1, "", "an empty line is a pause");
        assert_eq!(l.at(500), None);
        assert_eq!(l.at(1_000), Some(0));
        assert_eq!(l.at(15_000), Some(1));
        assert_eq!(l.at(99_000), Some(3));
    }

    #[test]
    fn words_pop_on_their_own_beat_or_spread_over_the_line() {
        let l = parse_lrc(
            "[00:10.00] <00:10.00>one <00:10.50>two three\n[00:20.00] four five\n[00:30.00]x",
        );
        assert_eq!(
            l.word_times(0),
            vec![
                (10_000, "one".into()),
                (10_500, "two".into()),
                (10_750, "three".into())
            ]
        );
        let spread = l.word_times(1);
        assert_eq!(spread[0], (20_000, "four".into()));
        // "four" (4 letters and a gap) sung at the pace: "five" waits for it.
        assert_eq!(
            spread[1],
            (20_450, "five".into()),
            "when it is sung, not before"
        );
        // A slow line spreads over its whole time, but no word outlasts the next line.
        let slow = parse_lrc("[00:01.00] a b\n[00:01.30]x");
        assert!(slow.word_times(0).iter().all(|(t, _)| *t < 1_300));
        assert!((0..1000).all(|s| (0.0..1.0).contains(&hash(s))));
    }

    #[test]
    fn another_try_is_another_place_nearer() {
        let camera = glam::DVec3::ZERO;
        let first = spawn_word("x".into(), 3, 2, camera, (0.0, 0.0), 0.0, 0, 6.0);
        let later = spawn_word("x".into(), 3, 2, camera, (0.0, 0.0), 0.0, 11, 6.0);
        assert_ne!(first.at, later.at);
        assert!(later.at.with_y(0.0).length() < 9.0 * 0.45 + 1e-6);
    }

    #[test]
    fn words_appear_ahead_at_a_distance_facing_the_player() {
        let camera = glam::DVec3::new(10.0, 70.0, -4.0);
        for line in 0..10 {
            for k in 0..8 {
                let w = spawn_word("x".into(), line, k, camera, (30.0, 0.0), 0.0, 0, 6.0);
                let d = w.at - camera;
                let flat = d.x.hypot(d.z);
                assert!((3.4..=9.1).contains(&flat), "{flat} away");
                // Ahead: within 50 degrees of the look (yaw 30).
                let look =
                    glam::DVec3::new(-(30f64.to_radians().sin()), 0.0, 30f64.to_radians().cos());
                assert!(d.with_y(0.0).normalize().dot(look) > 50f64.to_radians().cos() - 1e-6);
                // Its face towards the player, more or less, and upright enough to read.
                let normal = w.right.cross(w.up).as_dvec3();
                assert!(normal.dot(-d.normalize()) > 0.6, "seen from behind");
                assert!(w.up.y > 0.8);
            }
        }
    }

    #[test]
    fn an_elision_is_one_word_with_the_next() {
        let l = parse_lrc("[00:01.00] <00:01.00>C' <00:01.20>est l\u{2019} amour qu' on anniv' et");
        let words: Vec<String> = l.word_times(0).into_iter().map(|(_, w)| w).collect();
        assert_eq!(words, ["C'est", "l'amour", "qu'on", "anniv'", "et"]);
        assert_eq!(
            l.word_times(0)[0].0,
            1_000,
            "it pops when the first part does"
        );
    }

    #[test]
    fn each_words_own_time_is_left_out() {
        let l = parse_lrc("[00:18.89] <00:18.89>We're <00:19.25>no <00:19.53>strangers <3 <x>");
        assert_eq!(l.lines[0].1, "We're no strangers <3 <x>");
    }

    #[test]
    fn a_paused_song_stays_where_it_is() {
        let t = Track {
            position: 5_000,
            length: 6_000,
            playing: false,
            at: Some(Instant::now()),
            ..Track::default()
        };
        assert_eq!(t.now(), 5_000);
        let t = Track {
            position: 5_990,
            length: 6_000,
            playing: true,
            at: Instant::now().checked_sub(Duration::from_secs(1)),
            ..Track::default()
        };
        assert_eq!(t.now(), 6_000, "never past its end");
    }

    /// Reads this computer's media controls once: `cargo test -p horizon-client --lib
    /// reads_media_controls -- --ignored --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn reads_media_controls() {
        use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Manager;
        let m = Manager::RequestAsync()
            .and_then(|op| op.join())
            .expect("media controls");
        let s = media::pick_for_test(&m);
        println!("session: {}", s.is_some());
        if let Some(t) = s.as_ref().and_then(media::read_for_test) {
            println!(
                "{} - {} [{} / {} ms] playing: {} spotify: {}",
                t.title,
                t.artist,
                t.now(),
                t.length,
                t.playing,
                t.spotify
            );
            let cover = s.as_ref().and_then(media::cover_for_test);
            println!(
                "cover: {:?} shuffle: {} repeat: {}",
                cover.map(|c| c.len()),
                t.shuffle,
                t.repeat
            );
        }
    }

    /// Asks the real LRCLIB (network): `cargo test -p horizon-client --lib asks_lrclib -- --ignored`.
    #[test]
    #[ignore]
    fn asks_lrclib() {
        let t = Track {
            title: "Never Gonna Give You Up".into(),
            artist: "Rick Astley".into(),
            length: 213_000,
            ..Track::default()
        };
        let l = fetch_lyrics(&t).expect("lyrics");
        assert!(l.lines.len() > 20);
        println!(
            "{} lines, first at {} ms: {}",
            l.lines.len(),
            l.lines[0].0,
            l.lines[0].1
        );
    }
}
