//! The wordmark: one quiet line, every glyph single-cell so it stays
//! centered in any terminal font.

pub const TITLE: &str = "☾  I N S O M N I A";
pub const TITLE_W: u16 = 18;
pub const TITLE_H: u16 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_matches_its_declared_width() {
        assert_eq!(TITLE.chars().count(), TITLE_W as usize);
    }
}
