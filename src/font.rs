//! Big block-letter renderer for lyric lines.
//!
//! Glyphs are authored on a 10-row pixel grid but drawn with half-block
//! characters, so two pixel rows share one terminal row. That keeps the on
//! screen height at `HEIGHT` while doubling the vertical detail, and it also
//! evens out stroke weight: a terminal cell is roughly twice as tall as it is
//! wide, so a half-cell-tall horizontal bar reads the same thickness as a
//! one-cell-wide vertical stem.
//!
//! Widths are per-glyph rather than fixed, otherwise narrow letters like `I`
//! float in a pocket of empty space while `M` and `W` feel cramped.

/// Rows of terminal output produced by [`render`].
pub const HEIGHT: usize = 5;

/// Rows in the authored pixel grid. Always `HEIGHT * 2`.
const PX_ROWS: usize = HEIGHT * 2;

type Glyph = [&'static str; PX_ROWS];

const A: Glyph = [
    "  ##  ", " #  # ", " #  # ", "#    #", "#    #",
    "######", "#    #", "#    #", "#    #", "#    #",
];
const B: Glyph = [
    "##### ", "#    #", "#    #", "#    #", "##### ",
    "#    #", "#    #", "#    #", "#    #", "##### ",
];
const C: Glyph = [
    " #### ", "#    #", "#     ", "#     ", "#     ",
    "#     ", "#     ", "#     ", "#    #", " #### ",
];
const D: Glyph = [
    "##### ", "#    #", "#    #", "#    #", "#    #",
    "#    #", "#    #", "#    #", "#    #", "##### ",
];
const E: Glyph = [
    "######", "#     ", "#     ", "#     ", "##### ",
    "#     ", "#     ", "#     ", "#     ", "######",
];
const F: Glyph = [
    "######", "#     ", "#     ", "#     ", "##### ",
    "#     ", "#     ", "#     ", "#     ", "#     ",
];
const G: Glyph = [
    " #### ", "#    #", "#     ", "#     ", "#     ",
    "#  ###", "#    #", "#    #", "#    #", " #### ",
];
const H: Glyph = [
    "#    #", "#    #", "#    #", "#    #", "######",
    "#    #", "#    #", "#    #", "#    #", "#    #",
];
const I: Glyph = [
    "###", " # ", " # ", " # ", " # ",
    " # ", " # ", " # ", " # ", "###",
];
const J: Glyph = [
    "   ##", "    #", "    #", "    #", "    #",
    "    #", "    #", "#   #", "#   #", " ### ",
];
const K: Glyph = [
    "#    #", "#   # ", "#  #  ", "# #   ", "##    ",
    "##    ", "# #   ", "#  #  ", "#   # ", "#    #",
];
const L: Glyph = [
    "#     ", "#     ", "#     ", "#     ", "#     ",
    "#     ", "#     ", "#     ", "#     ", "######",
];
const M: Glyph = [
    "#      #", "##    ##", "# #  # #", "# #  # #", "#  ##  #",
    "#      #", "#      #", "#      #", "#      #", "#      #",
];
const N: Glyph = [
    "#    #", "##   #", "##   #", "# #  #", "# #  #",
    "#  # #", "#  # #", "#   ##", "#   ##", "#    #",
];
const O: Glyph = [
    " #### ", "#    #", "#    #", "#    #", "#    #",
    "#    #", "#    #", "#    #", "#    #", " #### ",
];
const P: Glyph = [
    "##### ", "#    #", "#    #", "#    #", "#    #",
    "##### ", "#     ", "#     ", "#     ", "#     ",
];
const Q: Glyph = [
    " #### ", "#    #", "#    #", "#    #", "#    #",
    "#    #", "#    #", "#  # #", "#   # ", " ### #",
];
const R: Glyph = [
    "##### ", "#    #", "#    #", "#    #", "#    #",
    "##### ", "#  #  ", "#   # ", "#    #", "#    #",
];
const S: Glyph = [
    " #### ", "#    #", "#     ", "#     ", " #### ",
    "     #", "     #", "     #", "#    #", " #### ",
];
const T: Glyph = [
    "#####", "  #  ", "  #  ", "  #  ", "  #  ",
    "  #  ", "  #  ", "  #  ", "  #  ", "  #  ",
];
const U: Glyph = [
    "#    #", "#    #", "#    #", "#    #", "#    #",
    "#    #", "#    #", "#    #", "#    #", " #### ",
];
const V: Glyph = [
    "#    #", "#    #", "#    #", "#    #", " #  # ",
    " #  # ", " #  # ", "  ##  ", "  ##  ", "  ##  ",
];
const W: Glyph = [
    "#      #", "#      #", "#      #", "#  ##  #", "#  ##  #",
    "# #  # #", "# #  # #", "##    ##", "##    ##", "#      #",
];
const X: Glyph = [
    "#    #", "#    #", " #  # ", " #  # ", "  ##  ",
    "  ##  ", " #  # ", " #  # ", "#    #", "#    #",
];
const Y: Glyph = [
    "#    #", "#    #", " #  # ", " #  # ", "  ##  ",
    "  ##  ", "  ##  ", "  ##  ", "  ##  ", "  ##  ",
];
const Z: Glyph = [
    "######", "     #", "    # ", "    # ", "   #  ",
    "  #   ", "  #   ", " #    ", "#     ", "######",
];

const D0: Glyph = [
    " #### ", "#    #", "#   ##", "#   ##", "#  # #",
    "# #  #", "##   #", "##   #", "#    #", " #### ",
];
const D1: Glyph = [
    "  #  ", " ##  ", "# #  ", "  #  ", "  #  ",
    "  #  ", "  #  ", "  #  ", "  #  ", "#####",
];
const D2: Glyph = [
    " #### ", "#    #", "     #", "     #", "    # ",
    "   #  ", "  #   ", " #    ", "#     ", "######",
];
const D3: Glyph = [
    " #### ", "#    #", "     #", "     #", " #### ",
    "     #", "     #", "     #", "#    #", " #### ",
];
const D4: Glyph = [
    "#    #", "#    #", "#    #", "#    #", "#    #",
    "######", "     #", "     #", "     #", "     #",
];
const D5: Glyph = [
    "######", "#     ", "#     ", "#     ", "##### ",
    "     #", "     #", "     #", "#    #", " #### ",
];
const D6: Glyph = [
    " #### ", "#    #", "#     ", "#     ", "##### ",
    "#    #", "#    #", "#    #", "#    #", " #### ",
];
const D7: Glyph = [
    "######", "     #", "    # ", "    # ", "   #  ",
    "   #  ", "  #   ", "  #   ", " #    ", " #    ",
];
const D8: Glyph = [
    " #### ", "#    #", "#    #", "#    #", " #### ",
    "#    #", "#    #", "#    #", "#    #", " #### ",
];
const D9: Glyph = [
    " #### ", "#    #", "#    #", "#    #", " #####",
    "     #", "     #", "     #", "#    #", " #### ",
];

const APOS: Glyph = ["##", "##", "##", "  ", "  ", "  ", "  ", "  ", "  ", "  "];
const COMMA: Glyph = ["   ", "   ", "   ", "   ", "   ", "   ", "   ", " ##", " ##", "## "];
const DOT: Glyph = ["  ", "  ", "  ", "  ", "  ", "  ", "  ", "  ", "##", "##"];
const BANG: Glyph = ["##", "##", "##", "##", "##", "##", "  ", "  ", "##", "##"];
const QUERY: Glyph = [
    " #### ", "#    #", "     #", "     #", "   ## ",
    "  #   ", "  #   ", "      ", "  ##  ", "  ##  ",
];
const DASH: Glyph = ["     ", "     ", "     ", "     ", "#####", "     ", "     ", "     ", "     ", "     "];
const SPACE: Glyph = ["   "; PX_ROWS];
const LPAREN: Glyph = ["  #", " # ", "#  ", "#  ", "#  ", "#  ", "#  ", "#  ", " # ", "  #"];
const RPAREN: Glyph = ["#  ", " # ", "  #", "  #", "  #", "  #", "  #", "  #", " # ", "#  "];
const COLON: Glyph = ["  ", "  ", "##", "##", "  ", "  ", "##", "##", "  ", "  "];
const QUOTE: Glyph = ["## ##", "## ##", "## ##", "     ", "     ", "     ", "     ", "     ", "     ", "     "];
const BLANK: Glyph = ["    "; PX_ROWS];

const CY_BE: Glyph = [
    "######", "#     ", "#     ", "#     ", "##### ",
    "#    #", "#    #", "#    #", "#    #", "##### ",
];
const CY_GE: Glyph = [
    "######", "#     ", "#     ", "#     ", "#     ",
    "#     ", "#     ", "#     ", "#     ", "#     ",
];
const CY_DE: Glyph = [
    "  ####  ", "  #  #  ", "  #  #  ", "  #  #  ", "  #  #  ",
    "  #  #  ", " #    # ", " #    # ", "########", "#      #",
];
const CY_YO: Glyph = [
    " #  # ", "      ", "######", "#     ", "##### ",
    "#     ", "#     ", "#     ", "#     ", "######",
];
const CY_ZHE: Glyph = [
    "#  #  #", "#  #  #", " # # # ", " # # # ", "  ###  ",
    "  ###  ", " # # # ", " # # # ", "#  #  #", "#  #  #",
];
const CY_I: Glyph = [
    "#    #", "#    #", "#   ##", "#  # #", "#  # #",
    "# #  #", "# #  #", "##   #", "#    #", "#    #",
];
const CY_IKR: Glyph = [
    " #### ", "      ", "#    #", "#   ##", "#  # #",
    "#  # #", "# #  #", "# #  #", "##   #", "#    #",
];
const CY_EL: Glyph = [
    "  ####", "  #  #", "  #  #", "  #  #", " #   #",
    " #   #", " #   #", "#    #", "#    #", "#    #",
];
const CY_PE: Glyph = [
    "######", "#    #", "#    #", "#    #", "#    #",
    "#    #", "#    #", "#    #", "#    #", "#    #",
];
const CY_U: Glyph = [
    "#    #", "#    #", " #  # ", " #  # ", "  ##  ",
    "  #   ", "  #   ", " #    ", "##    ", "#     ",
];
const CY_EF: Glyph = [
    "   #   ", " ##### ", "#  #  #", "#  #  #", "#  #  #",
    "#  #  #", "#  #  #", " ##### ", "   #   ", "   #   ",
];
const CY_TSE: Glyph = [
    "#    # ", "#    # ", "#    # ", "#    # ", "#    # ",
    "#    # ", "#    # ", "#    # ", "#######", "      #",
];
const CY_CHE: Glyph = [
    "#    #", "#    #", "#    #", "#    #", "#    #",
    " #####", "     #", "     #", "     #", "     #",
];
const CY_SHA: Glyph = [
    "#   #   #", "#   #   #", "#   #   #", "#   #   #", "#   #   #",
    "#   #   #", "#   #   #", "#   #   #", "#   #   #", "#########",
];
const CY_SHCHA: Glyph = [
    "#   #   # ", "#   #   # ", "#   #   # ", "#   #   # ", "#   #   # ",
    "#   #   # ", "#   #   # ", "#   #   # ", "##########", "        ##",
];
const CY_HARD: Glyph = [
    "##     ", " #     ", " #     ", " #     ", " ##### ",
    " #    #", " #    #", " #    #", " #    #", " ##### ",
];
const CY_YERY: Glyph = [
    "#      #", "#      #", "#      #", "#      #", "#####  #",
    "#    # #", "#    # #", "#    # #", "#    # #", "#####  #",
];
const CY_SOFT: Glyph = [
    "#     ", "#     ", "#     ", "#     ", "##### ",
    "#    #", "#    #", "#    #", "#    #", "##### ",
];
const CY_E: Glyph = [
    " #### ", "#    #", "     #", "     #", "  ####",
    "     #", "     #", "     #", "#    #", " #### ",
];
const CY_YU: Glyph = [
    "#   #### ", "#  #    #", "#  #    #", "#  #    #", "####    #",
    "#  #    #", "#  #    #", "#  #    #", "#  #    #", "#   #### ",
];
const CY_YA: Glyph = [
    " #####", "#    #", "#    #", "#    #", " #####",
    "   # #", "  #  #", " #   #", "#    #", "#    #",
];

fn glyph(c: char) -> &'static Glyph {
    let upper = c.to_uppercase().next().unwrap_or(c);
    match upper {
        'A' | 'А' => &A,
        'B' => &B,
        'C' | 'С' => &C,
        'D' => &D,
        'E' | 'Е' => &E,
        'F' => &F,
        'G' => &G,
        'H' | 'Н' => &H,
        'I' => &I,
        'J' => &J,
        'K' | 'К' => &K,
        'L' => &L,
        'M' | 'М' => &M,
        'N' => &N,
        'O' | 'О' => &O,
        'P' | 'Р' => &P,
        'Q' => &Q,
        'R' => &R,
        'S' => &S,
        'T' | 'Т' => &T,
        'U' => &U,
        'V' => &V,
        'W' => &W,
        'X' | 'Х' => &X,
        'Y' => &Y,
        'Z' => &Z,

        'В' => &B,
        'Б' => &CY_BE,
        'Г' => &CY_GE,
        'Д' => &CY_DE,
        'Ё' => &CY_YO,
        'Ж' => &CY_ZHE,
        'З' => &D3,
        'И' => &CY_I,
        'Й' => &CY_IKR,
        'Л' => &CY_EL,
        'П' => &CY_PE,
        'У' => &CY_U,
        'Ф' => &CY_EF,
        'Ц' => &CY_TSE,
        'Ч' => &CY_CHE,
        'Ш' => &CY_SHA,
        'Щ' => &CY_SHCHA,
        'Ъ' => &CY_HARD,
        'Ы' => &CY_YERY,
        'Ь' => &CY_SOFT,
        'Э' => &CY_E,
        'Ю' => &CY_YU,
        'Я' => &CY_YA,

        '0' => &D0,
        '1' => &D1,
        '2' => &D2,
        '3' => &D3,
        '4' => &D4,
        '5' => &D5,
        '6' => &D6,
        '7' => &D7,
        '8' => &D8,
        '9' => &D9,

        '\'' | '\u{2019}' => &APOS,
        ',' => &COMMA,
        '.' => &DOT,
        '!' => &BANG,
        '?' => &QUERY,
        '-' | '\u{2013}' | '\u{2014}' => &DASH,
        ' ' => &SPACE,
        '(' | '[' => &LPAREN,
        ')' | ']' => &RPAREN,
        ':' | ';' => &COLON,
        '"' | '\u{201c}' | '\u{201d}' => &QUOTE,

        _ => &BLANK,
    }
}

/// Renders `text` as big block letters, one string per output row.
///
/// Each output row packs two pixel rows: the upper one becomes `▀`, the lower
/// `▄`, both `█`, neither a space.
pub fn render(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut rows = vec![String::new(); HEIGHT];

    for (i, &ch) in chars.iter().enumerate() {
        let g = glyph(ch);

        for (row, out) in rows.iter_mut().enumerate() {
            let top: Vec<char> = g[row * 2].chars().collect();
            let bottom: Vec<char> = g[row * 2 + 1].chars().collect();
            let width = top.len().max(bottom.len());

            for x in 0..width {
                let t = top.get(x).is_some_and(|&c| c != ' ');
                let b = bottom.get(x).is_some_and(|&c| c != ' ');
                out.push(match (t, b) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                });
            }
        }

        if i + 1 != chars.len() {
            for row in rows.iter_mut() {
                row.push(' ');
            }
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUPPORTED: &str = concat!(
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "0123456789",
        "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ",
        "',.!?- ()[]:;\"",
    );

    /// A glyph whose rows disagree on width would shear apart when the two
    /// pixel rows are packed into one terminal row.
    #[test]
    fn every_glyph_is_rectangular() {
        for c in SUPPORTED.chars() {
            let g = glyph(c);
            let width = g[0].chars().count();
            for (i, row) in g.iter().enumerate() {
                assert_eq!(
                    row.chars().count(),
                    width,
                    "glyph {c:?} row {i} is {} wide, expected {width}",
                    row.chars().count()
                );
            }
        }
    }

    #[test]
    fn render_emits_exactly_height_rows_of_equal_width() {
        let rows = render("Hello, Мир!");
        assert_eq!(rows.len(), HEIGHT);
        let width = rows[0].chars().count();
        assert!(rows.iter().all(|r| r.chars().count() == width));
    }

    #[test]
    fn unknown_characters_do_not_panic() {
        assert_eq!(render("日本語 ♫ ").len(), HEIGHT);
    }

    /// Eyeballing is the only real test for letterforms.
    /// `cargo test -- --ignored --nocapture preview`
    #[test]
    #[ignore]
    fn preview() {
        for line in ["ABCDEFGHIJKLM", "NOPQRSTUVWXYZ", "0123456789", "АБВГДЕЁЖЗИЙКЛМ", "НОПРСТУФХЦЧШЩ", "ЪЫЬЭЮЯ ?!,.-'", "I miss the love"] {
            for row in render(line) {
                println!("{row}");
            }
            println!();
        }
    }
}
