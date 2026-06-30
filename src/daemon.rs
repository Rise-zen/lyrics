use anyhow::Result;
use dbus::blocking::Connection;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::lyrics_api::{self, Lyrics};
use crate::mpris;
use crate::theme;

/// JSON state file the quickshell panel (lyrics-panel) polls. Kept in /tmp so
/// it's wiped on reboot and never pollutes the user's home.
fn state_path() -> PathBuf {
    PathBuf::from("/tmp/lyrics.json")
}

#[derive(Serialize)]
struct LineOut {
    time: f64,
    text: String,
}

#[derive(Serialize, serde::Deserialize, Clone)]
struct RecentTrack {
    title: String,
    artist: String,
    cover: String,
    accent: String,
    /// A URI the QML can hand to `playerctl open` to jump back to this track.
    /// Empty if the player isn't Spotify (other MPRIS players don't reliably
    /// support OpenUri for past tracks).
    uri: String,
}

/// Build a Spotify `spotify:track:<id>` URI from a raw mpris:trackid path like
/// `/com/spotify/track/<id>`. Returns empty string for other formats.
fn spotify_uri_from_trackid(trackid: &str) -> String {
    if let Some(id) = trackid.strip_prefix("/com/spotify/track/") {
        return format!("spotify:track:{id}");
    }
    String::new()
}

#[derive(Serialize)]
struct StateOut {
    playing: bool,
    title: String,
    artist: String,
    position: f64,
    length: f64,
    /// "synced" | "plain" | "none"
    kind: &'static str,
    accent: String,
    /// absolute file path to the cover art, or "" if no art is available
    cover: String,
    lines: Vec<LineOut>,
    /// Rolling history of the most recent N tracks the daemon has seen,
    /// newest first. The orbital-clock QML reads this to populate cover
    /// bubbles around the clock.
    recent: Vec<RecentTrack>,
}

struct FetchReq {
    key: (String, String),
    title: String,
    artist: String,
    album: String,
    length: Option<f64>,
}

struct FetchResp {
    key: (String, String),
    lyrics: Lyrics,
}

fn spawn_fetcher() -> (mpsc::Sender<FetchReq>, mpsc::Receiver<FetchResp>) {
    let (req_tx, req_rx) = mpsc::channel::<FetchReq>();
    let (res_tx, res_rx) = mpsc::channel::<FetchResp>();

    thread::spawn(move || {
        while let Ok(req) = req_rx.recv() {
            let mut latest = req;
            while let Ok(next) = req_rx.try_recv() {
                latest = next;
            }
            let lyrics =
                lyrics_api::fetch(&latest.title, &latest.artist, &latest.album, latest.length)
                    .unwrap_or(Lyrics::NotFound);
            let _ = res_tx.send(FetchResp { key: latest.key, lyrics });
        }
    });

    (req_tx, res_rx)
}

fn color_to_hex(c: crossterm::style::Color) -> String {
    match c {
        crossterm::style::Color::Rgb { r, g, b } => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "#89b4fa".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Cover-art accent extraction
// ---------------------------------------------------------------------------

struct ArtReq {
    key: (String, String),
    art_url: String,
}

struct ArtResp {
    key: (String, String),
    accent: String,
    cover_path: String,
}

/// Worker thread: turns an MPRIS artUrl into a vibrant accent hex. Downloads
/// http(s) art to a cache, then uses ImageMagick (already a dep of the bar) to
/// quantize the cover to its dominant colors and picks the punchiest one.
fn spawn_art_worker() -> (mpsc::Sender<ArtReq>, mpsc::Receiver<ArtResp>) {
    let (req_tx, req_rx) = mpsc::channel::<ArtReq>();
    let (res_tx, res_rx) = mpsc::channel::<ArtResp>();

    thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .ok();

        while let Ok(req) = req_rx.recv() {
            // only act on the most recent track
            let mut latest = req;
            while let Ok(next) = req_rx.try_recv() {
                latest = next;
            }

            if let Some(path) = resolve_art(&latest.art_url, client.as_ref()) {
                let accent = extract_accent(&path).unwrap_or_else(|| "#89b4fa".to_string());
                let cover_path = path.to_string_lossy().to_string();
                let _ = res_tx.send(ArtResp {
                    key: latest.key,
                    accent,
                    cover_path,
                });
            }
        }
    });

    (req_tx, res_rx)
}

/// Returns a local file path for the cover, downloading http(s) art into
/// /tmp/lyrics-art if needed. file:// and bare paths are used directly.
fn resolve_art(url: &str, client: Option<&reqwest::blocking::Client>) -> Option<PathBuf> {
    if url.is_empty() {
        return None;
    }
    if let Some(p) = url.strip_prefix("file://") {
        return Some(PathBuf::from(p));
    }
    if !url.starts_with("http") {
        return Some(PathBuf::from(url));
    }

    let client = client?;
    let cache_dir = PathBuf::from("/tmp/lyrics-art");
    let _ = fs::create_dir_all(&cache_dir);
    let name = format!("{:x}.img", fnv(url.as_bytes()));
    let dest = cache_dir.join(name);
    if dest.exists() {
        return Some(dest);
    }
    let bytes = client.get(url).send().ok()?.bytes().ok()?;
    fs::write(&dest, &bytes).ok()?;
    Some(dest)
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// Quantizes the cover to 6 colors via ImageMagick and scores each candidate
/// by saturation × log(pixel count), then normalizes it into a bright,
/// readable accent (same idea as astrium's vivid()).
fn extract_accent(path: &std::path::Path) -> Option<String> {
    let out = std::process::Command::new("convert")
        .arg(path)
        .args([
            "-resize", "80x80", "-alpha", "off", "+dither",
            "-quantize", "RGB", "-colors", "6",
            "-format", "%c", "histogram:info:",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);

    let mut best: Option<(f32, (u8, u8, u8))> = None;
    for line in text.lines() {
        // lines look like:  "  1234: (r,g,b) #RRGGBB srgb(...)"
        let count: f32 = line
            .trim()
            .split(':')
            .next()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(0.0);
        let hex = line
            .split('#')
            .nth(1)
            .map(|s| &s[..s.len().min(6)])
            .unwrap_or("");
        let Some(rgb) = parse_hex6(hex) else { continue };
        let (_, s, _) = rgb_to_hsl(rgb);
        let score = s * (count + 1.0).ln();
        if best.map_or(true, |(b, _)| score > b) {
            best = Some((score, rgb));
        }
    }

    best.map(|(_, rgb)| {
        let (r, g, b) = vivid(rgb);
        format!("#{r:02x}{g:02x}{b:02x}")
    })
}

fn parse_hex6(hex: &str) -> Option<(u8, u8, u8)> {
    if hex.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

/// Boost a washed cover color into a bright, readable accent.
fn vivid(c: (u8, u8, u8)) -> (u8, u8, u8) {
    let (h, s, l) = rgb_to_hsl(c);
    hsl_to_rgb(h, s.max(0.55), l.clamp(0.55, 0.72))
}

fn rgb_to_hsl(c: (u8, u8, u8)) -> (f32, f32, f32) {
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * (((b - r) / d) + 2.0)
    } else {
        60.0 * (((r - g) / d) + 4.0)
    };
    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

fn write_state(state: &StateOut) {
    let Ok(json) = serde_json::to_string(state) else { return };
    // Atomic write: tmp + rename so the panel never reads a half-written file.
    let tmp = state_path().with_extension("json.tmp");
    if let Ok(mut f) = fs::File::create(&tmp) {
        if f.write_all(json.as_bytes()).is_ok() {
            let _ = fs::rename(&tmp, state_path());
        }
    }
}

fn build_lines(lyrics: &Lyrics) -> (Vec<LineOut>, &'static str) {
    match lyrics {
        Lyrics::Synced(lines) => (
            lines
                .iter()
                .map(|l| LineOut { time: l.time, text: l.text.clone() })
                .collect(),
            "synced",
        ),
        Lyrics::Plain(text) => (
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| LineOut { time: -1.0, text: l.to_string() })
                .collect(),
            "plain",
        ),
        Lyrics::NotFound => (Vec::new(), "none"),
    }
}

pub fn run(running: &AtomicBool) -> Result<()> {
    let conn = Connection::new_session()?;
    let palette = theme::load();
    let (req_tx, res_rx) = spawn_fetcher();
    let (art_tx, art_rx) = spawn_art_worker();

    let mut current_key: Option<(String, String)> = None;
    let mut cached_lines: Vec<LineOut> = Vec::new();
    let mut cached_kind: &'static str = "none";
    // accent derived from the cover art; falls back to the astrium palette
    // until the art worker reports back for the current track.
    let mut art_accent: Option<String> = None;
    let mut cover_path: String = String::new();
    // Rolling history of recently seen tracks (newest first). A new entry is
    // pushed whenever a track gains both a known cover and an accent — that
    // way we don't fill the orbit with greys before the art worker resolves.
    let mut recent: Vec<RecentTrack> = Vec::new();
    let mut last_history_key: Option<(String, String)> = None;
    let history_limit = 12;

    while running.load(Ordering::SeqCst) {
        while let Ok(resp) = res_rx.try_recv() {
            if current_key.as_ref() == Some(&resp.key) {
                let (l, k) = build_lines(&resp.lyrics);
                cached_lines = l;
                cached_kind = k;
            }
        }
        while let Ok(resp) = art_rx.try_recv() {
            if current_key.as_ref() == Some(&resp.key) {
                art_accent = Some(resp.accent);
                cover_path = resp.cover_path;
            }
        }

        match mpris::poll(&conn) {
            Ok(state) => {
                let key = (state.track.title.clone(), state.track.artist.clone());
                if current_key.as_ref() != Some(&key) {
                    current_key = Some(key.clone());
                    cached_lines = Vec::new();
                    cached_kind = "none";
                    art_accent = None;
                    cover_path = String::new();
                    let _ = req_tx.send(FetchReq {
                        key: key.clone(),
                        title: state.track.title.clone(),
                        artist: state.track.artist.clone(),
                        album: state.track.album.clone(),
                        length: state.track.length_secs,
                    });
                    let _ = art_tx.send(ArtReq {
                        key,
                        art_url: state.track.art_url.clone(),
                    });
                }

                // cover accent wins; astrium palette is the fallback
                let accent = art_accent.clone().unwrap_or_else(|| {
                    color_to_hex(theme::pick_accent(
                        &palette,
                        &state.track.title,
                        &state.track.artist,
                    ))
                });

                // Promote the current track into the history once we have a
                // real cover AND a click-to-jump URI. The URI gate filters out
                // Chromium/Firefox MPRIS sessions (YouTube/SoundCloud tabs) —
                // those expose tab favicons as cover art but have no
                // spotify:track:* identifier, so they'd just clutter the orbit
                // with browser icons.
                let uri = spotify_uri_from_trackid(&state.track.track_id);
                if !cover_path.is_empty()
                    && !uri.is_empty()
                    && last_history_key.as_ref() != current_key.as_ref()
                {
                    if let Some(k) = current_key.clone() {
                        let entry = RecentTrack {
                            title: state.track.title.clone(),
                            artist: state.track.artist.clone(),
                            cover: cover_path.clone(),
                            accent: accent.clone(),
                            uri,
                        };
                        recent.retain(|t| !(t.title == entry.title && t.artist == entry.artist));
                        recent.insert(0, entry);
                        recent.truncate(history_limit);
                        last_history_key = Some(k);
                    }
                }

                let out = StateOut {
                    playing: state.playing,
                    title: state.track.title.clone(),
                    artist: state.track.artist.clone(),
                    position: state.position_secs,
                    length: state.track.length_secs.unwrap_or(0.0),
                    kind: cached_kind,
                    accent,
                    cover: cover_path.clone(),
                    lines: cached_lines
                        .iter()
                        .map(|l| LineOut { time: l.time, text: l.text.clone() })
                        .collect(),
                    recent: recent.clone(),
                };
                write_state(&out);
            }
            Err(_) => {
                current_key = None;
                let out = StateOut {
                    playing: false,
                    title: String::new(),
                    artist: String::new(),
                    position: 0.0,
                    length: 0.0,
                    kind: "none",
                    accent: "#89b4fa".to_string(),
                    cover: String::new(),
                    lines: Vec::new(),
                    recent: recent.clone(),
                };
                write_state(&out);
            }
        }

        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}
