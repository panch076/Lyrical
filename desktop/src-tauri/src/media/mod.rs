//! System-wide "now playing" detection.
//!
//! - Windows: Global System Media Transport Controls (the media flyout API)
//! - Linux: MPRIS over D-Bus
//! - Demo: a simulated track, used on macOS and when `LYRICAL_DEMO=1`

use serde::Serialize;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;
mod demo;

pub use demo::{DEMO_LYRICS, DEMO_TRACK_ID};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Human-readable name of the playing app, e.g. "Spotify".
    pub source: String,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Seconds.
    pub duration: Option<f64>,
    /// Playback position in seconds, valid at `timestamp_ms`.
    pub position: f64,
    /// Unix time in milliseconds at which `position` was sampled.
    pub timestamp_ms: f64,
    pub is_playing: bool,
    pub can_control: bool,
    pub can_seek: bool,
}

// Only the Windows/Linux backends produce artwork or need friendly names.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
pub enum Artwork {
    Bytes(Vec<u8>),
    Url(String),
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    PlayPause,
    Next,
    Previous,
    Seek(f64),
}

pub trait MediaBackend: Send {
    fn snapshot(&mut self) -> Option<Snapshot>;
    /// Artwork for the track from the most recent snapshot.
    fn artwork(&mut self) -> Option<Artwork>;
    fn send(&mut self, command: Command);
}

pub fn backend() -> Box<dyn MediaBackend> {
    if std::env::var("LYRICAL_DEMO").is_ok_and(|v| v == "1") {
        return Box::new(demo::DemoBackend::new());
    }
    #[cfg(windows)]
    return Box::new(windows::WindowsBackend::new());
    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxBackend::new());
    #[allow(unreachable_code)]
    Box::new(demo::DemoBackend::new())
}

/// Call once on the polling thread before using the backend.
pub fn init_thread() {
    #[cfg(windows)]
    windows::init_thread();
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
/// Turns raw app identifiers ("Spotify.exe", "chromium.instance1234") into names.
pub fn friendly_source(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let known = [
        ("spotify", "Spotify"),
        ("applemusic", "Apple Music"),
        ("chrome", "Chrome"),
        ("chromium", "Chromium"),
        ("msedge", "Edge"),
        ("firefox", "Firefox"),
        ("brave", "Brave"),
        ("opera", "Opera"),
        ("vivaldi", "Vivaldi"),
        ("zunemusic", "Media Player"),
        ("vlc", "VLC"),
        ("tidal", "TIDAL"),
        ("deezer", "Deezer"),
        ("youtube", "YouTube Music"),
        ("rhythmbox", "Rhythmbox"),
        ("elisa", "Elisa"),
        ("strawberry", "Strawberry"),
        ("amberol", "Amberol"),
    ];
    for (needle, name) in known {
        if lower.contains(needle) {
            return name.to_string();
        }
    }
    let base = raw.rsplit(['\\', '/']).next().unwrap_or(raw);
    let base = base.trim_end_matches(".exe").trim_end_matches(".EXE");
    let base = base.split('!').next().unwrap_or(base);
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Media".to_string(),
    }
}
