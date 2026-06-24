# lyrics

Terminal lyrics display. Detects the currently playing track via MPRIS, fetches
synced lyrics from multiple sources, and renders one giant line at a time in
the colors of your wallpaper-derived theme.

![demo placeholder](demo.gif)

## Features

- **5 lyrics sources** with automatic fallback:
  - [lrclib](https://lrclib.net) — synced, English/global pop
  - [Netease Music](https://music.163.com) — synced, huge Russian/Asian catalog
  - lrclib plain — when synced isn't available
  - [lyrics.ovh](https://lyrics.ovh) — plain, English/European
  - [Genius](https://genius.com) — scraped HTML, hip-hop/underground
- **MPRIS auto-detection** — works with any MPRIS-compliant player
  (Spotify, mpv, VLC, Audacious, Strawberry, …)
- **Big block-letter rendering** — one line at a time, centered, full-width
- **Full Cyrillic support** in the block font
- **Animated music note** during instrumentals and gaps, smoothly cycling
  through the active palette
- **Background fetcher** — HTTP never blocks the render loop, UI stays
  responsive at 100ms poll interval
- **Theme-aware** — pulls accent colors from
  [refract](https://github.com/Rise-zen/refract)'s palette so colors follow
  your wallpaper; each track gets a deterministic color from the palette
- **Single static binary** — no daemon, no runtime, ~5 MB

## Install

```bash
git clone https://github.com/Rise-zen/lyrics
cd lyrics
cargo build --release
cp target/release/lyrics ~/.local/bin/
```

Then just run:

```bash
lyrics
```

## How it picks lyrics

For each track change the background worker tries each source in order until
one returns lyrics, then displays them. Synced sources win over plain. Failed
lookups are not retried within the same session.

## Theming

If you use [refract](https://github.com/Rise-zen/refract), `lyrics` reads
`~/.cache/refract/colors.json` and uses `color1`–`color6` + `color9`–`color14`
as the accent pool. Otherwise it falls back to terminal cyan.

## License

MIT
