//! Oteria: incremental re-download of a server's resource pack.
//!
//! A server that gives no SHA-1 for its pack (`ResourcePackPush.hash` empty)
//! defeats vanilla's own cache: `server_packs::download` names the file after
//! the hash of whatever came back, so a pack rebuilt with new file
//! timestamps, even with every actual file unchanged, downloads again in
//! full on every reconnect. This re-downloads only the zip entries whose
//! *content* (name, CRC-32 and sizes) differs from the last time this pack
//! id was fetched, raw-copying the rest from the file already on disk, and
//! drops entries the server removed.
//!
//! Used only when the server gave no hash: with one, vanilla must end up
//! with the exact bytes it named, and a pack rebuilt by a different zip tool
//! run rarely reproduces the old bytes for an "unchanged" entry (a fresh
//! timestamp in its local header, say), so a mismatch there would only ever
//! fall back to a full download anyway. Entirely best-effort: any failure
//! here (a server with no Range support, a pack too odd to diff, almost
//! everything changed) falls back to [`download::download_file`] exactly as
//! before, so correctness never depends on this module.
use super::super::server_packs::{Batch, Request, download};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

const NET_TIMEOUT: Duration = Duration::from_secs(30);
/// The suffix fetched first to find the end-of-central-directory record and,
/// usually, the whole central directory with it. When a pack's directory is
/// bigger than this, [`HttpRangeReader`] simply fetches more as `zip` asks
/// for it; this only sizes the common case's single round trip. Tiny in
/// tests, so even a fixture of a few kilobytes still exercises that fetch,
/// rather than fitting whole in this first request.
#[cfg(not(test))]
const TAIL_GUESS: u64 = 1 << 20;
#[cfg(test)]
const TAIL_GUESS: u64 = 64;
/// A "before the tail" read that misses the window fetches at least this
/// much more, so a changed entry's data rarely needs a second request.
const WINDOW: u64 = 1 << 20;
/// Below this share of the pack's compressed bytes actually changed, the
/// per-entry requests are worth it; past it, one plain download is faster
/// than many small ones for little saved.
const WORTHWHILE_CHANGE: f64 = 0.7;

/// One entry's identity across two downloads of the same pack id: unchanged
/// content keeps every one of these (a rebuilt entry with new metadata but
/// the same bytes is still "changed" by this and simply re-fetched, safe but
/// not optimal).
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct EntryKey {
    name: String,
    crc32: u32,
    size: u64,
    compressed_size: u64,
}

/// What a pack id's last successful download looked like.
#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    /// The full pack file on disk (in `downloads/<id>/`) that an unchanged
    /// entry's bytes are raw-copied from.
    previous_file: PathBuf,
    entries: Vec<EntryKey>,
}

/// Where manifests live: beside `downloads/`, never inside it, so vanilla's
/// own cache cleaner ([`download::vacuum`]) never counts or deletes them.
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

/// A single Range request, as a fresh connection (like
/// [`download::download_file`]'s own requests): the status, the body, and
/// the archive's total length from `Content-Range` (or `Content-Length` when
/// the server ignored the range and sent the whole thing, status 200).
fn range_get(
    url: &str,
    headers: &[(&'static str, String)],
    range: &str,
) -> io::Result<(u16, Vec<u8>, Option<u64>)> {
    let mut call = ureq::AgentBuilder::new()
        .timeout_connect(NET_TIMEOUT)
        .timeout_read(NET_TIMEOUT)
        .build()
        .get(url)
        .set("Range", range);
    for (name, value) in headers {
        call = call.set(name, value);
    }
    let response = call.call().map_err(io::Error::other)?;
    let status = response.status();
    let total = response
        .header("Content-Range")
        .and_then(|r| r.rsplit('/').next())
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            (status == 200)
                .then(|| response.header("Content-Length"))
                .flatten()
                .and_then(|s| s.parse().ok())
        });
    let mut body = Vec::new();
    response
        .into_reader()
        .take(download::MAX_SIZE + 1)
        .read_to_end(&mut body)
        .map_err(io::Error::other)?;
    Ok((status, body, total))
}

/// A remote zip, read through Range requests: the tail (central directory
/// and end record) fetched once, everything earlier fetched in a sliding
/// window as `zip` asks for it. `Seek`/`Read` so `zip::ZipArchive` can read
/// it exactly as a local file, parsing local headers itself: this never
/// hand-parses the zip format.
struct HttpRangeReader {
    url: String,
    headers: Vec<(&'static str, String)>,
    len: u64,
    tail: Arc<[u8]>,
    tail_at: u64,
    window: Option<(u64, Vec<u8>)>,
    position: u64,
}
impl HttpRangeReader {
    fn open(url: String, headers: Vec<(&'static str, String)>) -> io::Result<Self> {
        let (status, body, total) = range_get(&url, &headers, &format!("bytes=-{TAIL_GUESS}"))?;
        if status != 206 {
            return Err(io::Error::other(
                "the server did not answer a Range request with 206 Partial Content",
            ));
        }
        let len =
            total.ok_or_else(|| io::Error::other("no Content-Range total on a 206 response"))?;
        if len > download::MAX_SIZE {
            return Err(io::Error::other(
                "the pack is bigger than the download limit",
            ));
        }
        let tail_at = len.saturating_sub(body.len() as u64);
        Ok(Self {
            url,
            headers,
            len,
            tail: Arc::from(body),
            tail_at,
            window: None,
            position: 0,
        })
    }
    fn fill_before_tail(&mut self, want: usize) -> io::Result<()> {
        if let Some((start, data)) = &self.window
            && self.position >= *start
            && self.position < start + data.len() as u64
        {
            return Ok(());
        }
        let end = (self.position + (want as u64).max(WINDOW)).min(self.tail_at);
        let range = format!("bytes={}-{}", self.position, end.saturating_sub(1));
        let (status, body, _) = range_get(&self.url, &self.headers, &range)?;
        if status != 206 {
            return Err(io::Error::other(
                "the server did not answer a Range request with 206 Partial Content",
            ));
        }
        self.window = Some((self.position, body));
        Ok(())
    }
}
impl Read for HttpRangeReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.position >= self.len {
            return Ok(0);
        }
        if self.position >= self.tail_at {
            let into = (self.position - self.tail_at) as usize;
            let rest = self.tail.get(into..).unwrap_or(&[]);
            let n = rest.len().min(out.len());
            out[..n].copy_from_slice(&rest[..n]);
            self.position += n as u64;
            return Ok(n);
        }
        self.fill_before_tail(out.len())?;
        let (start, data) = self.window.as_ref().expect("just filled");
        let into = (self.position - start) as usize;
        let rest = data.get(into..).unwrap_or(&[]);
        let n = rest
            .len()
            .min(out.len())
            .min((self.tail_at - self.position) as usize);
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "short range response",
            ));
        }
        out[..n].copy_from_slice(&rest[..n]);
        self.position += n as u64;
        Ok(n)
    }
}
impl Seek for HttpRangeReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let position = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(by) => self.len.checked_add_signed(by),
            SeekFrom::Current(by) => self.position.checked_add_signed(by),
        };
        self.position =
            position.ok_or_else(|| io::Error::other("seek before the archive's start"))?;
        Ok(self.position)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Every entry's identity in the zip at `path`, read locally (no network):
/// what [`try_delta`] compares a later download against.
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

/// After a plain [`download::download_file`] succeeds with no declared
/// hash, index it so the next download of this pack id can diff against it.
fn remember(downloads_dir: &Path, request: &Request, path: &Path) {
    if request.sha1.is_some() {
        return;
    }
    match snapshot(path) {
        Ok(entries) => save_manifest(
            &delta_dir(downloads_dir),
            request.id,
            &Manifest {
                previous_file: path.to_path_buf(),
                entries,
            },
        ),
        Err(e) => log::debug!(
            "Horizon: could not index {} for incremental pack downloads: {e}",
            path.display()
        ),
    }
}

/// Tries to reconstruct the pack from the last one downloaded for this id,
/// fetching only the entries that changed. `None` when there is no earlier
/// download to diff against (the very first time, which downloads in full
/// as usual) or the server gave a hash (vanilla must match it exactly, and a
/// rebuilt pack's "unchanged" entries rarely still have byte-identical
/// headers). Errors are all the ways this can safely give up partway.
fn try_delta(
    downloads_dir: &Path,
    request: &Request,
    headers: &[(&'static str, String)],
) -> Option<Result<PathBuf, io::Error>> {
    if request.sha1.is_some() {
        return None;
    }
    let delta_dir = delta_dir(downloads_dir);
    let manifest = load_manifest(&delta_dir, request.id)?;
    Some(reconstruct(
        downloads_dir,
        &delta_dir,
        request,
        headers,
        &manifest,
    ))
}
fn reconstruct(
    downloads_dir: &Path,
    delta_dir: &Path,
    request: &Request,
    headers: &[(&'static str, String)],
    manifest: &Manifest,
) -> io::Result<PathBuf> {
    let old_file = io::BufReader::new(fs::File::open(&manifest.previous_file)?);
    let mut old = zip::ZipArchive::new(old_file).map_err(io::Error::other)?;
    let known: HashMap<&str, &EntryKey> = manifest
        .entries
        .iter()
        .map(|e| (e.name.as_str(), e))
        .collect();

    let reader = HttpRangeReader::open(request.url.clone(), headers.to_vec())?;
    let mut live = zip::ZipArchive::new(reader).map_err(io::Error::other)?;

    let mut keys = Vec::with_capacity(live.len());
    let (mut total, mut changed) = (0u64, 0u64);
    for i in 0..live.len() {
        let entry = live.by_index_data(i).map_err(io::Error::other)?;
        let key = EntryKey {
            name: entry.name().map_err(io::Error::other)?.into_owned(),
            crc32: entry.crc32(),
            size: entry.size(),
            compressed_size: entry.compressed_size(),
        };
        total += key.compressed_size;
        if known.get(key.name.as_str()) != Some(&&key) {
            changed += key.compressed_size;
        }
        keys.push(key);
    }
    if total > 0 && changed as f64 > total as f64 * WORTHWHILE_CHANGE {
        return Err(io::Error::other(
            "most of the pack changed: a plain download is faster",
        ));
    }

    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    for (i, key) in keys.iter().enumerate() {
        let unchanged = known.get(key.name.as_str()) == Some(&key);
        if unchanged {
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
    let bytes = writer.finish().map_err(io::Error::other)?.into_inner();
    if bytes.len() as u64 > download::MAX_SIZE {
        return Err(io::Error::other(
            "the reconstructed pack is bigger than the download limit",
        ));
    }
    let target = downloads_dir.join(hex(&Sha1::digest(&bytes)));
    fs::create_dir_all(downloads_dir)?;
    fs::write(&target, &bytes)?;
    log::info!(
        "Horizon: pack {:032x} reconstructed, {changed} of {total} compressed bytes fetched",
        request.id
    );
    save_manifest(
        delta_dir,
        request.id,
        &Manifest {
            previous_file: target.clone(),
            entries: keys,
        },
    );
    Ok(target)
}

/// One request: the delta path when it applies and works, else vanilla's own
/// [`download::download_file`] (which still, by itself, reuses an exact
/// hash match with no network at all).
fn download_one(
    downloads_dir: &Path,
    request: &Request,
    headers: &[(&'static str, String)],
    progress: &Sender<download::Progress>,
) -> Result<PathBuf, String> {
    if let Some(result) = try_delta(downloads_dir, request, headers) {
        match result {
            Ok(path) => {
                let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let _ = progress.send(download::Progress::DownloadStart(Some(size)));
                let _ = progress.send(download::Progress::Bytes(size));
                return Ok(path);
            }
            Err(e) => log::debug!(
                "Horizon: incremental download of pack {:032x} skipped: {e}",
                request.id
            ),
        }
    }
    let id = horizon_protocol::crypto::uuid_string(request.id);
    let result = download::download_file(
        &downloads_dir.join(&id),
        request,
        headers,
        download::MAX_SIZE,
        progress,
    );
    if let Ok(path) = &result {
        remember(downloads_dir, request, path);
    }
    result
}

type Job = (
    Vec<Request>,
    Vec<(&'static str, String)>,
    Sender<download::Progress>,
);
fn run(
    dir: &Path,
    requests: &[Request],
    headers: &[(&'static str, String)],
    progress: &Sender<download::Progress>,
) -> Batch {
    let mut batch = Batch::default();
    for request in requests {
        let _ = progress.send(download::Progress::RequestStart);
        let result = download_one(dir, request, headers, progress);
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
/// type: batches download in order, on one thread, trying the incremental
/// path first for each pack with no declared hash.
pub(super) struct DeltaQueue {
    jobs: Sender<Job>,
    done: Receiver<Batch>,
}
impl DeltaQueue {
    pub(super) fn open(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        // `DownloadQueue.MAX_KEPT_PACKS`: kept in step with
        // `server_packs::download`'s own cleanup, since both share this folder.
        download::vacuum(&dir, 20);
        let (jobs, todo) = channel::<Job>();
        let (finished, done) = channel();
        std::thread::Builder::new()
            .name("pack-delta-queue".into())
            .spawn(move || {
                for (requests, headers, progress) in todo {
                    if finished
                        .send(run(&dir, &requests, &headers, &progress))
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
    use std::io::Write;
    use std::net::TcpListener;

    fn scratch() -> PathBuf {
        use std::hash::{BuildHasher, RandomState};
        let n = RandomState::new().hash_one(std::time::Instant::now());
        let dir = std::env::temp_dir().join(format!("horizon-pack-delta-{n:016x}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A one-shot zip, built with the real `zip` crate so its local headers,
    /// central directory and end record are exactly what a real pack's are.
    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for &(name, body) in files {
            w.start_file(name, options).unwrap();
            io::Write::write_all(&mut w, body).unwrap();
        }
        w.finish().unwrap().into_inner()
    }
    fn names_and_bytes(path: &Path) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .map(|i| {
                let mut file = archive.by_index(i).unwrap();
                let name = file.name().unwrap().into_owned();
                let mut body = Vec::new();
                file.read_to_end(&mut body).unwrap();
                (name, body)
            })
            .collect()
    }

    /// Answers Range requests against `body`, one TCP connection each (as
    /// every real request here is), until `max_requests` or 300ms pass with
    /// none pending: the bytes sent across however many actually came, since
    /// how many that is depends on how `zip` happens to batch its reads.
    fn serve_ranges(body: Vec<u8>, max_requests: usize) -> (String, std::thread::JoinHandle<u64>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/pack.zip", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut sent = 0u64;
            let mut served = 0;
            let mut idle_since = std::time::Instant::now();
            while served < max_requests {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        if served > 0 && idle_since.elapsed() > Duration::from_millis(300) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => break,
                };
                stream.set_nonblocking(false).unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                while !request.ends_with(b"\r\n\r\n") {
                    let n = stream.read(&mut buf).unwrap();
                    request.extend_from_slice(&buf[..n]);
                }
                let text = String::from_utf8_lossy(&request);
                let range = text
                    .lines()
                    .find_map(|l| l.strip_prefix("Range: bytes=").map(str::trim))
                    .expect("every request here sets Range");
                // Either a suffix range (`-N`: the last N bytes) or `start-end`.
                let (start, end): (u64, u64) = match range.strip_prefix('-') {
                    Some(suffix) => {
                        let n: u64 = suffix.parse().unwrap();
                        (
                            body.len() as u64 - n.min(body.len() as u64),
                            body.len() as u64 - 1,
                        )
                    }
                    None => {
                        let (s, e) = range.split_once('-').unwrap();
                        let end = if e.is_empty() {
                            body.len() as u64 - 1
                        } else {
                            e.parse().unwrap()
                        };
                        (s.parse().unwrap(), end.min(body.len() as u64 - 1))
                    }
                };
                let slice = &body[start as usize..=end as usize];
                sent += slice.len() as u64;
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len(),
                    slice.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(slice).unwrap();
                served += 1;
                idle_since = std::time::Instant::now();
            }
            sent
        });
        (url, handle)
    }

    /// Content that deflate cannot shrink away, so this entry's size in the
    /// archive stays close to `n` bytes: large enough next to the rest of
    /// the fixture that skipping it is a clearly measurable saving.
    fn filler(n: usize, seed: u8) -> Vec<u8> {
        (0..n)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed) ^ ((i >> 3) as u8))
            .collect()
    }

    #[test]
    fn a_reconstructed_pack_reuses_unchanged_entries_and_drops_removed_ones() {
        let dir = scratch();
        let old_path = dir.join("old.zip");
        let unchanged = filler(4000, 1);
        fs::write(
            &old_path,
            // `a.txt` first and large: its bytes must never cross the wire again.
            zip_of(&[
                ("a.txt", &unchanged),
                ("b.txt", b"will change"),
                ("c.txt", b"will be removed"),
            ]),
        )
        .unwrap();
        let delta = delta_dir(&dir);
        save_manifest(
            &delta,
            7,
            &Manifest {
                previous_file: old_path.clone(),
                entries: snapshot(&old_path).unwrap(),
            },
        );
        let new_body = zip_of(&[
            ("a.txt", &unchanged),
            ("b.txt", b"it changed now"),
            ("d.txt", b"brand new"),
        ]);
        let (url, server) = serve_ranges(new_body.clone(), 10);
        let request = Request {
            id: 7,
            url,
            sha1: None,
        };
        let result = reconstruct(
            &dir,
            &delta,
            &request,
            &[],
            &load_manifest(&delta, 7).unwrap(),
        )
        .unwrap();
        let sent = server.join().unwrap();
        assert!(
            sent < new_body.len() as u64,
            "a.txt's bytes should not have been fetched again: {sent} of {}",
            new_body.len()
        );

        let mut entries = names_and_bytes(&result);
        entries.sort();
        assert_eq!(
            entries,
            [
                ("a.txt".to_owned(), unchanged),
                ("b.txt".to_owned(), b"it changed now".to_vec()),
                ("d.txt".to_owned(), b"brand new".to_vec()),
            ]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_first_download_has_nothing_to_diff_against() {
        let dir = scratch();
        let request = Request {
            id: 1,
            url: "http://127.0.0.1:9/dead.zip".into(),
            sha1: None,
        };
        assert!(try_delta(&dir, &request, &[]).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_declared_hash_never_tries_the_incremental_path() {
        let dir = scratch();
        let old_path = dir.join("old.zip");
        fs::write(&old_path, zip_of(&[("a.txt", b"x")])).unwrap();
        save_manifest(
            &delta_dir(&dir),
            2,
            &Manifest {
                previous_file: old_path,
                entries: snapshot(&dir.join("old.zip")).unwrap(),
            },
        );
        let request = Request {
            id: 2,
            url: "http://127.0.0.1:9/p.zip".into(),
            sha1: Some("0".repeat(40)),
        };
        assert!(try_delta(&dir, &request, &[]).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_vacuumed_cache_file_falls_back_cleanly() {
        let dir = scratch();
        let delta = delta_dir(&dir);
        save_manifest(
            &delta,
            3,
            &Manifest {
                previous_file: dir.join("gone.zip"),
                entries: Vec::new(),
            },
        );
        let request = Request {
            id: 3,
            url: "http://127.0.0.1:9/p.zip".into(),
            sha1: None,
        };
        assert!(
            reconstruct(
                &dir,
                &delta,
                &request,
                &[],
                &load_manifest(&delta, 3).unwrap()
            )
            .is_err()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_server_without_range_support_fails_the_attempt() {
        let dir = scratch();
        let old_path = dir.join("old.zip");
        fs::write(&old_path, zip_of(&[("a.txt", b"x")])).unwrap();
        save_manifest(
            &delta_dir(&dir),
            4,
            &Manifest {
                previous_file: old_path,
                entries: snapshot(&dir.join("old.zip")).unwrap(),
            },
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pack.zip", listener.local_addr().unwrap());
        let body = zip_of(&[("a.txt", b"x")]);
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        });
        let request = Request {
            id: 4,
            url,
            sha1: None,
        };
        let result = reconstruct(
            &dir,
            &delta_dir(&dir),
            &request,
            &[],
            &load_manifest(&delta_dir(&dir), 4).unwrap(),
        );
        assert!(result.is_err());
        server.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_almost_entirely_changed_pack_gives_up_for_a_plain_download() {
        let dir = scratch();
        let old_path = dir.join("old.zip");
        fs::write(
            &old_path,
            zip_of(&[("a.txt", b"old a"), ("b.txt", b"old b")]),
        )
        .unwrap();
        let delta = delta_dir(&dir);
        save_manifest(
            &delta,
            5,
            &Manifest {
                previous_file: old_path,
                entries: snapshot(&dir.join("old.zip")).unwrap(),
            },
        );
        let new_body = zip_of(&[("a.txt", b"new a"), ("b.txt", b"new b")]);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pack.zip", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
            }
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                new_body.len() - 1,
                new_body.len(),
                new_body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&new_body).unwrap();
        });
        let request = Request {
            id: 5,
            url,
            sha1: None,
        };
        let result = reconstruct(
            &dir,
            &delta,
            &request,
            &[],
            &load_manifest(&delta, 5).unwrap(),
        );
        assert!(
            result.is_err(),
            "both entries changed: not worth the per-entry requests"
        );
        server.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
