//! Windows backend: Global System Media Transport Controls (GSMTC).
//! Every app that shows up in the Windows media flyout — Spotify, Chrome/Edge
//! (including YouTube Music), Apple Music, Media Player… — is visible here.

use super::{friendly_source, now_ms, Artwork, Command, MediaBackend, Snapshot};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

/// 100ns ticks between 1601-01-01 (Windows epoch) and 1970-01-01.
const EPOCH_DIFFERENCE_TICKS: i64 = 116_444_736_000_000_000;

pub fn init_thread() {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}

pub struct WindowsBackend {
    manager: Option<Manager>,
    session: Option<Session>,
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self { manager: None, session: None }
    }

    fn manager(&mut self) -> Option<&Manager> {
        if self.manager.is_none() {
            self.manager = Manager::RequestAsync().ok()?.get().ok();
        }
        self.manager.as_ref()
    }

    /// Prefers a session that is actively playing; otherwise the one Windows
    /// considers current.
    fn pick_session(&mut self) -> Option<Session> {
        let manager = self.manager()?.clone();
        if let Ok(sessions) = manager.GetSessions() {
            for session in sessions {
                let playing = session
                    .GetPlaybackInfo()
                    .and_then(|info| info.PlaybackStatus())
                    .is_ok_and(|status| status == Status::Playing);
                if playing {
                    return Some(session);
                }
            }
        }
        manager.GetCurrentSession().ok()
    }

    fn read(&mut self, session: &Session) -> windows::core::Result<Option<Snapshot>> {
        let props = session.TryGetMediaPropertiesAsync()?.get()?;
        let title = props.Title()?.to_string();
        if title.trim().is_empty() {
            return Ok(None);
        }
        let artist = props.Artist()?.to_string();
        let album = props.AlbumTitle()?.to_string();
        let source = friendly_source(&session.SourceAppUserModelId()?.to_string());

        let info = session.GetPlaybackInfo()?;
        let is_playing = info.PlaybackStatus()? == Status::Playing;
        let controls = info.Controls()?;
        let can_control = controls.IsPlayPauseToggleEnabled()? || controls.IsPlayEnabled()? || controls.IsPauseEnabled()?;
        let can_seek = controls.IsPlaybackPositionEnabled()?;

        let timeline = session.GetTimelineProperties()?;
        let start = timeline.StartTime()?.Duration as f64 / 1e7;
        let end = timeline.EndTime()?.Duration as f64 / 1e7;
        let position = (timeline.Position()?.Duration as f64 / 1e7 - start).max(0.0);
        let updated = timeline.LastUpdatedTime()?.UniversalTime;
        let timestamp_ms = if updated > EPOCH_DIFFERENCE_TICKS {
            (updated - EPOCH_DIFFERENCE_TICKS) as f64 / 10_000.0
        } else {
            now_ms()
        };
        let duration = (end - start > 1.0).then_some(end - start);

        Ok(Some(Snapshot {
            track_id: format!("{source}|{title}|{artist}"),
            source,
            title,
            artist,
            album,
            duration,
            position,
            timestamp_ms,
            is_playing,
            can_control,
            can_seek: can_seek && duration.is_some(),
        }))
    }

    fn thumbnail(session: &Session) -> windows::core::Result<Vec<u8>> {
        let props = session.TryGetMediaPropertiesAsync()?.get()?;
        let stream = props.Thumbnail()?.OpenReadAsync()?.get()?;
        let size = stream.Size()? as u32;
        let reader = DataReader::CreateDataReader(&stream)?;
        reader.LoadAsync(size)?.get()?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes)?;
        Ok(bytes)
    }
}

impl MediaBackend for WindowsBackend {
    fn snapshot(&mut self) -> Option<Snapshot> {
        let session = self.pick_session()?;
        let snapshot = self.read(&session).ok().flatten();
        self.session = Some(session);
        snapshot
    }

    fn artwork(&mut self) -> Option<Artwork> {
        let session = self.session.as_ref()?;
        Self::thumbnail(session).ok().filter(|b| b.len() > 64).map(Artwork::Bytes)
    }

    fn send(&mut self, command: Command) {
        let Some(session) = self.session.as_ref() else { return };
        let result = match command {
            Command::PlayPause => session.TryTogglePlayPauseAsync().and_then(|op| op.get()),
            Command::Next => session.TrySkipNextAsync().and_then(|op| op.get()),
            Command::Previous => session.TrySkipPreviousAsync().and_then(|op| op.get()),
            Command::Seek(seconds) => {
                let start = session
                    .GetTimelineProperties()
                    .and_then(|t| t.StartTime())
                    .map(|t| t.Duration)
                    .unwrap_or(0);
                session
                    .TryChangePlaybackPositionAsync(start + (seconds.max(0.0) * 1e7) as i64)
                    .and_then(|op| op.get())
            }
        };
        let _ = result;
    }
}
