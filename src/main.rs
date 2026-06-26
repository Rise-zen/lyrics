mod daemon;
mod font;
mod genius;
mod lyrics_api;
mod lyrics_ovh;
mod mpris;
mod netease;
mod theme;

use anyhow::Result;
use crossterm::{
    cursor, execute, terminal,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
};
use dbus::blocking::Connection;
use lyrics_api::Lyrics;
use std::io::{stdout, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::SeqCst))?;

    // Daemon mode: no terminal UI, just write /tmp/lyrics.json for the
    // quickshell panel (lyrics-panel) to render. Triggered by `lyrics --json`.
    if std::env::args().any(|a| a == "--json" || a == "--daemon") {
        return daemon::run(&running);
    }

    let mut out = stdout();
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;

    let result = run(&running);

    execute!(out, cursor::Show, terminal::LeaveAlternateScreen)?;
    result
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

/// Spawns a worker that owns all HTTP traffic to lrclib/netease so the render
/// loop never blocks. Render loop sends the track to fetch; worker replies
/// when done. Stale replies (track changed in the meantime) are ignored by
/// matching on `key`.
fn spawn_fetcher() -> (mpsc::Sender<FetchReq>, mpsc::Receiver<FetchResp>) {
    let (req_tx, req_rx) = mpsc::channel::<FetchReq>();
    let (res_tx, res_rx) = mpsc::channel::<FetchResp>();

    thread::spawn(move || {
        while let Ok(req) = req_rx.recv() {
            // Drain any queued requests; only act on the latest one so we
            // don't waste time fetching stale tracks the user already skipped.
            let mut latest = req;
            while let Ok(next) = req_rx.try_recv() {
                latest = next;
            }
            let lyrics = lyrics_api::fetch(&latest.title, &latest.artist, &latest.album, latest.length)
                .unwrap_or(Lyrics::NotFound);
            let _ = res_tx.send(FetchResp { key: latest.key, lyrics });
        }
    });

    (req_tx, res_rx)
}

struct Session {
    current_key: Option<(String, String)>,
    lyrics: Lyrics,
}

fn run(running: &AtomicBool) -> Result<()> {
    let conn = Connection::new_session()?;
    let palette = theme::load();
    let mut session = Session { current_key: None, lyrics: Lyrics::NotFound };
    let (req_tx, res_rx) = spawn_fetcher();

    while running.load(Ordering::SeqCst) {
        // Drain background fetch results: keep only the freshest one that
        // still matches the currently-playing track.
        while let Ok(resp) = res_rx.try_recv() {
            if session.current_key.as_ref() == Some(&resp.key) {
                session.lyrics = resp.lyrics;
            }
        }

        match mpris::poll(&conn) {
            Ok(state) => {
                let key = (state.track.title.clone(), state.track.artist.clone());
                if session.current_key.as_ref() != Some(&key) {
                    session.current_key = Some(key.clone());
                    session.lyrics = Lyrics::NotFound;
                    let _ = req_tx.send(FetchReq {
                        key,
                        title: state.track.title.clone(),
                        artist: state.track.artist.clone(),
                        album: state.track.album.clone(),
                        length: state.track.length_secs,
                    });
                }
                render(&state, &session.lyrics, &palette)?;
            }
            Err(_) => {
                session.current_key = None;
                render_idle()?;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn render(state: &mpris::PlayerState, lyrics: &Lyrics, palette: &theme::Palette) -> Result<()> {
    let mut out = stdout();
    let (cols, rows) = terminal::size()?;
    execute!(out, terminal::Clear(terminal::ClearType::All))?;

    let accent = theme::pick_accent(palette, &state.track.title, &state.track.artist);

    let status = if state.playing { "▶" } else { "⏸" };
    let header = format!("{status} {} — {}", state.track.title, state.track.artist);
    print_centered(&mut out, cols, 1, &header, Some(accent), true)?;

    match lyrics {
        Lyrics::Synced(lines) => {
            let pos = state.position_secs;
            let idx = current_index(lines, pos);
            let next_time = lines.get(idx + 1).map(|l| l.time);
            let before_first = pos < lines[0].time;

            let in_gap = next_time
                .map(|t| t - pos > 4.0 && pos - lines[idx].time > 2.0)
                .unwrap_or(false);

            if before_first || in_gap {
                render_music_waiting(&mut out, cols, rows, palette)?;
            } else {
                render_block_line(&mut out, cols, rows, &lines[idx].text, accent)?;
            }
        }
        Lyrics::Plain(text) => {
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            if lines.is_empty() {
                render_music_waiting(&mut out, cols, rows, palette)?;
            } else {
                let idx = plain_line_index(&lines, state.position_secs, state.track.length_secs);
                render_block_line(&mut out, cols, rows, lines[idx], accent)?;
            }
        }
        Lyrics::NotFound => {
            render_music_waiting(&mut out, cols, rows, palette)?;
        }
    }
    out.flush()?;
    Ok(())
}

/// Renders a single lyric line as huge block letters, centered both
/// horizontally and vertically — the "one giant line at a time" look of
/// tacoproz1/tacos-terminal-lyrics, instead of a scrolling list.
fn render_block_line(out: &mut Stdout, cols: u16, rows: u16, text: &str, color: Color) -> Result<()> {
    let glyph_rows = font::render(text);
    let block_width = glyph_rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u16;
    let block_height = glyph_rows.len() as u16;

    let top = if block_height < rows { (rows - block_height) / 2 } else { 0 };
    let left = if block_width < cols { (cols - block_width) / 2 } else { 0 };

    for (i, line) in glyph_rows.iter().enumerate() {
        let clipped: String = line.chars().take(cols as usize).collect();
        execute!(out, cursor::MoveTo(left, top + i as u16))?;
        execute!(out, SetForegroundColor(color), Print(clipped), ResetColor)?;
    }
    Ok(())
}

/// Single music note centered on screen, smoothly cycling color through every
/// vibrant accent in the palette. Uses sub-second RGB interpolation between
/// adjacent palette entries so the transition feels continuous, not stepped.
fn render_music_waiting(out: &mut Stdout, cols: u16, rows: u16, palette: &theme::Palette) -> Result<()> {
    let note = "♫";
    let row = rows / 2;
    let col = cols / 2;

    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_millis() as f64;
    let period = 2200.0;
    let phase = now_ms / period;
    let n = palette.accents.len().max(1);

    let i = (phase as usize) % n;
    let j = (i + 1) % n;
    let t = phase.fract();

    let color = lerp_color(palette.accents[i], palette.accents[j], t);

    execute!(out, cursor::MoveTo(col, row))?;
    execute!(out, SetForegroundColor(color), Print(note), ResetColor)?;
    Ok(())
}

fn lerp_color(a: Color, b: Color, t: f64) -> Color {
    let (ar, ag, ab) = rgb(a);
    let (br, bg, bb) = rgb(b);
    let mix = |x: u8, y: u8| ((x as f64) * (1.0 - t) + (y as f64) * t).round() as u8;
    Color::Rgb { r: mix(ar, br), g: mix(ag, bg), b: mix(ab, bb) }
}

fn rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb { r, g, b } => (r, g, b),
        _ => (200, 200, 200),
    }
}

fn current_index(lines: &[lyrics_api::LyricLine], position: f64) -> usize {
    lines.iter().rposition(|l| l.time <= position).unwrap_or(0)
}

fn plain_line_index(lines: &[&str], position: f64, length: Option<f64>) -> usize {
    let total = length.unwrap_or(0.0);
    if total <= 0.0 || lines.is_empty() {
        return 0;
    }
    let frac = (position / total).clamp(0.0, 0.999);
    (frac * lines.len() as f64) as usize
}

fn print_centered(out: &mut Stdout, cols: u16, row: u16, text: &str, color: Option<Color>, bold: bool) -> Result<()> {
    let len = text.chars().count() as u16;
    let col = if len < cols { (cols - len) / 2 } else { 0 };
    execute!(out, cursor::MoveTo(col, row))?;
    if bold {
        execute!(out, SetAttribute(Attribute::Bold))?;
    }
    if let Some(c) = color {
        execute!(out, SetForegroundColor(c))?;
    }
    execute!(out, Print(text), ResetColor, SetAttribute(Attribute::Reset))?;
    Ok(())
}

fn render_idle() -> Result<()> {
    let mut out = stdout();
    let (cols, rows) = terminal::size()?;
    execute!(out, terminal::Clear(terminal::ClearType::All))?;
    print_centered(&mut out, cols, rows / 2, "Ничего не играет...", None, false)?;
    out.flush()?;
    Ok(())
}
