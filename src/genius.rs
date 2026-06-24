use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

const UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

#[derive(Deserialize)]
struct SearchResp {
    response: SearchInner,
}

#[derive(Deserialize)]
struct SearchInner {
    sections: Vec<Section>,
}

#[derive(Deserialize)]
struct Section {
    hits: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    result: HitResult,
}

#[derive(Deserialize)]
struct HitResult {
    url: String,
    artist_names: Option<String>,
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(10))
        .build()?)
}

/// Searches Genius via its internal autocomplete endpoint and scrapes the
/// matching song page for plain lyrics. No sync (Genius doesn't expose
/// timecodes) — purely a coverage boost for tracks the other sources miss.
pub fn fetch(title: &str, artist: &str) -> Result<Option<String>> {
    let c = client()?;
    let q = format!("{title} {artist}");
    let search: SearchResp = c
        .get("https://genius.com/api/search/song")
        .header("Accept", "application/json")
        .query(&[("q", q.as_str())])
        .send()?
        .json()?;

    let artist_l = artist.to_lowercase();
    let best = search
        .response
        .sections
        .into_iter()
        .flat_map(|s| s.hits)
        .max_by_key(|h| {
            let names = h
                .result
                .artist_names
                .as_deref()
                .unwrap_or("")
                .to_lowercase();
            artist_l.split_whitespace().filter(|w| names.contains(w)).count()
        });

    let Some(hit) = best else { return Ok(None) };

    let html = c.get(&hit.result.url).send()?.text()?;
    Ok(extract_lyrics(&html))
}

fn extract_lyrics(html: &str) -> Option<String> {
    // Genius wraps each lyric block in: <div data-lyrics-container="true">...</div>
    // (modern layout). Concatenate every match.
    let mut out = String::new();
    let needle = "data-lyrics-container=\"true\"";
    let mut cursor = 0;
    while let Some(idx) = html[cursor..].find(needle) {
        let abs = cursor + idx;
        let start = html[abs..].find('>')? + abs + 1;
        let end = html[start..].find("</div>")? + start;
        let block = &html[start..end];
        out.push_str(&strip_html(block));
        out.push('\n');
        cursor = end;
    }
    let trimmed = out.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

fn strip_html(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let tag_lower: String = s[i..i.saturating_add(4).min(s.len())].to_lowercase();
            if tag_lower.starts_with("<br") {
                out.push('\n');
            }
            // Skip until '>'
            while i < bytes.len() && bytes[i] != b'>' { i += 1; }
            i += 1;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    decode_entities(&out)
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}
