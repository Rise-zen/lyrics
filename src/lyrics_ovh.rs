use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct Resp {
    lyrics: Option<String>,
}

/// Plain-text fallback via lyrics.ovh — no auth, no sync, decent coverage for
/// English/European pop. Returns None if not found or response is empty.
pub fn fetch(title: &str, artist: &str) -> Result<Option<String>> {
    let url = format!(
        "https://api.lyrics.ovh/v1/{}/{}",
        urlencoding(artist),
        urlencoding(title),
    );
    let resp = reqwest::blocking::Client::builder()
        .user_agent("lyrics-cli/0.1")
        .timeout(Duration::from_secs(8))
        .build()?
        .get(url)
        .send()?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let parsed: Resp = resp.json()?;
    Ok(parsed.lyrics.filter(|s| !s.trim().is_empty()))
}

fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}
