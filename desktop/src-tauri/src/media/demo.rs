//! A simulated track for development on machines without a supported media
//! API (macOS) and for trying the UI without music playing. The lyrics are
//! original placeholder text written for this demo.

use super::{now_ms, Artwork, Command, MediaBackend, Snapshot};
use std::time::Instant;

pub const DEMO_TRACK_ID: &str = "demo|Glass Horizon|The Placeholders";
const DURATION: f64 = 96.0;

/// (seconds, text). An empty line marks an instrumental break.
pub const DEMO_LYRICS: &[(f64, &str)] = &[
    (4.0, "This is a demo track for Lyrical"),
    (8.0, "Every line here is placeholder text"),
    (12.0, "Watch the highlight sweep across each word as it plays"),
    (17.0, "Longer lines wrap onto a second row in full screen and keep filling in order"),
    (23.0, "Short line"),
    (26.0, ""),
    (34.0, "That pause was an instrumental break"),
    (38.0, "Click any line in full screen to jump to it"),
    (43.0, "Press space to pause, escape to leave"),
    (48.0, "The floating box keeps three lines in view"),
    (53.0, "With the current one centred in the middle"),
    (58.0, "And rolls to the next as the song moves on"),
    (63.0, "Play real music to see real lyrics"),
    (68.0, ""),
    (76.0, "Back for one last chorus of nothing in particular"),
    (82.0, "Thanks for testing Lyrical"),
];

pub struct DemoBackend {
    anchor: Instant,
    offset: f64,
    playing: bool,
}

impl DemoBackend {
    pub fn new() -> Self {
        Self { anchor: Instant::now(), offset: 0.0, playing: true }
    }

    fn position(&self) -> f64 {
        let elapsed = if self.playing { self.anchor.elapsed().as_secs_f64() } else { 0.0 };
        (self.offset + elapsed) % DURATION
    }
}

impl MediaBackend for DemoBackend {
    fn snapshot(&mut self) -> Option<Snapshot> {
        Some(Snapshot {
            source: "Demo".into(),
            track_id: DEMO_TRACK_ID.into(),
            title: "Glass Horizon".into(),
            artist: "The Placeholders".into(),
            album: "Lyrical Demo".into(),
            duration: Some(DURATION),
            position: self.position(),
            timestamp_ms: now_ms(),
            is_playing: self.playing,
            can_control: true,
            can_seek: true,
        })
    }

    fn artwork(&mut self) -> Option<Artwork> {
        None
    }

    fn send(&mut self, command: Command) {
        let position = self.position();
        match command {
            Command::PlayPause => {
                self.offset = position;
                self.playing = !self.playing;
            }
            Command::Next | Command::Previous => self.offset = 0.0,
            Command::Seek(seconds) => self.offset = seconds.clamp(0.0, DURATION),
        }
        self.anchor = Instant::now();
    }
}
