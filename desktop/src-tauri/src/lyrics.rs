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

const API: &str = "https://lrclib.net/api";

/// Seconds to wait before each retry. LRCLIB serves songs it has cached but
/// answers others with "503 ServerOverloaded" when busy, sometimes for well
/// over a minute, so less popular songs need patience.
const RETRY_DELAYS: [u64; 8] = [2, 4, 8, 15, 30, 30, 30, 30];

/// `still_wanted` stops the retries once the track is no longer playing.
pub async fn fetch(track: &Snapshot, still_wanted: impl Fn() -> bool) -> LyricsPayload {
    fetch_from(API, track, still_wanted).await
}

async fn fetch_from(api: &str, track: &Snapshot, still_wanted: impl Fn() -> bool) -> LyricsPayload {
    if track.track_id == DEMO_TRACK_ID {
        return demo(track);
    }
    let key = format!("{}|{}", track.artist, track.title).to_lowercase();
    if let Some(cached) = CACHE.lock().unwrap().get(&key) {
        return LyricsPayload { track_id: track.track_id.clone(), ..cached.clone() };
    }

    let (mut records, mut failed) = lookup(api, track).await;
    for delay in RETRY_DELAYS {
        if !failed || records.iter().any(Record::has_synced) || !still_wanted() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        if !still_wanted() {
            break;
        }
        (records, failed) = lookup(api, track).await;
    }

    let result = choose(&records, track.duration, &track.track_id);
    // A failed lookup may have missed better lyrics, so try again next time.
    if !failed || records.iter().any(Record::has_synced) {
        CACHE.lock().unwrap().insert(key, result.clone());
    }
    result
}

/// Runs each query until one has synced lyrics. The flag is true if any
/// request failed, meaning the records may be incomplete.
async fn lookup(api: &str, track: &Snapshot) -> (Vec<Record>, bool) {
    let mut failed = false;
    let mut records = Vec::new();
    for (title, artist) in queries(track) {
        // LRCLIB accepts an empty album, which is all some players report.
        if let Some(duration) = track.duration {
            match get(api, &title, &artist, &track.album, duration).await {
                Ok(Some(record)) => records.push(record),
                Ok(None) => {}
                Err(_) => failed = true,
            }
        }
        if !records.iter().any(Record::has_synced) {
            match search(api, &title, &artist).await {
                Ok(found) => records.extend(found),
                Err(_) => failed = true,
            }
        }
        if records.iter().any(Record::has_synced) {
            break;
        }
    }
    (records, failed)
}

/// The (title, artist) pairs to try for a track, best first.
fn queries(track: &Snapshot) -> Vec<(String, String)> {
    let mut queries = vec![(track.title.clone(), track.artist.clone())];
    let cleaned = (clean_title(&track.title), primary_artist(&track.artist));
    if !queries.contains(&cleaned) {
        queries.push(cleaned);
    }
    // Browser videos (YouTube in Chrome/Edge) report "Artist - Song" as the
    // title and the channel name as the artist.
    if let Some((artist, title)) = split_video_title(&track.title) {
        if !queries.contains(&(title.clone(), artist.clone())) {
            queries.push((title, artist));
        }
    }
    queries
}

/// None means LRCLIB gave a definite answer that it has nothing. Errors, which
/// include it being overloaded or rate limiting, are worth retrying.
async fn request(url: String, query: &[(&str, &str)]) -> reqwest::Result<Option<reqwest::Response>> {
    let response = CLIENT.get(url).query(query).send().await?;
    let status = response.status();
    if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Ok(None);
    }
    response.error_for_status().map(Some)
}

async fn get(api: &str, title: &str, artist: &str, album: &str, duration: f64) -> reqwest::Result<Option<Record>> {
    let duration = duration.round().to_string();
    let query = [("track_name", title), ("artist_name", artist), ("album_name", album), ("duration", &duration)];
    let Some(response) = request(format!("{api}/get"), &query).await? else { return Ok(None) };
    Ok(response.json().await.ok())
}

async fn search(api: &str, title: &str, artist: &str) -> reqwest::Result<Vec<Record>> {
    let query = [("track_name", title), ("artist_name", artist)];
    let Some(response) = request(format!("{api}/search"), &query).await? else { return Ok(Vec::new()) };
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

/// "Artist - Song (Official Video)" → ("Artist", "Song").
pub fn split_video_title(title: &str) -> Option<(String, String)> {
    let (artist, song) = title.split_once(" - ").or_else(|| title.split_once(" – "))?;
    let (artist, song) = (primary_artist(artist), clean_title(song));
    (!artist.is_empty() && !song.is_empty()).then_some((artist, song))
}

pub fn primary_artist(artist: &str) -> String {
    let artist = artist.trim();
    let artist = artist.strip_suffix(" - Topic").unwrap_or(artist);
    let artist = artist.strip_suffix("VEVO").unwrap_or(artist);
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
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use tauri::async_runtime::block_on;

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
        assert_eq!(primary_artist("Adele - Topic"), "Adele");
        assert_eq!(primary_artist("AdeleVEVO"), "Adele");
    }

    #[test]
    fn splits_video_titles() {
        assert_eq!(
            split_video_title("Adele - Hello (Official Music Video)"),
            Some(("Adele".into(), "Hello".into()))
        );
        assert_eq!(split_video_title("Hello"), None);
    }

    const SYNCED: &str = r#"{"duration":100,"syncedLyrics":"[00:01.00]hello","plainLyrics":"hello"}"#;

    fn track(title: &str, artist: &str, duration: Option<f64>) -> Snapshot {
        Snapshot {
            track_id: format!("test|{title}|{artist}"),
            title: title.into(),
            artist: artist.into(),
            duration,
            ..Default::default()
        }
    }

    /// Serves canned responses, one per request, then repeats the last. Returns
    /// the base URL and a log of the request lines it received.
    fn mock_server(responses: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let requests = log.clone();
        std::thread::spawn(move || {
            for (index, stream) in listener.incoming().enumerate() {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                requests.lock().unwrap().push(line);
                let (status, body) = &responses[index.min(responses.len() - 1)];
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        (base, log)
    }

    fn busy() -> (u16, String) {
        (503, r#"{"name":"ServerOverloaded","statusCode":503}"#.into())
    }

    #[test]
    fn retries_until_an_overloaded_server_answers() {
        // Both requests of the first attempt fail; the retry's first succeeds.
        let (base, log) = mock_server(vec![busy(), busy(), (200, SYNCED.into())]);
        let song = track("Retry Song", "Artist", Some(100.0));
        let payload = block_on(fetch_from(&base, &song, || true));
        assert_eq!(payload.state, "synced");
        assert_eq!(log.lock().unwrap().len(), 3);

        // The result is cached now, so this makes no requests.
        let again = block_on(fetch_from(&base, &song, || true));
        assert_eq!(again.state, "synced");
        assert_eq!(log.lock().unwrap().len(), 3);
    }

    #[test]
    fn stops_retrying_once_the_track_is_gone_and_caches_nothing() {
        let (base, log) = mock_server(vec![busy()]);
        let song = track("Skipped Song", "Artist", Some(100.0));
        let payload = block_on(fetch_from(&base, &song, || false));
        assert_eq!(payload.state, "unavailable");
        assert_eq!(log.lock().unwrap().len(), 2);
        assert!(!CACHE.lock().unwrap().contains_key("artist|skipped song"));
    }

    #[test]
    fn a_definite_not_found_is_not_retried() {
        let (base, log) = mock_server(vec![(404, "{}".into()), (200, "[]".into())]);
        let song = track("Missing Song", "Artist", Some(100.0));
        let payload = block_on(fetch_from(&base, &song, || true));
        assert_eq!(payload.state, "unavailable");
        assert_eq!(log.lock().unwrap().len(), 2);
        assert!(CACHE.lock().unwrap().contains_key("artist|missing song"));
    }

    #[test]
    fn exact_lookup_runs_without_an_album() {
        let (base, log) = mock_server(vec![(200, SYNCED.into())]);
        let song = track("No Album Song", "Artist", Some(100.0));
        assert_eq!(block_on(fetch_from(&base, &song, || true)).state, "synced");
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert!(log[0].starts_with("GET /get?"), "{}", log[0]);
        assert!(log[0].contains("album_name=&"), "{}", log[0]);
    }

    #[test]
    fn queries_skip_duplicates() {
        assert_eq!(queries(&track("Song", "Artist", None)).len(), 1);
        assert_eq!(
            queries(&track("Song (Live)", "A & B", None)),
            [("Song (Live)".to_string(), "A & B".to_string()), ("Song".to_string(), "A".to_string())]
        );
    }

    /// Hits the real service: `cargo test --lib live_ -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_lesser_known_songs_resolve() {
        let songs = [
            ("Figaro", "Madvillain", 145.0),
            ("Rhymes Like Dimes", "MF DOOM", 259.0),
            ("Curls", "MF DOOM", 95.0),
            ("Hoe Cakes", "MF DOOM", 174.0),
            ("Whoa", "Earl Sweatshirt", 157.0),
            ("Burgundy", "Earl Sweatshirt", 133.0),
            ("Fire Escape", "Saba", 215.0),
            ("Rainbow Six", "JPEGMAFIA", 121.0),
            ("Thug Tears", "JPEGMAFIA", 214.0),
            ("Rock and Roll Night Club", "Mac DeMarco", 187.0),
        ];
        for (title, artist, duration) in songs {
            let start = std::time::Instant::now();
            let payload = block_on(fetch(&track(title, artist, Some(duration)), || true));
            println!("{title:28} {artist:18} {:12} {} lines {:>5.1}s", payload.state, payload.lines.len(), start.elapsed().as_secs_f64());
        }
    }
}
