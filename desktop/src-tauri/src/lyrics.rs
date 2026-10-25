//! Time-synced lyrics from LRCLIB (https://lrclib.net), fetched at runtime.

use crate::media::{Snapshot, DEMO_LYRICS, DEMO_TRACK_ID};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Debug, Serialize)]
pub struct Line {
    pub time: f64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsPayload {
    pub track_id: String,
    /// "loading" | "synced" | "plain" | "instrumental" | "unavailable"
    pub state: &'static str,
    pub lines: Vec<Line>,
    pub plain: Vec<String>,
}

impl LyricsPayload {
    pub fn loading(track_id: &str) -> Self {
        Self { track_id: track_id.into(), state: "loading", lines: Vec::new(), plain: Vec::new() }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    duration: Option<f64>,
    instrumental: Option<bool>,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl Record {
    fn has_synced(&self) -> bool {
        self.synced_lyrics.as_deref().is_some_and(|s| !s.is_empty())
    }
    fn has_plain(&self) -> bool {
        self.plain_lyrics.as_deref().is_some_and(|s| !s.is_empty())
    }
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Lyrical/1.0 (desktop lyrics overlay)")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .expect("http client")
});

static CACHE: LazyLock<Mutex<HashMap<String, LyricsPayload>>> = LazyLock::new(Default::default);

pub async fn fetch(track: &Snapshot) -> LyricsPayload {
    if track.track_id == DEMO_TRACK_ID {
        return demo(track);
    }
    let key = format!("{}|{}", track.artist, track.title).to_lowercase();
    if let Some(cached) = CACHE.lock().unwrap().get(&key) {
        return LyricsPayload { track_id: track.track_id.clone(), ..cached.clone() };
    }

    let mut network_failed = false;
    let mut records = Vec::new();

    if let (Some(duration), false) = (track.duration, track.album.is_empty()) {
        match get(&track.title, &track.artist, &track.album, duration).await {
            Ok(Some(record)) => records.push(record),
            Ok(None) => {}
            Err(_) => network_failed = true,
        }
    }
    if !records.iter().any(Record::has_synced) {
        match search(&track.title, &track.artist).await {
            Ok(found) => records.extend(found),
            Err(_) => network_failed = true,
        }
    }
    if !records.iter().any(Record::has_synced) {
        let title = clean_title(&track.title);
        let artist = primary_artist(&track.artist);
        if title != track.title || artist != track.artist {
            records.extend(search(&title, &artist).await.unwrap_or_default());
        }
    }

    let result = choose(&records, track.duration, &track.track_id);
    if !(network_failed && result.state == "unavailable") {
        CACHE.lock().unwrap().insert(key, result.clone());
    }
    result
}

async fn get(title: &str, artist: &str, album: &str, duration: f64) -> reqwest::Result<Option<Record>> {
    let response = CLIENT
        .get("https://lrclib.net/api/get")
        .query(&[
            ("track_name", title),
            ("artist_name", artist),
            ("album_name", album),
            ("duration", &duration.round().to_string()),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        return Ok(None);
    }
    Ok(response.json().await.ok())
}

async fn search(title: &str, artist: &str) -> reqwest::Result<Vec<Record>> {
    let response = CLIENT
        .get("https://lrclib.net/api/search")
        .query(&[("track_name", title), ("artist_name", artist)])
        .send()
        .await?;
    if !response.status().is_success() {
        return Ok(Vec::new());
    }
    Ok(response.json().await.unwrap_or_default())
}

fn choose(records: &[Record], duration: Option<f64>, track_id: &str) -> LyricsPayload {
    let distance = |r: &Record| match (duration, r.duration) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => 0.0,
    };
    let mut synced: Vec<&Record> = records
        .iter()
        .filter(|r| r.has_synced() && distance(r) < 15.0)
        .collect();
    synced.sort_by(|a, b| distance(a).total_cmp(&distance(b)));

    let mut payload = LyricsPayload { track_id: track_id.into(), state: "unavailable", lines: Vec::new(), plain: Vec::new() };
    if let Some(lines) = synced.first().and_then(|r| parse_lrc(r.synced_lyrics.as_deref().unwrap_or(""))) {
        payload.state = "synced";
        payload.lines = lines;
    } else if records.first().is_some_and(|r| r.instrumental == Some(true)) {
        payload.state = "instrumental";
    } else if let Some(plain) = records.iter().find(|r| r.has_plain()) {
        payload.state = "plain";
        payload.plain = plain.plain_lyrics.as_deref().unwrap_or("").lines().map(str::to_string).collect();
    }
    payload
}

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(\d+):(\d+(?:[.:]\d+)?)\]").unwrap());
static WORD_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<\d+:\d+(?:\.\d+)?>").unwrap());

pub fn parse_lrc(lrc: &str) -> Option<Vec<Line>> {
    let mut entries: Vec<(f64, String)> = Vec::new();
    for raw in lrc.lines() {
        let tags: Vec<_> = TAG.captures_iter(raw).collect();
        let Some(last) = tags.last() else { continue };
        let text_start = last.get(0).unwrap().end();
        let text = WORD_TAG.replace_all(&raw[text_start..], "").trim().to_string();
        for tag in &tags {
            let minutes: f64 = tag[1].parse().unwrap_or(0.0);
            let seconds: f64 = tag[2].replace(':', ".").parse().unwrap_or(0.0);
            entries.push((minutes * 60.0 + seconds, text.clone()));
        }
    }
    if entries.is_empty() {
        return None;
    }
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Collapse consecutive gaps so instrumental breaks render once.
    let mut lines: Vec<Line> = Vec::new();
    for (time, text) in entries {
        if text.is_empty() && lines.last().is_some_and(|l| l.text.is_empty()) {
            continue;
        }
        lines.push(Line { time, text });
    }
    while lines.last().is_some_and(|l| l.text.is_empty()) {
        lines.pop();
    }
    Some(lines)
}

static CLEAN_PARENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[\(\[][^\)\]]*(feat|ft\.|with|remaster|version|edit|mix|live|mono|stereo|deluxe|bonus|explicit|official|video|audio|lyrics|visualizer)[^\)\]]*[\)\]]").unwrap()
});
static CLEAN_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s+-\s+.*(remaster|version|edit|mix|live|mono|stereo|official|video|audio).*$").unwrap()
});

pub fn clean_title(title: &str) -> String {
    let result = CLEAN_PARENS.replace_all(title, "");
    CLEAN_SUFFIX.replace_all(&result, "").trim().to_string()
}

pub fn primary_artist(artist: &str) -> String {
    let lower = artist.to_lowercase();
    let cut = [", ", " & ", " feat. ", " feat ", " ft. ", " x ", " and "]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .min()
        .unwrap_or(artist.len());
    artist[..cut].trim().to_string()
}

fn demo(track: &Snapshot) -> LyricsPayload {
    LyricsPayload {
        track_id: track.track_id.clone(),
        state: "synced",
        lines: DEMO_LYRICS.iter().map(|(time, text)| Line { time: *time, text: (*text).to_string() }).collect(),
        plain: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_and_collapses_gaps() {
        let lines = parse_lrc("[00:01.00]first\n[00:02.50]\n[00:03.00]\n[00:04.25]second\n[00:09.00]").unwrap();
        let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["first", "", "second"]);
        assert!((lines[2].time - 4.25).abs() < 1e-9);
    }

    #[test]
    fn handles_multiple_tags_per_line() {
        let lines = parse_lrc("[00:10.00][00:01.00]repeat").unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].time < lines[1].time);
    }

    #[test]
    fn cleans_titles_and_artists() {
        assert_eq!(clean_title("Song (Official Video)"), "Song");
        assert_eq!(clean_title("Song - Remastered 2011"), "Song");
        assert_eq!(clean_title("Song (feat. Someone)"), "Song");
        assert_eq!(primary_artist("A & B"), "A");
        assert_eq!(primary_artist("A, B feat. C"), "A");
    }
}
