use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct LrcLibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

pub struct LyricLine {
    pub time: f64,
    pub text: String,
}

pub enum Lyrics {
    Synced(Vec<LyricLine>),
    Plain(String),
    NotFound,
}

pub fn fetch(title: &str, artist: &str, album: &str, duration_secs: Option<f64>) -> Result<Lyrics> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("lyrics-cli/0.1")
        .timeout(Duration::from_secs(10))
        .build()?;

    let mut query = vec![
        ("track_name".to_string(), title.to_string()),
        ("artist_name".to_string(), artist.to_string()),
        ("album_name".to_string(), album.to_string()),
    ];
    if let Some(d) = duration_secs {
        query.push(("duration".to_string(), (d.round() as i64).to_string()));
    }

    let resp = client.get("https://lrclib.net/api/get").query(&query).send()?;
    let parsed: Option<LrcLibResponse> = if resp.status().is_success() {
        resp.json().ok()
    } else {
        None
    };

    if let Some(p) = &parsed {
        if let Some(synced) = &p.synced_lyrics {
            let lines = parse_lrc(synced);
            if !lines.is_empty() {
                let result = Lyrics::Synced(lines);
                return Ok(result);
            }
        }
    }

    // Netease has much better coverage for Russian-language tracks than lrclib
    // and returns synced LRC, so try it before falling back to plain text.
    if let Ok(Some(lrc)) = crate::netease::fetch_lrc(title, artist) {
        let lines = parse_lrc(&lrc);
        if !lines.is_empty() {
            let result = Lyrics::Synced(lines);
            return Ok(result);
        }
    }

    if let Some(p) = parsed {
        if let Some(plain) = p.plain_lyrics {
            if !plain.trim().is_empty() {
                let result = Lyrics::Plain(plain);
                return Ok(result);
            }
        }
    }

    // lyrics.ovh — no sync, decent English/Euro pop coverage.
    if let Ok(Some(plain)) = crate::lyrics_ovh::fetch(title, artist) {
        let result = Lyrics::Plain(plain);
        return Ok(result);
    }

    // Genius — huge catalog including hip-hop/underground, scraped from HTML.
    if let Ok(Some(plain)) = crate::genius::fetch(title, artist) {
        let result = Lyrics::Plain(plain);
        return Ok(result);
    }

    Ok(Lyrics::NotFound)
}

fn parse_lrc(raw: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if !line.starts_with('[') {
            continue;
        }
        if let Some(close) = line.find(']') {
            let ts = &line[1..close];
            let text = line[close + 1..].trim().to_string();
            if let Some(secs) = parse_timestamp(ts) {
                lines.push(LyricLine { time: secs, text });
            }
        }
    }
    lines.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    lines
}

fn parse_timestamp(ts: &str) -> Option<f64> {
    let (min, rest) = ts.split_once(':')?;
    let min: f64 = min.parse().ok()?;
    let sec: f64 = rest.parse().ok()?;
    Some(min * 60.0 + sec)
}
