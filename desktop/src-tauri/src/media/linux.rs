//! Linux backend: MPRIS over the D-Bus session bus. Spotify, Chrome/Chromium,
//! Firefox and nearly every Linux music player implement it.

use super::{friendly_source, now_ms, Artwork, Command, MediaBackend, Snapshot};
use std::collections::HashMap;
use zbus::blocking::{fdo::DBusProxy, Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const ROOT: &str = "org.mpris.MediaPlayer2";

pub struct LinuxBackend {
    connection: Option<Connection>,
    current: Option<String>,
    track_object: Option<String>,
    art_url: Option<String>,
}

impl LinuxBackend {
    pub fn new() -> Self {
        Self { connection: None, current: None, track_object: None, art_url: None }
    }

    fn connection(&mut self) -> Option<Connection> {
        if self.connection.is_none() {
            self.connection = Connection::session().ok();
        }
        self.connection.clone()
    }

    fn proxy<'a>(connection: &Connection, name: &'a str, interface: &'a str) -> zbus::Result<Proxy<'a>> {
        // Position changes continuously and isn't signalled, so never cache.
        zbus::blocking::proxy::Builder::<Proxy>::new(connection)
            .destination(name)?
            .path(PATH)?
            .interface(interface)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
    }

    fn players(connection: &Connection) -> Vec<String> {
        DBusProxy::new(connection)
            .ok()
            .and_then(|dbus| dbus.list_names().ok())
            .map(|names| {
                names
                    .into_iter()
                    .map(|n| n.to_string())
                    .filter(|n| n.starts_with(PREFIX))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn status(connection: &Connection, name: &str) -> Option<String> {
        Self::proxy(connection, name, PLAYER).ok()?.get_property::<String>("PlaybackStatus").ok()
    }

    /// Playing beats paused; among equals, stick with the player we used last.
    fn pick(&self, connection: &Connection) -> Option<String> {
        let mut playing = Vec::new();
        let mut paused = Vec::new();
        for name in Self::players(connection) {
            match Self::status(connection, &name).as_deref() {
                Some("Playing") => playing.push(name),
                Some("Paused") => paused.push(name),
                _ => {}
            }
        }
        let prefer = |names: &Vec<String>| {
            names
                .iter()
                .find(|n| Some(n.as_str()) == self.current.as_deref())
                .or_else(|| names.first())
                .cloned()
        };
        prefer(&playing).or_else(|| prefer(&paused))
    }

    fn read(&mut self, connection: &Connection, name: &str) -> zbus::Result<Option<Snapshot>> {
        let player = Self::proxy(connection, name, PLAYER)?;
        let metadata: HashMap<String, OwnedValue> = player.get_property("Metadata")?;
        let title = string(metadata.get("xesam:title")).unwrap_or_default();
        if title.trim().is_empty() {
            return Ok(None);
        }
        let artist = strings(metadata.get("xesam:artist")).join(", ");
        let album = string(metadata.get("xesam:album")).unwrap_or_default();
        let duration = number(metadata.get("mpris:length")).map(|us| us / 1e6).filter(|d| *d > 1.0);
        self.track_object = object_path(metadata.get("mpris:trackid"));
        self.art_url = string(metadata.get("mpris:artUrl")).filter(|u| !u.is_empty());

        let status: String = player.get_property("PlaybackStatus")?;
        let position = player.get_property::<i64>("Position").map(|us| us as f64 / 1e6).unwrap_or(0.0);
        let can_control = player.get_property::<bool>("CanControl").unwrap_or(true);
        let can_seek = player.get_property::<bool>("CanSeek").unwrap_or(false);

        let identity = Self::proxy(connection, name, ROOT)
            .and_then(|root| root.get_property::<String>("Identity"))
            .unwrap_or_else(|_| name.trim_start_matches(PREFIX).to_string());
        let source = friendly_source(&identity);

        Ok(Some(Snapshot {
            track_id: format!("{source}|{title}|{artist}"),
            source,
            title,
            artist,
            album,
            duration,
            position,
            timestamp_ms: now_ms(),
            is_playing: status == "Playing",
            can_control,
            can_seek: can_seek && can_control && duration.is_some(),
        }))
    }
}

impl MediaBackend for LinuxBackend {
    fn snapshot(&mut self) -> Option<Snapshot> {
        let connection = self.connection()?;
        let name = self.pick(&connection)?;
        let snapshot = self.read(&connection, &name).ok().flatten();
        self.current = Some(name);
        snapshot
    }

    fn artwork(&mut self) -> Option<Artwork> {
        self.art_url.clone().map(Artwork::Url)
    }

    fn send(&mut self, command: Command) {
        let (Some(connection), Some(name)) = (self.connection(), self.current.clone()) else { return };
        let Ok(player) = Self::proxy(&connection, &name, PLAYER) else { return };
        let _ = match command {
            Command::PlayPause => player.call_method("PlayPause", &()),
            Command::Next => player.call_method("Next", &()),
            Command::Previous => player.call_method("Previous", &()),
            Command::Seek(seconds) => {
                let Some(track) = self.track_object.as_deref().and_then(|t| ObjectPath::try_from(t).ok()) else { return };
                player.call_method("SetPosition", &(track, (seconds.max(0.0) * 1e6) as i64))
            }
        };
    }
}

fn string(value: Option<&OwnedValue>) -> Option<String> {
    match &**value? {
        Value::Str(s) => Some(s.to_string()),
        Value::ObjectPath(p) => Some(p.to_string()),
        _ => None,
    }
}

fn strings(value: Option<&OwnedValue>) -> Vec<String> {
    let Some(value) = value else { return Vec::new() };
    match &**value {
        Value::Str(s) => vec![s.to_string()],
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn number(value: Option<&OwnedValue>) -> Option<f64> {
    match &**value? {
        Value::I64(n) => Some(*n as f64),
        Value::U64(n) => Some(*n as f64),
        Value::I32(n) => Some(*n as f64),
        Value::U32(n) => Some(*n as f64),
        Value::F64(n) => Some(*n),
        _ => None,
    }
}

fn object_path(value: Option<&OwnedValue>) -> Option<String> {
    string(value)
}
