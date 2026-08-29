// Option C: a quiet one-line orbit wordmark, without a trailing rail or glint.
// Every glyph is single-cell so it stays centered in any terminal font.
pub const TITLE: [&str; 1] = ["☾  I N S O M N I A"];
pub const TITLE_W: u16 = 18;
pub const TITLE_H: u16 = 1;
pub const WORDMARK_X: u16 = 0;
pub const WORDMARK_W: u16 = TITLE_W;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_rows_match_declared_dimensions() {
        assert_eq!(TITLE.len(), TITLE_H as usize);
        for line in TITLE {
            assert_eq!(line.chars().count(), TITLE_W as usize, "{line}");
        }
    }
}
