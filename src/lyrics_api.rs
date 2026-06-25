use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
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

fn cache_path(title: &str, artist: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let safe = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
            .collect()
    };
    PathBuf::from(format!("{home}/.cache/lyrics/{}__{}.lrc", safe(artist), safe(title)))
}

fn load_cached(title: &str, artist: &str) -> Option<Lyrics> {
    let raw = fs::read_to_string(cache_path(title, artist)).ok()?;
    if let Some(plain) = raw.strip_prefix("PLAIN\n") {
        return (!plain.trim().is_empty()).then(|| Lyrics::Plain(plain.to_string()));
    }
    let lines = parse_lrc(&raw);
    (!lines.is_empty()).then_some(Lyrics::Synced(lines))
}

fn save_cached(title: &str, artist: &str, lyrics: &Lyrics, raw_synced: Option<&str>) {
    let path = cache_path(title, artist);
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let body = match (lyrics, raw_synced) {
        (Lyrics::Synced(_), Some(lrc)) => lrc.to_string(),
        (Lyrics::Plain(t), _) => format!("PLAIN\n{t}"),
        _ => return,
    };
    let _ = fs::write(path, body);
}

pub fn fetch(title: &str, artist: &str, album: &str, duration_secs: Option<f64>) -> Result<Lyrics> {
    if let Some(cached) = load_cached(title, artist) {
        return Ok(cached);
    }

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
                save_cached(title, artist, &result, Some(synced));
                return Ok(result);
            }
        }
    }

    if let Ok(Some(lrc)) = crate::netease::fetch_lrc(title, artist) {
        let lines = parse_lrc(&lrc);
        if !lines.is_empty() {
            let result = Lyrics::Synced(lines);
            save_cached(title, artist, &result, Some(&lrc));
            return Ok(result);
        }
    }

    if let Some(p) = parsed {
        if let Some(plain) = p.plain_lyrics {
            if !plain.trim().is_empty() {
                let result = Lyrics::Plain(plain);
                save_cached(title, artist, &result, None);
                return Ok(result);
            }
        }
    }

    if let Ok(Some(plain)) = crate::lyrics_ovh::fetch(title, artist) {
        let result = Lyrics::Plain(plain);
        save_cached(title, artist, &result, None);
        return Ok(result);
    }

    if let Ok(Some(plain)) = crate::genius::fetch(title, artist) {
        let result = Lyrics::Plain(plain);
        save_cached(title, artist, &result, None);
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
