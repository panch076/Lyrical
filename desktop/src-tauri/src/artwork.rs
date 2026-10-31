//! Resolves album artwork to a data URL the webviews can display directly.
//! Order: the player's own artwork, then the iTunes Search API.

use crate::media::Artwork;
use base64::Engine;
use regex::Regex;
use serde::Deserialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Lyrical/1.0")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .expect("http client")
});

/// Polls after a track change during which artwork is re-read every time.
const FAST_POLLS: u32 = 30;
/// After that, artwork is only re-read once per this many polls.
const SLOW_EVERY: u32 = 10;
/// Polls a new track may go without player artwork before it counts as having none.
const SETTLE_POLLS: u32 = 4;

pub enum Update {
    Keep,
    Resolve(Option<Artwork>),
}

/// Decides when the player's artwork needs resolving. Players publish a new
/// title first and its artwork some time later, so artwork read at the moment
/// of a track change is often missing or still the previous track's.
#[derive(Default)]
pub struct Watcher {
    polls: u32,
    fingerprint: Option<u64>,
    published: bool,
}

impl Watcher {
    pub fn track_changed(&mut self) {
        self.polls = 0;
        self.published = false;
    }

    /// Whether to read the player's artwork on this poll.
    pub fn due(&mut self) -> bool {
        let due = self.polls < FAST_POLLS || self.polls % SLOW_EVERY == 0;
        self.polls = self.polls.wrapping_add(1);
        due
    }

    pub fn update(&mut self, art: Option<Artwork>) -> Update {
        match art {
            Some(art) => {
                let fingerprint = fingerprint(&art);
                if self.published && self.fingerprint == Some(fingerprint) {
                    return Update::Keep;
                }
                self.fingerprint = Some(fingerprint);
                self.published = true;
                Update::Resolve(Some(art))
            }
            // A failed read isn't a change: only a new track that never gets
            // artwork falls back to a lookup.
            None if !self.published && self.polls > SETTLE_POLLS => {
                self.fingerprint = None;
                self.published = true;
                Update::Resolve(None)
            }
            None => Update::Keep,
        }
    }
}

fn fingerprint(art: &Artwork) -> u64 {
    let mut hasher = DefaultHasher::new();
    match art {
        Artwork::Bytes(bytes) => bytes.hash(&mut hasher),
        Artwork::Url(url) => url.hash(&mut hasher),
    }
    hasher.finish()
}

pub async fn resolve(source: Option<Artwork>, title: &str, artist: &str) -> Option<String> {
    let bytes = match source {
        Some(Artwork::Bytes(bytes)) => Some(bytes),
        Some(Artwork::Url(url)) => load(&url).await,
        None => None,
    };
    let bytes = match bytes {
        Some(bytes) => bytes,
        None => {
            let url = itunes_artwork(title, artist).await?;
            download(&url).await?
        }
    };
    Some(data_url(&bytes))
}

async fn load(url: &str) -> Option<Vec<u8>> {
    if url.starts_with("file://") {
        let path = url::Url::parse(url).ok()?.to_file_path().ok()?;
        return std::fs::read(path).ok();
    }
    if url.starts_with("http") {
        if let Some(bytes) = download(&high_resolution(url)).await {
            return Some(bytes);
        }
        return download(url).await;
    }
    None
}

async fn download(url: &str) -> Option<Vec<u8>> {
    let response = CLIENT.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (bytes.len() > 64).then(|| bytes.to_vec())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    artwork_url100: Option<String>,
}

#[derive(Deserialize)]
struct Response {
    results: Vec<Item>,
}

async fn itunes_artwork(title: &str, artist: &str) -> Option<String> {
    let term = format!("{artist} {title}");
    let response: Response = CLIENT
        .get("https://itunes.apple.com/search")
        .query(&[("term", term.as_str()), ("entity", "song"), ("limit", "1")])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let url = response.results.into_iter().find_map(|item| item.artwork_url100)?;
    Some(high_resolution(&url))
}

static APPLE_SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+x\d+(bb|cc|sr)?").unwrap());
static GOOGLE_SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"=w\d+-h\d+").unwrap());

/// Rewrites CDN artwork URLs to a large size (Apple's …/100x100bb.jpg and
/// Google's/YouTube Music's …=w120-h120-l90-rj).
pub fn high_resolution(url: &str) -> String {
    if url.contains("mzstatic.com") {
        APPLE_SIZE.replace_all(url, "1200x1200bb").into_owned()
    } else if url.contains("googleusercontent.com") || url.contains("ggpht.com") {
        GOOGLE_SIZE.replace_all(url, "=w1200-h1200").into_owned()
    } else {
        url.to_string()
    }
}

fn data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        "image/jpeg"
    };
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(value: u8) -> Option<Artwork> {
        Some(Artwork::Bytes(vec![value; 100]))
    }

    /// Runs one poll, returning whether artwork was published and if it was a lookup.
    fn poll(watcher: &mut Watcher, art: Option<Artwork>) -> Option<bool> {
        assert!(watcher.due());
        match watcher.update(art) {
            Update::Keep => None,
            Update::Resolve(art) => Some(art.is_none()),
        }
    }

    #[test]
    fn artwork_arriving_after_the_title_replaces_the_stale_one() {
        let mut watcher = Watcher::default();
        watcher.track_changed();
        assert_eq!(poll(&mut watcher, bytes(1)), Some(false));
        watcher.track_changed();
        // Still the previous track's artwork, published under the new track.
        assert_eq!(poll(&mut watcher, bytes(1)), Some(false));
        assert_eq!(poll(&mut watcher, bytes(1)), None);
        assert_eq!(poll(&mut watcher, bytes(2)), Some(false));
        assert_eq!(poll(&mut watcher, bytes(2)), None);
    }

    #[test]
    fn missing_artwork_waits_before_falling_back_to_a_lookup() {
        let mut watcher = Watcher::default();
        watcher.track_changed();
        for _ in 0..SETTLE_POLLS {
            assert_eq!(poll(&mut watcher, None), None);
        }
        assert_eq!(poll(&mut watcher, None), Some(true));
        assert_eq!(poll(&mut watcher, None), None);
        // The player's own artwork still wins if it turns up later.
        assert_eq!(poll(&mut watcher, bytes(1)), Some(false));
    }

    #[test]
    fn failed_reads_do_not_drop_published_artwork() {
        let mut watcher = Watcher::default();
        watcher.track_changed();
        assert_eq!(poll(&mut watcher, bytes(1)), Some(false));
        for _ in 0..SETTLE_POLLS + 2 {
            assert_eq!(poll(&mut watcher, None), None);
        }
        assert_eq!(poll(&mut watcher, bytes(1)), None);
    }

    #[test]
    fn slows_down_once_a_track_has_settled() {
        let mut watcher = Watcher::default();
        watcher.track_changed();
        let due = (0..FAST_POLLS + SLOW_EVERY).filter(|_| watcher.due()).count() as u32;
        assert_eq!(due, FAST_POLLS + 1);
        watcher.track_changed();
        assert!(watcher.due());
    }
}
