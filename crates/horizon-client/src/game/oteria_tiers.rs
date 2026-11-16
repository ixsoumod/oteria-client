//! Oteria's Tier Tagger (as the Fabric mod `tiertagger`): each player's PvP tier from MCTiers
//! (`horizon_addresses::MCTIERS_API`), before their name over their head and in the player list,
//! in the game mode the player chose on the Tier Tagger tile, or their best one. Off by default.
//!
//! A player's tiers are asked once, on a thread of their own, and kept for half an hour: by
//! UUID for a premium account (version 4), by name otherwise (an offline-mode server's UUIDs
//! are made from the name, which MCTiers doesn't know). Only the UUID or the name goes out.

use super::Game;
use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The tag shown before a name: the tier (`HT3`), its colour (`0xRRGGBB`), and the icon of the
/// game mode it is in (an atlas sprite, `oteria_icons::tier_mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TierTag {
    pub label: &'static str,
    pub colour: u32,
    pub icon: Option<&'static str>,
}

/// How long tiers are kept before being asked again.
const FRESH: Duration = Duration::from_secs(30 * 60);
/// How long a question that went unanswered (no network) waits before it is asked again.
const RETRY: Duration = Duration::from_secs(2 * 60);
/// The most players waiting to be asked: more are asked once others are done.
const QUEUE: usize = 256;

/// A player's tier in one game mode: tier 1 (best) to 5, high or low, retired.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Ranking {
    pub mode: String,
    pub tier: u8,
    pub high: bool,
    pub retired: bool,
}

enum Entry {
    /// Asked at this instant, no answer yet.
    Asked(Instant),
    /// The answer (empty: not on the list), and when it came.
    Got(Instant, Vec<Ranking>),
}

#[derive(Default)]
struct Shared {
    cache: HashMap<u128, Entry>,
    queue: VecDeque<(u128, String)>,
}

fn shared() -> &'static (Mutex<Shared>, Condvar) {
    static SHARED: OnceLock<(Mutex<Shared>, Condvar)> = OnceLock::new();
    SHARED.get_or_init(|| {
        std::thread::Builder::new()
            .name("oteria tiers".into())
            .spawn(worker)
            .ok();
        (Mutex::new(Shared::default()), Condvar::new())
    })
}

/// Asks MCTiers for the players queued, one after another.
fn worker() {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .user_agent("Oteria Client")
        .build();
    loop {
        let (lock, wake) = shared();
        let (uuid, name) = {
            let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if let Some(next) = s.queue.pop_front() {
                    break next;
                }
                s = wake.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        };
        let got = fetch(&agent, uuid, &name);
        let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
        match got {
            // Not answered: left as asked, so it is asked again after `RETRY`.
            None => {}
            Some(rankings) => {
                s.cache.insert(uuid, Entry::Got(Instant::now(), rankings));
            }
        }
        drop(s);
        // Gently: a few questions a second at most.
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// A premium account's UUID (version 4); an offline one is made from the name (version 3).
fn premium(uuid: u128) -> bool {
    (uuid >> 76) & 0xF == 4
}

/// One player's tiers; `Some(empty)` when MCTiers doesn't know them, `None` when it didn't answer.
fn fetch(agent: &ureq::Agent, uuid: u128, name: &str) -> Option<Vec<Ranking>> {
    let base = horizon_addresses::MCTIERS_API;
    let by_uuid = premium(uuid);
    let url = if by_uuid {
        format!("{base}/profile/{uuid:032x}/rankings")
    } else {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Some(Vec::new());
        }
        format!("{base}/profile/by-name/{name}")
    };
    let body = match agent.get(&url).call() {
        Ok(r) => r.into_string().ok()?,
        Err(ureq::Error::Status(404, _)) => return Some(Vec::new()),
        Err(e) => {
            log::debug!("Oteria tiers: {name} not answered: {e}");
            return None;
        }
    };
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    let rankings = if by_uuid {
        &value
    } else {
        value.get("rankings")?
    };
    Some(parse_rankings(rankings))
}

/// MCTiers' rankings object: `{"sword": {"tier": 3, "pos": 0, "retired": false, ...}, ...}`.
pub(super) fn parse_rankings(v: &serde_json::Value) -> Vec<Ranking> {
    let Some(map) = v.as_object() else {
        return Vec::new();
    };
    let mut out: Vec<Ranking> = map
        .iter()
        .filter_map(|(mode, r)| {
            let tier = r.get("tier")?.as_u64()?;
            (1..=5).contains(&tier).then(|| Ranking {
                mode: mode.clone(),
                tier: tier as u8,
                high: r.get("pos").and_then(|p| p.as_u64()) == Some(0),
                retired: r.get("retired").and_then(|p| p.as_bool()).unwrap_or(false),
            })
        })
        .collect();
    out.sort_by(|a, b| a.mode.cmp(&b.mode));
    out
}

/// The tier to show: `mode`'s, or (empty mode) the best one held now, else the best retired.
pub(super) fn pick<'a>(rankings: &'a [Ranking], mode: &str) -> Option<&'a Ranking> {
    if !mode.is_empty() {
        return rankings.iter().find(|r| r.mode == mode);
    }
    let rank = |r: &&Ranking| (r.retired, r.tier, !r.high);
    rankings.iter().min_by_key(rank)
}

/// The tag's text: `HT3`, `LT1`, `RHT2` (retired).
pub(super) fn label(r: &Ranking) -> &'static str {
    const LABELS: [[[&str; 5]; 2]; 2] = [
        [
            ["HT1", "HT2", "HT3", "HT4", "HT5"],
            ["LT1", "LT2", "LT3", "LT4", "LT5"],
        ],
        [
            ["RHT1", "RHT2", "RHT3", "RHT4", "RHT5"],
            ["RLT1", "RLT2", "RLT3", "RLT4", "RLT5"],
        ],
    ];
    LABELS[usize::from(r.retired)][usize::from(!r.high)][usize::from(r.tier.clamp(1, 5) - 1)]
}

/// The tag's colour (`0xRRGGBB`), as TierTagger paints them: gold, silver, bronze, then purples;
/// the low tier a shade darker; retired, a pale blue.
pub(super) fn colour(r: &Ranking) -> u32 {
    if r.retired {
        return 0xA2_D6FF;
    }
    match (r.tier, r.high) {
        (1, true) => 0xFF_CF4A,
        (1, false) => 0xD5_B355,
        (2, true) => 0xC4_D3E7,
        (2, false) => 0xA0_A7B2,
        (3, true) => 0xF8_9F5A,
        (3, false) => 0xC6_7B42,
        (4, true) => 0x81_749A,
        (4, false) => 0x65_5B79,
        (_, true) => 0x8F_82A8,
        (_, false) => 0x65_5B79,
    }
}

impl Game<'_> {
    /// The tier tag before a player's name (its mode's icon, the tier, its colour), when Tier
    /// Tagger is on and the player has one; asks MCTiers the first time a player is seen.
    pub(super) fn tier_tag(&self, uuid: u128, name: &str) -> Option<TierTag> {
        let t = &self.settings.extras.pvp.tiers;
        if !t.enabled || uuid == 0 {
            return None;
        }
        let (lock, wake) = shared();
        let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let (ask, shown) = match s.cache.get(&uuid) {
            None => (true, None),
            Some(Entry::Asked(at)) => (now.duration_since(*at) > RETRY, None),
            Some(Entry::Got(at, rankings)) => (
                now.duration_since(*at) > FRESH,
                pick(rankings, &t.mode).map(|r| TierTag {
                    label: label(r),
                    colour: colour(r),
                    icon: horizon_client::oteria_icons::tier_mode(&r.mode),
                }),
            ),
        };
        if ask && s.queue.len() < QUEUE && !s.queue.iter().any(|(u, _)| *u == uuid) {
            // A stale answer stays shown until the new one comes.
            if !matches!(s.cache.get(&uuid), Some(Entry::Got(..))) {
                s.cache.insert(uuid, Entry::Asked(now));
            } else if let Some(Entry::Got(at, _)) = s.cache.get_mut(&uuid) {
                // Not asked again before this answer is in.
                *at = now;
            }
            s.queue.push_back((uuid, name.to_owned()));
            wake.notify_one();
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rankings() -> Vec<Ranking> {
        parse_rankings(&serde_json::json!({
            "sword": {"tier": 3, "pos": 0, "retired": false},
            "pot": {"tier": 1, "pos": 0, "retired": true},
            "axe": {"tier": 2, "pos": 1, "retired": false},
            "uhc": {"tier": 9, "pos": 0},
        }))
    }

    #[test]
    fn the_best_tier_held_wins_over_a_retired_one() {
        let r = rankings();
        assert_eq!(r.len(), 3, "a tier out of 1..=5 is left out");
        let best = pick(&r, "").unwrap();
        assert_eq!((best.mode.as_str(), label(best)), ("axe", "LT2"));
        assert_eq!(label(pick(&r, "pot").unwrap()), "RHT1");
        assert_eq!(label(pick(&r, "sword").unwrap()), "HT3");
        assert!(pick(&r, "mace").is_none());
        // Only retired ones: the best of them.
        let retired: Vec<Ranking> = r.into_iter().filter(|r| r.retired).collect();
        assert_eq!(label(pick(&retired, "").unwrap()), "RHT1");
    }

    /// Asks the real MCTiers (network): `cargo test -p horizon-client --lib asks_mctiers -- --ignored`.
    #[test]
    #[ignore]
    fn asks_mctiers() {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(20))
            .build();
        let by_uuid = fetch(&agent, 0x06ec3577_3299_45fa_bbdf_613b1f86c8ab, "ItzRealMe").unwrap();
        let by_name = fetch(&agent, 0x06ec3577_3299_35fa_bbdf_613b1f86c8ab, "ItzRealMe").unwrap();
        assert!(!by_uuid.is_empty());
        assert_eq!(by_uuid, by_name);
        println!("best: {}", label(pick(&by_uuid, "").unwrap()));
        let unknown = fetch(&agent, 0x00000000_0000_4000_8000_000000000001, "x").unwrap();
        assert!(unknown.is_empty());
    }

    #[test]
    fn the_modes_icon_goes_before_the_tier_and_survives_the_player_list() {
        let tag = TierTag {
            label: "HT3",
            colour: 0xF8_9F5A,
            icon: horizon_client::oteria_icons::tier_mode("sword"),
        };
        let text = super::super::names::tier_prefix(tag);
        let json = text.component().to_string();
        assert!(json.contains("gui/sprites/oteria_tiers/sword"), "{json}");
        // The player list turns it into a component and back.
        let back = horizon_client::scoreboard::comp_text(&text.component());
        assert!(back.component().to_string().contains("oteria_tiers/sword"));
        assert!(back.plain().contains("HT3"));
        for mode in [
            "sword", "vanilla", "uhc", "pot", "nethop", "smp", "axe", "mace",
        ] {
            assert!(
                horizon_client::oteria_icons::tier_mode(mode).is_some(),
                "{mode}"
            );
        }
    }

    #[test]
    fn premium_uuids_are_version_four() {
        assert!(premium(0x06ec3577_3299_45fa_bbdf_613b1f86c8ab));
        assert!(!premium(0x06ec3577_3299_35fa_bbdf_613b1f86c8ab));
    }
}
