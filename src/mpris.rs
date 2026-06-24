use anyhow::{anyhow, Result};
use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::blocking::Connection;
use std::time::Duration;

pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length_secs: Option<f64>,
}

pub struct PlayerState {
    pub track: Track,
    pub position_secs: f64,
    pub playing: bool,
}

fn find_player_dest(conn: &Connection) -> Result<String> {
    let proxy = conn.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        Duration::from_millis(2000),
    );
    let (names,): (Vec<String>,) = proxy.method_call("org.freedesktop.DBus", "ListNames", ())?;

    let mut mpris_names: Vec<String> = names
        .into_iter()
        .filter(|n| n.starts_with("org.mpris.MediaPlayer2."))
        .collect();

    if let Some(pos) = mpris_names.iter().position(|n| n.contains("spotify")) {
        return Ok(mpris_names.remove(pos));
    }
    mpris_names
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no MPRIS player found"))
}

pub fn poll(conn: &Connection) -> Result<PlayerState> {
    let dest = find_player_dest(conn)?;
    let proxy = conn.with_proxy(dest, "/org/mpris/MediaPlayer2", Duration::from_millis(2000));

    let status: String = proxy.get("org.mpris.MediaPlayer2.Player", "PlaybackStatus")?;
    let position_us: i64 = proxy
        .get("org.mpris.MediaPlayer2.Player", "Position")
        .unwrap_or(0);
    let metadata: PropMap = proxy.get("org.mpris.MediaPlayer2.Player", "Metadata")?;

    let title = metadata
        .get("xesam:title")
        .and_then(|v| v.0.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let artist = metadata
        .get("xesam:artist")
        .and_then(|v| v.0.as_iter())
        .and_then(|mut it| it.next())
        .and_then(|a| a.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown".to_string());

    let album = metadata
        .get("xesam:album")
        .and_then(|v| v.0.as_str())
        .unwrap_or("")
        .to_string();

    let length_secs = metadata
        .get("mpris:length")
        .and_then(|v| v.0.as_i64())
        .map(|us| us as f64 / 1_000_000.0);

    Ok(PlayerState {
        track: Track { title, artist, album, length_secs },
        position_secs: position_us as f64 / 1_000_000.0,
        playing: status == "Playing",
    })
}
