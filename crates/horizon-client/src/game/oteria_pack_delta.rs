//! Oteria: fast server resource pack downloads.
//!
//! Vanilla (`HttpUtil.downloadFile`, kept as [`download::download_file`])
//! fetches a pack as one stream on a fresh connection every time the server
//! names no hash, or whenever the named hash isn't cached yet. This does three
//! things on top of the same `downloads/<pack id>/<SHA-1>` cache, in order:
//!
//! 1. **A conditional request.** The last download of a pack id remembers the
//!    server's `ETag` / `Last-Modified`; the next one asks with
//!    `If-None-Match` / `If-Modified-Since`, and a `304 Not Modified` reuses the
//!    file on disk with no body at all.
//! 2. **Only what changed.** When the pack did change, its central directory
//!    (the zip's tail) is compared with the last download: the byte spans of
//!    the changed or new entries are sorted, merged when they are close, and
//!    fetched as a handful of Range requests in parallel on pooled keep-alive
//!    connections; unchanged entries are raw-copied from the file on disk.
//! 3. **Parallel segments.** A full download (first time, most of the pack
//!    changed, or a pack with a declared hash not in the cache) is split into
//!    segments fetched at once over several connections, so a CDN that paces
//!    each connection doesn't pace the whole pack.
//!
//! Progress goes to the same [`download::Progress`] channel as vanilla's, so
//! the loading screen's bar moves during every path. Any failure (no Range
//! support, an odd zip, a short answer) falls back to the plain
//! [`download::download_file`], so correctness never depends on this module;
//! a declared hash is always checked before the file is used.
use super::super::server_packs::{Batch, Request, download};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    fs,
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::{Duration, Instant, SystemTime},
};

const NET_TIMEOUT: Duration = Duration::from_secs(30);
/// The suffix fetched first: the end record and, nearly always, the whole
/// central directory with it. Tiny in tests so a fixture still needs it.
#[cfg(not(test))]
const TAIL_GUESS: u64 = 1 << 20;
#[cfg(test)]
const TAIL_GUESS: u64 = 64;
/// A second, bigger tail when the central directory didn't fit the first.
const TAIL_RETRY: u64 = 16 << 20;
/// Connections used at once for one pack.
const CONNECTIONS: usize = 6;
/// Below this, one stream is as fast as segments.
#[cfg(not(test))]
const SEGMENTED_MIN: u64 = 2 << 20;
#[cfg(test)]
const SEGMENTED_MIN: u64 = 256;
/// Changed entries closer than this are fetched as one range: one request
/// with a few unchanged bytes in it beats two round trips.
const MERGE_GAP: u64 = 64 << 10;
/// Past this share of the pack changed, a segmented full download is simpler
/// and about as fast.
const WORTHWHILE_CHANGE: f64 = 0.6;
/// How often the loading bar hears about progress.
const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// One entry's identity across two downloads of the same pack id.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct EntryKey {
    name: String,
    crc32: u32,
    size: u64,
    compressed_size: u64,
}

/// What a pack id's last successful download looked like.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Manifest {
    /// The full pack file on disk (in `downloads/<id>/`).
    previous_file: PathBuf,
    entries: Vec<EntryKey>,
    /// The URL it came from: validators only mean something for the same one.
    #[serde(default)]
    url: String,
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
}

/// Beside `downloads/`, never inside it, so vanilla's cache cleaner
/// ([`download::vacuum`]) never counts or deletes them.
fn delta_dir(downloads_dir: &Path) -> PathBuf {
    downloads_dir
        .parent()
        .unwrap_or(downloads_dir)
        .join("oteria-pack-delta")
}
fn manifest_path(delta_dir: &Path, id: u128) -> PathBuf {
    delta_dir.join(format!("{id:032x}.json"))
}
fn load_manifest(delta_dir: &Path, id: u128) -> Option<Manifest> {
    serde_json::from_slice(&fs::read(manifest_path(delta_dir, id)).ok()?).ok()
}
fn save_manifest(delta_dir: &Path, id: u128, manifest: &Manifest) {
    if let Err(e) = fs::create_dir_all(delta_dir) {
        log::warn!("Horizon: could not create {}: {e}", delta_dir.display());
        return;
    }
    let path = manifest_path(delta_dir, id);
    match serde_json::to_vec(manifest) {
        Ok(bytes) => {
            if let Err(e) = fs::write(&path, bytes) {
                log::warn!("Horizon: could not save {}: {e}", path.display());
            }
        }
        Err(e) => log::warn!("Horizon: could not encode {}: {e}", path.display()),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn sha1_of(path: &Path) -> io::Result<String> {
    let mut hasher = Sha1::new();
    let mut file = io::BufReader::with_capacity(1 << 20, fs::File::open(path)?);
    io::copy(&mut file, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}
/// `updateModificationTime`: used now, so the cleaner keeps it longest.
fn touch(path: &Path) {
    let _ = fs::File::options()
        .append(true)
        .open(path)
        .and_then(|f| f.set_modified(SystemTime::now()));
}
fn temp_name() -> String {
    use std::hash::{BuildHasher, RandomState};
    format!(
        "download{:016x}.tmp",
        RandomState::new().hash_one(Instant::now())
    )
}

/// The connections of one download thread: kept alive and reused across
/// requests and packs, instead of a new TCP + TLS handshake for each.
#[derive(Clone)]
struct Net {
    agent: ureq::Agent,
    headers: Vec<(&'static str, String)>,
}
impl Net {
    fn new(agent: ureq::Agent, headers: &[(&'static str, String)]) -> Self {
        Self {
            agent,
            headers: headers.to_vec(),
        }
    }
    fn get(&self, url: &str) -> ureq::Request {
        let mut call = self.agent.get(url);
        for (name, value) in &self.headers {
            call = call.set(name, value);
        }
        call
    }
    /// Bytes `start..end` (exclusive) of `url`, counted into `counter` as
    /// they arrive. Fails unless the server answers exactly that range.
    fn range(&self, url: &str, start: u64, end: u64, counter: &AtomicU64) -> io::Result<Vec<u8>> {
        let response = self
            .get(url)
            .set("Range", &format!("bytes={start}-{}", end - 1))
            .call()
            .map_err(io::Error::other)?;
        let answered = response
            .header("Content-Range")
            .and_then(|r| r.strip_prefix("bytes "))
            .and_then(|r| r.split('-').next())
            .and_then(|s| s.trim().parse::<u64>().ok());
        if response.status() != 206 || answered != Some(start) {
            return Err(io::Error::other(
                "the server did not answer the Range request as asked",
            ));
        }
        let want = (end - start) as usize;
        let mut body = Vec::with_capacity(want);
        let mut reader = response.into_reader().take(want as u64);
        let mut buffer = vec![0u8; 256 << 10];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&buffer[..n]);
            counter.fetch_add(n as u64, Ordering::Relaxed);
        }
        if body.len() != want {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "short range response",
            ));
        }
        Ok(body)
    }
}
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(NET_TIMEOUT)
        .timeout_read(NET_TIMEOUT)
        .max_idle_connections_per_host(CONNECTIONS + 2)
        .build()
}

/// Runs `work` while sending the bytes counted so far to the loading bar.
fn with_progress<T: Send>(
    counter: &AtomicU64,
    progress: &Sender<download::Progress>,
    work: impl FnOnce() -> T + Send,
) -> T {
    std::thread::scope(|s| {
        let handle = s.spawn(work);
        let mut sent = u64::MAX;
        while !handle.is_finished() {
            let now = counter.load(Ordering::Relaxed);
            if now != sent {
                let _ = progress.send(download::Progress::Bytes(now));
                sent = now;
            }
            std::thread::sleep(PROGRESS_EVERY);
        }
        let _ = progress.send(download::Progress::Bytes(counter.load(Ordering::Relaxed)));
        handle.join().expect("download worker panicked")
    })
}

/// Fetches `ranges` (start, end exclusive) on up to [`CONNECTIONS`] at once,
/// each result in the order asked.
fn fetch_all(
    net: &Net,
    url: &str,
    ranges: &[(u64, u64)],
    counter: &AtomicU64,
) -> io::Result<Vec<Vec<u8>>> {
    let next = AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<io::Result<Vec<u8>>>>> =
        ranges.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..CONNECTIONS.min(ranges.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(start, end)) = ranges.get(i) else {
                        break;
                    };
                    let result = net.range(url, start, end, counter);
                    let failed = result.is_err();
                    *results[i].lock().unwrap() = Some(result);
                    if failed {
                        // The others stop after their current range.
                        next.store(ranges.len(), Ordering::Relaxed);
                        break;
                    }
                }
            });
        }
    });
    results
        .into_iter()
        .map(|r| {
            r.into_inner()
                .unwrap()
                .unwrap_or_else(|| Err(io::Error::other("a range was never fetched")))
        })
        .collect()
}

/// What the first request learned: the tail of the zip and its size, or that
/// nothing changed, or (a server ignoring `Range`) the whole body already.
enum Probe {
    NotModified,
    Partial {
        len: u64,
        tail: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    Whole(ureq::Response),
}
fn probe(net: &Net, url: &str, previous: Option<&Manifest>) -> io::Result<Probe> {
    let mut call = net.get(url).set("Range", &format!("bytes=-{TAIL_GUESS}"));
    if let Some(m) = previous {
        if let Some(etag) = &m.etag {
            call = call.set("If-None-Match", etag);
        }
        if let Some(date) = &m.last_modified {
            call = call.set("If-Modified-Since", date);
        }
    }
    let response = call.call().map_err(io::Error::other)?;
    match response.status() {
        304 => Ok(Probe::NotModified),
        206 => {
            let len = response
                .header("Content-Range")
                .and_then(|r| r.rsplit('/').next())
                .and_then(|s| s.trim().parse::<u64>().ok())
                .ok_or_else(|| io::Error::other("no total in Content-Range"))?;
            if len > download::MAX_SIZE {
                return Err(io::Error::other("the pack is bigger than the download limit"));
            }
            let etag = response.header("ETag").map(str::to_owned);
            let last_modified = response.header("Last-Modified").map(str::to_owned);
            let mut tail = Vec::new();
            response
                .into_reader()
                .take(TAIL_GUESS + 1)
                .read_to_end(&mut tail)?;
            Ok(Probe::Partial {
                len,
                tail,
                etag,
                last_modified,
            })
        }
        200 => Ok(Probe::Whole(response)),
        status => Err(io::Error::other(format!("unexpected HTTP status {status}"))),
    }
}

/// A remote zip of which only some byte spans are known: enough for `zip`
/// to read the central directory and the entries that were fetched. Reading
/// anywhere else is an error, never a hidden request.
#[derive(Clone)]
struct Sparse {
    len: u64,
    /// (start, bytes), sorted by start; shared so spans fetched after the
    /// central directory was read still reach the archive's own copy.
    spans: Arc<std::sync::Mutex<Vec<(u64, Arc<[u8]>)>>>,
    position: u64,
}
impl Sparse {
    fn new(len: u64, start: u64, bytes: Vec<u8>) -> Self {
        Self {
            len,
            spans: Arc::new(std::sync::Mutex::new(vec![(start, Arc::from(bytes))])),
            position: 0,
        }
    }
    fn add(&self, start: u64, bytes: Vec<u8>) {
        let mut spans = self.spans.lock().unwrap();
        spans.push((start, Arc::from(bytes)));
        spans.sort_by_key(|s| s.0);
    }
}
impl Read for Sparse {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.position >= self.len {
            return Ok(0);
        }
        let at = self.position;
        let (span, first) = {
            let spans = self.spans.lock().unwrap();
            let span = spans
                .iter()
                .rev()
                .find(|(start, bytes)| *start <= at && at < start + bytes.len() as u64)
                .cloned();
            (span, spans.first().map_or(self.len, |s| s.0))
        };
        let Some((start, bytes)) = span else {
            // Before every fetched span, zeros: `zip` looks for the end record in a block
            // reaching back before the tail. No entry read here ever lies there (the changed
            // ones are fetched first); a gap between spans stays an error.
            if at < first {
                let n = out.len().min((first - at) as usize);
                out[..n].fill(0);
                self.position += n as u64;
                return Ok(n);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "read outside the fetched spans",
            ));
        };
        let rest = &bytes[(at - start) as usize..];
        let n = rest.len().min(out.len());
        out[..n].copy_from_slice(&rest[..n]);
        self.position += n as u64;
        Ok(n)
    }
}
impl Seek for Sparse {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let position = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(by) => self.len.checked_add_signed(by),
            SeekFrom::Current(by) => self.position.checked_add_signed(by),
        };
        self.position = position.ok_or_else(|| io::Error::other("seek before the start"))?;
        Ok(self.position)
    }
}

/// Where the central directory starts, from the end-of-central-directory record in the tail
/// fetched at `tail_at` (its offset field, `PK\x05\x06` + 16). `None` without one in the tail,
/// or for a Zip64 archive (the field is `0xFFFFFFFF` there).
fn directory_start(sparse: &Sparse, tail_at: u64) -> Option<u64> {
    let spans = sparse.spans.lock().ok()?;
    let (_, tail) = spans.iter().find(|(start, _)| *start == tail_at)?;
    let at = tail.windows(4).rposition(|w| w == [0x50, 0x4B, 0x05, 0x06])?;
    let field = tail.get(at + 16..at + 20)?;
    let offset = u32::from_le_bytes(field.try_into().ok()?);
    (offset != u32::MAX).then_some(u64::from(offset))
}

/// Every entry's identity in the zip at `path`, read locally.
fn snapshot(path: &Path) -> io::Result<Vec<EntryKey>> {
    let archive = zip::ZipArchive::new(io::BufReader::new(fs::File::open(path)?))
        .map_err(io::Error::other)?;
    (0..archive.len())
        .map(|i| {
            let entry = archive.by_index_data(i).map_err(io::Error::other)?;
            Ok(EntryKey {
                name: entry.name().map_err(io::Error::other)?.into_owned(),
                crc32: entry.crc32(),
                size: entry.size(),
                compressed_size: entry.compressed_size(),
            })
        })
        .collect()
}

/// Moves a finished temporary file to `<dir>/<its SHA-1>` (checked against a
/// declared hash), reusing an identical file already there.
fn settle(dir: &Path, tmp: &Path, declared: Option<&str>) -> io::Result<PathBuf> {
    let actual = sha1_of(tmp)?;
    if let Some(sha1) = declared
        && actual != sha1
    {
        let _ = fs::remove_file(tmp);
        return Err(io::Error::other(format!(
            "Hash of downloaded file ({actual}) did not match requested ({sha1})"
        )));
    }
    let path = dir.join(&actual);
    if path.exists() {
        let _ = fs::remove_file(tmp);
        touch(&path);
    } else {
        fs::rename(tmp, &path)?;
    }
    Ok(path)
}

/// The whole pack, `len` bytes, in parallel segments written in place.
fn segmented(
    net: &Net,
    url: &str,
    len: u64,
    dir: &Path,
    declared: Option<&str>,
    progress: &Sender<download::Progress>,
) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(temp_name());
    let file = fs::File::create(&tmp)?;
    file.set_len(len)?;
    drop(file);
    let _ = progress.send(download::Progress::DownloadStart(Some(len)));
    let counter = AtomicU64::new(0);
    let parts = CONNECTIONS as u64;
    let step = len.div_ceil(parts).max(1);
    let ranges: Vec<(u64, u64)> = (0..parts)
        .map(|i| (i * step, ((i + 1) * step).min(len)))
        .filter(|(a, b)| a < b)
        .collect();
    let result = with_progress(&counter, progress, || -> io::Result<()> {
        std::thread::scope(|s| {
            let handles: Vec<_> = ranges
                .iter()
                .map(|&(start, end)| {
                    let (tmp, counter) = (&tmp, &counter);
                    s.spawn(move || -> io::Result<()> {
                        let response = net
                            .get(url)
                            .set("Range", &format!("bytes={start}-{}", end - 1))
                            .call()
                            .map_err(io::Error::other)?;
                        if response.status() != 206 {
                            return Err(io::Error::other("no Range support for a segment"));
                        }
                        let mut out = fs::OpenOptions::new().write(true).open(tmp)?;
                        out.seek(SeekFrom::Start(start))?;
                        let mut out = BufWriter::with_capacity(1 << 20, out);
                        let mut reader = response.into_reader().take(end - start);
                        let mut buffer = vec![0u8; 256 << 10];
                        let mut got = 0u64;
                        loop {
                            let n = reader.read(&mut buffer)?;
                            if n == 0 {
                                break;
                            }
                            out.write_all(&buffer[..n])?;
                            got += n as u64;
                            counter.fetch_add(n as u64, Ordering::Relaxed);
                        }
                        out.flush()?;
                        if got != end - start {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "short segment",
                            ));
                        }
                        Ok(())
                    })
                })
                .collect();
            handles
                .into_iter()
                .try_for_each(|h| h.join().expect("segment worker panicked"))
        })
    });
    match result {
        Ok(()) => settle(dir, &tmp, declared),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// A `200 OK` body (a server that ignores `Range`), streamed to disk.
fn stream(
    response: ureq::Response,
    dir: &Path,
    declared: Option<&str>,
    progress: &Sender<download::Progress>,
) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let size = response
        .header("Content-Length")
        .and_then(|v| v.trim().parse::<u64>().ok());
    if size.is_some_and(|s| s > download::MAX_SIZE) {
        return Err(io::Error::other("the pack is bigger than the download limit"));
    }
    let _ = progress.send(download::Progress::DownloadStart(size));
    let tmp = dir.join(temp_name());
    let counter = AtomicU64::new(0);
    let result = with_progress(&counter, progress, || -> io::Result<()> {
        let mut out = BufWriter::with_capacity(1 << 20, fs::File::create(&tmp)?);
        let mut reader = response.into_reader().take(download::MAX_SIZE + 1);
        let mut buffer = vec![0u8; 256 << 10];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            out.write_all(&buffer[..n])?;
            if counter.fetch_add(n as u64, Ordering::Relaxed) + n as u64 > download::MAX_SIZE {
                return Err(io::Error::other("the pack is bigger than the download limit"));
            }
        }
        out.flush()
    });
    match result {
        Ok(()) => settle(dir, &tmp, declared),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// The pack rebuilt from `previous` plus the changed entries' spans, or an
/// error when that isn't possible or not worth it.
#[allow(clippy::too_many_arguments)]
fn delta(
    net: &Net,
    url: &str,
    len: u64,
    tail: Vec<u8>,
    previous: &Manifest,
    dir: &Path,
    progress: &Sender<download::Progress>,
) -> io::Result<(PathBuf, Vec<EntryKey>)> {
    let mut old = zip::ZipArchive::new(io::BufReader::new(fs::File::open(
        &previous.previous_file,
    )?))
    .map_err(io::Error::other)?;
    let known: HashMap<&str, &EntryKey> = previous
        .entries
        .iter()
        .map(|e| (e.name.as_str(), e))
        .collect();

    let tail_at = len - tail.len() as u64;
    let sparse = Sparse::new(len, tail_at, tail);
    let mut live = match zip::ZipArchive::new(sparse.clone()) {
        Ok(live) => live,
        Err(_) if tail_at > 0 => {
            // The central directory starts before the first tail: its start from the end
            // record when the tail holds one, else a bigger tail.
            let start = directory_start(&sparse, tail_at)
                .filter(|&s| s < tail_at)
                .unwrap_or_else(|| len.saturating_sub(TAIL_RETRY));
            let bytes = net.range(url, start, tail_at, &AtomicU64::new(0))?;
            sparse.add(start, bytes);
            zip::ZipArchive::new(sparse.clone()).map_err(io::Error::other)?
        }
        Err(e) => return Err(io::Error::other(e)),
    };

    // Each entry's span runs from its local header to the next one's (or the
    // central directory): header, data and data descriptor.
    let mut keys = Vec::with_capacity(live.len());
    let mut starts = Vec::with_capacity(live.len());
    let mut directory = len;
    for i in 0..live.len() {
        let entry = live.by_index_data(i).map_err(io::Error::other)?;
        keys.push(EntryKey {
            name: entry.name().map_err(io::Error::other)?.into_owned(),
            crc32: entry.crc32(),
            size: entry.size(),
            compressed_size: entry.compressed_size(),
        });
        starts.push(entry.header_start());
        directory = directory.min(entry.central_header_start());
    }
    let mut sorted: Vec<u64> = starts.clone();
    sorted.sort_unstable();
    sorted.dedup();
    let span_end = |start: u64| {
        let i = sorted.partition_point(|&s| s <= start);
        sorted.get(i).copied().unwrap_or(directory).min(directory)
    };
    let mut wanted: Vec<(u64, u64)> = keys
        .iter()
        .zip(&starts)
        .filter(|(key, _)| known.get(key.name.as_str()) != Some(key))
        .map(|(_, &start)| (start, span_end(start)))
        .filter(|(a, b)| a < b)
        .collect();
    wanted.sort_unstable();
    let mut ranges: Vec<(u64, u64)> = Vec::new();
    for (start, end) in wanted {
        match ranges.last_mut() {
            Some(last) if start <= last.1 + MERGE_GAP => last.1 = last.1.max(end),
            _ => ranges.push((start, end)),
        }
    }
    let fetch: u64 = ranges.iter().map(|(a, b)| b - a).sum();
    if fetch as f64 > len as f64 * WORTHWHILE_CHANGE {
        return Err(io::Error::other("most of the pack changed"));
    }

    let _ = progress.send(download::Progress::DownloadStart(Some(fetch)));
    let counter = AtomicU64::new(0);
    let bodies = with_progress(&counter, progress, || {
        fetch_all(net, url, &ranges, &counter)
    })?;
    for (&(start, _), body) in ranges.iter().zip(bodies) {
        sparse.add(start, body);
    }

    fs::create_dir_all(dir)?;
    let tmp = dir.join(temp_name());
    let built = (|| -> io::Result<()> {
        let out = BufWriter::with_capacity(1 << 20, fs::File::create(&tmp)?);
        let mut writer = zip::ZipWriter::new(out);
        for (i, key) in keys.iter().enumerate() {
            if known.get(key.name.as_str()) == Some(&key) {
                let index = old.index_for_name(&key.name).ok_or_else(|| {
                    io::Error::other("an unchanged entry vanished from the cached file")
                })?;
                let file = old.by_index_raw(index).map_err(io::Error::other)?;
                writer.raw_copy_file(file).map_err(io::Error::other)?;
            } else {
                let file = live.by_index_raw(i).map_err(io::Error::other)?;
                writer.raw_copy_file(file).map_err(io::Error::other)?;
            }
        }
        writer.finish().map_err(io::Error::other)?.flush()
    })();
    if let Err(e) = built {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    log::info!(
        "Oteria: pack {url} rebuilt from the cache, {fetch} of {len} bytes fetched in {} ranges",
        ranges.len()
    );
    Ok((settle(dir, &tmp, None)?, keys))
}

/// The fast path for one request; errors fall back to vanilla's download.
fn fast(
    net: &Net,
    downloads_dir: &Path,
    request: &Request,
    progress: &Sender<download::Progress>,
) -> io::Result<PathBuf> {
    let dir = downloads_dir.join(horizon_protocol::crypto::uuid_string(request.id));
    let delta_dir = delta_dir(downloads_dir);
    // A declared hash already on disk: vanilla's own cache hit, no network.
    if let Some(sha1) = &request.sha1 {
        let path = dir.join(sha1);
        if path.exists() && sha1_of(&path).is_ok_and(|actual| &actual == sha1) {
            log::info!("Horizon: returning cached file since actual hash matches requested");
            touch(&path);
            return Ok(path);
        }
    }
    let previous = load_manifest(&delta_dir, request.id)
        .filter(|m| m.previous_file.is_file())
        .filter(|_| request.sha1.is_none());
    let conditional = previous.as_ref().filter(|m| m.url == request.url);
    let started = Instant::now();
    let (path, etag, last_modified, entries) = match probe(net, &request.url, conditional)? {
        Probe::NotModified => {
            let Some(m) = conditional else {
                return Err(io::Error::other("304 Not Modified to a plain request"));
            };
            touch(&m.previous_file);
            log::info!(
                "Oteria: pack {} not modified, reused from the cache",
                request.url
            );
            let size = fs::metadata(&m.previous_file).map(|m| m.len()).unwrap_or(0);
            let _ = progress.send(download::Progress::DownloadStart(Some(size)));
            let _ = progress.send(download::Progress::Bytes(size));
            return Ok(m.previous_file.clone());
        }
        Probe::Whole(response) => {
            let etag = response.header("ETag").map(str::to_owned);
            let last_modified = response.header("Last-Modified").map(str::to_owned);
            let path = stream(response, &dir, request.sha1.as_deref(), progress)?;
            (path, etag, last_modified, None)
        }
        Probe::Partial {
            len,
            tail,
            etag,
            last_modified,
        } if tail.len() as u64 == len => {
            // A pack smaller than the first request: it already came whole.
            fs::create_dir_all(&dir)?;
            let tmp = dir.join(temp_name());
            fs::write(&tmp, &tail)?;
            let _ = progress.send(download::Progress::DownloadStart(Some(len)));
            let _ = progress.send(download::Progress::Bytes(len));
            let path = settle(&dir, &tmp, request.sha1.as_deref())?;
            (path, etag, last_modified, None)
        }
        Probe::Partial {
            len,
            tail,
            etag,
            last_modified,
        } => {
            let rebuilt = previous.as_ref().and_then(|m| {
                delta(net, &request.url, len, tail, m, &dir, progress)
                    .inspect_err(|e| log::debug!("Oteria: no partial pack download: {e}"))
                    .ok()
            });
            match rebuilt {
                Some((path, entries)) => (path, etag, last_modified, Some(entries)),
                None if len >= SEGMENTED_MIN => {
                    let path =
                        segmented(net, &request.url, len, &dir, request.sha1.as_deref(), progress)?;
                    (path, etag, last_modified, None)
                }
                None => {
                    let response = net.get(&request.url).call().map_err(io::Error::other)?;
                    let path = stream(response, &dir, request.sha1.as_deref(), progress)?;
                    (path, etag, last_modified, None)
                }
            }
        }
    };
    log::info!(
        "Oteria: pack {} ready in {} ms",
        request.url,
        started.elapsed().as_millis()
    );
    if request.sha1.is_none() {
        let entries = match entries {
            Some(entries) => Some(entries),
            None => snapshot(&path).ok(),
        };
        if let Some(entries) = entries {
            save_manifest(
                &delta_dir,
                request.id,
                &Manifest {
                    previous_file: path.clone(),
                    entries,
                    url: request.url.clone(),
                    etag,
                    last_modified,
                },
            );
        }
    }
    Ok(path)
}

fn download_one(
    net: &Net,
    downloads_dir: &Path,
    request: &Request,
    progress: &Sender<download::Progress>,
) -> Result<PathBuf, String> {
    match fast(net, downloads_dir, request, progress) {
        Ok(path) => return Ok(path),
        Err(e) => log::warn!(
            "Oteria: fast download of {} failed ({e}), downloading it plainly",
            request.url
        ),
    }
    let id = horizon_protocol::crypto::uuid_string(request.id);
    download::download_file(
        &downloads_dir.join(&id),
        request,
        &net.headers,
        download::MAX_SIZE,
        progress,
    )
}

type Job = (
    Vec<Request>,
    Vec<(&'static str, String)>,
    Sender<download::Progress>,
);
fn run(
    agent: &ureq::Agent,
    dir: &Path,
    requests: &[Request],
    headers: &[(&'static str, String)],
    progress: &Sender<download::Progress>,
) -> Batch {
    let net = Net::new(agent.clone(), headers);
    let mut batch = Batch::default();
    for request in requests {
        let _ = progress.send(download::Progress::RequestStart);
        let result = download_one(&net, dir, request, progress);
        let _ = progress.send(download::Progress::RequestFinished(result.is_ok()));
        match result {
            Ok(path) => batch.downloaded.push((request.id, path)),
            Err(e) => {
                log::error!("Horizon: failed to download {}: {e}", request.url);
                batch.failed.push(request.id);
            }
        }
    }
    batch
}

/// [`download::Queue`]'s own shape, so `game/server_packs.rs` only swaps its
/// type: batches download in order, on one thread with one connection pool.
pub(super) struct DeltaQueue {
    jobs: Sender<Job>,
    done: Receiver<Batch>,
}
impl DeltaQueue {
    pub(super) fn open(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        // `DownloadQueue.MAX_KEPT_PACKS`, as `server_packs::download` keeps it.
        download::vacuum(&dir, 20);
        let (jobs, todo) = channel::<Job>();
        let (finished, done) = channel();
        std::thread::Builder::new()
            .name("pack-delta-queue".into())
            .spawn(move || {
                let agent = agent();
                for (requests, headers, progress) in todo {
                    if finished
                        .send(run(&agent, &dir, &requests, &headers, &progress))
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self { jobs, done })
    }
    pub(super) fn download(
        &self,
        requests: Vec<Request>,
        headers: Vec<(&'static str, String)>,
        progress: Sender<download::Progress>,
    ) {
        let _ = self.jobs.send((requests, headers, progress));
    }
    pub(super) fn finished(&self) -> Vec<Batch> {
        self.done.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn scratch() -> PathBuf {
        use std::hash::{BuildHasher, RandomState};
        let n = RandomState::new().hash_one(Instant::now());
        let dir = std::env::temp_dir().join(format!("oteria-pack-fetch-{n:016x}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for &(name, body) in files {
            w.start_file(name, options).unwrap();
            w.write_all(body).unwrap();
        }
        w.finish().unwrap().into_inner()
    }
    fn names_and_bytes(path: &Path) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut out: Vec<_> = (0..archive.len())
            .map(|i| {
                let mut file = archive.by_index(i).unwrap();
                let name = file.name().unwrap().into_owned();
                let mut body = Vec::new();
                file.read_to_end(&mut body).unwrap();
                (name, body)
            })
            .collect();
        out.sort();
        out
    }
    /// Bytes deflate can't shrink, so an entry's size stays measurable.
    fn filler(n: usize, seed: u8) -> Vec<u8> {
        let mut x = u32::from(seed).wrapping_mul(2_654_435_761) | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect()
    }

    /// Serves `body` with Range and `ETag` support on as many connections
    /// as come, until the test ends; counts the body bytes it sent.
    struct Server {
        url: String,
        sent: Arc<AtomicU64>,
    }
    fn serve(body: Vec<u8>, etag: &'static str, ranges: bool) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pack.zip", listener.local_addr().unwrap());
        let sent = Arc::new(AtomicU64::new(0));
        let body = Arc::new(body);
        let counter = sent.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let (body, counter) = (body.clone(), counter.clone());
                std::thread::spawn(move || {
                    let mut request = Vec::new();
                    let mut buf = [0u8; 4096];
                    while !request.ends_with(b"\r\n\r\n") {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => request.extend_from_slice(&buf[..n]),
                        }
                    }
                    let text = String::from_utf8_lossy(&request).to_ascii_lowercase();
                    let header = |name: &str| {
                        text.lines()
                            .find_map(|l| l.strip_prefix(name).map(|v| v.trim().to_owned()))
                    };
                    if header("if-none-match:").as_deref() == Some(etag) {
                        let _ = stream.write_all(
                            b"HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                        return;
                    }
                    let len = body.len() as u64;
                    let range = header("range: bytes=").filter(|_| ranges);
                    let partial = "206 Partial Content";
                    let (status, start, end) = match range {
                        None => ("200 OK", 0, len),
                        Some(r) => match r.strip_prefix('-') {
                            Some(n) => (partial, len - n.parse::<u64>().unwrap().min(len), len),
                            None => {
                                let (a, b) = r.split_once('-').unwrap();
                                let b = if b.is_empty() {
                                    len
                                } else {
                                    b.parse::<u64>().unwrap() + 1
                                };
                                (partial, a.parse().unwrap(), b.min(len))
                            }
                        },
                    };
                    let slice = &body[start as usize..end as usize];
                    let mut head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nETag: {etag}\r\nConnection: close\r\n",
                        slice.len()
                    );
                    if status == partial {
                        head += &format!("Content-Range: bytes {start}-{}/{len}\r\n", end - 1);
                    }
                    head += "\r\n";
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(slice);
                    counter.fetch_add(slice.len() as u64, Ordering::Relaxed);
                });
            }
        });
        Server { url, sent }
    }
    fn fetch(dir: &Path, id: u128, url: &str, sha1: Option<String>) -> io::Result<PathBuf> {
        let (tx, _rx) = channel();
        let net = Net::new(agent(), &[]);
        fast(
            &net,
            dir,
            &Request {
                id,
                url: url.to_owned(),
                sha1,
            },
            &tx,
        )
    }

    #[test]
    fn a_changed_pack_fetches_only_its_changed_entries() {
        let dir = scratch();
        let big = filler(40_000, 1);
        let old = zip_of(&[("a.png", &big), ("b.txt", b"old"), ("c.txt", b"gone")]);
        let server = serve(old, "\"v1\"", true);
        let first = fetch(&dir, 7, &server.url, None).unwrap();
        assert_eq!(names_and_bytes(&first).len(), 3);

        let new = zip_of(&[("a.png", &big), ("b.txt", b"it changed"), ("d.txt", b"new")]);
        let new_len = new.len() as u64;
        let server = serve(new, "\"v2\"", true);
        let second = fetch(&dir, 7, &server.url, None).unwrap();
        let sent = server.sent.load(Ordering::Relaxed);
        assert!(sent < new_len / 2, "a.png came again: {sent} of {new_len}");
        assert_eq!(
            names_and_bytes(&second),
            [
                ("a.png".to_owned(), big),
                ("b.txt".to_owned(), b"it changed".to_vec()),
                ("d.txt".to_owned(), b"new".to_vec()),
            ]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unmodified_pack_is_reused_without_its_body() {
        let dir = scratch();
        let body = zip_of(&[("a.png", &filler(20_000, 2))]);
        let server = serve(body, "\"same\"", true);
        let first = fetch(&dir, 3, &server.url, None).unwrap();
        let before = server.sent.load(Ordering::Relaxed);
        let second = fetch(&dir, 3, &server.url, None).unwrap();
        assert_eq!(first, second);
        assert_eq!(server.sent.load(Ordering::Relaxed), before, "304 sends no body");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_segmented_download_is_byte_identical_and_checks_the_hash() {
        let dir = scratch();
        let body = zip_of(&[("a.png", &filler(50_000, 3)), ("b.png", &filler(9_000, 4))]);
        let sha1 = hex(&Sha1::digest(&body));
        let server = serve(body.clone(), "\"x\"", true);
        let path = fetch(&dir, 9, &server.url, Some(sha1.clone())).unwrap();
        assert_eq!(fs::read(&path).unwrap(), body);
        assert!(path.ends_with(&sha1));
        assert!(fetch(&dir, 10, &server.url, Some("0".repeat(40))).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_server_without_ranges_still_downloads_in_one_pass() {
        let dir = scratch();
        let body = zip_of(&[("a.png", &filler(5_000, 5))]);
        let server = serve(body.clone(), "\"y\"", false);
        let path = fetch(&dir, 11, &server.url, None).unwrap();
        assert_eq!(fs::read(&path).unwrap(), body);
        assert_eq!(server.sent.load(Ordering::Relaxed), body.len() as u64);
        fs::remove_dir_all(dir).unwrap();
    }
}
