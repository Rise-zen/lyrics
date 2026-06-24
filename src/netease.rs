use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct SearchResp {
    result: Option<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    #[serde(default)]
    songs: Vec<SearchSong>,
}

#[derive(Deserialize)]
struct SearchSong {
    id: i64,
    name: String,
    #[serde(default)]
    artists: Vec<Artist>,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
}

#[derive(Deserialize)]
struct LyricResp {
    lrc: Option<LyricBody>,
}

#[derive(Deserialize)]
struct LyricBody {
    #[serde(default)]
    lyric: String,
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(Duration::from_secs(10))
        .build()?)
}

/// Searches Netease for `title artist` and returns the synced LRC string of
/// the best-matching result, if any. Match scoring: artist substring match
/// beats title-only; ties broken by search order (Netease's own ranking).
pub fn fetch_lrc(title: &str, artist: &str) -> Result<Option<String>> {
    let c = client()?;
    let query = format!("{title} {artist}");

    let search: SearchResp = c
        .get("https://music.163.com/api/search/get")
        .header("Referer", "https://music.163.com")
        .query(&[("s", query.as_str()), ("type", "1"), ("limit", "10")])
        .send()?
        .json()?;

    let songs = match search.result {
        Some(r) => r.songs,
        None => return Ok(None),
    };

    let artist_l = artist.to_lowercase();
    let title_l = title.to_lowercase();

    let best = songs.iter().max_by_key(|s| {
        let artists_l: String = s.artists.iter().map(|a| a.name.to_lowercase()).collect::<Vec<_>>().join(" ");
        let name_l = s.name.to_lowercase();
        let artist_hit = artist_l.split_whitespace().any(|w| artists_l.contains(w));
        let title_hit = title_l.split_whitespace().any(|w| name_l.contains(w));
        (artist_hit as u8) * 2 + (title_hit as u8)
    });

    let Some(song) = best else { return Ok(None) };

    let lyr: LyricResp = c
        .get("https://music.163.com/api/song/lyric")
        .header("Referer", "https://music.163.com")
        .query(&[("id", song.id.to_string().as_str()), ("lv", "1"), ("kv", "1"), ("tv", "-1")])
        .send()?
        .json()?;

    let lrc = lyr.lrc.map(|b| b.lyric).unwrap_or_default();
    if lrc.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(lrc))
    }
}
