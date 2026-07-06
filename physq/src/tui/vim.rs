//! Pure Vim-emulation types and text motions for the TUI's `--vim` keymap.
//! Kept free of `App`/`ratatui` so the motion math is unit-testable; all key
//! dispatch and side effects stay in `tui::mod`.

/// The modal state while Vim keybindings are active. `Normal` is deliberately
/// invisible in the status bar (Vim itself shows nothing for it); every other
/// mode is displayed as a fixed `-- MODE --` indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    Insert,
    Normal,
    Visual,
}

/// Cursor placement when entering INSERT mode (`i`/`a`/`I`/`A`, plus the
/// change operator, `/` and `:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertAt {
    /// `i` — keep the cursor where it is.
    Here,
    /// `a` — one to the right.
    After,
    /// `I` — column 0.
    Start,
    /// `A` — end of the line.
    End,
}

/// `w`: start of the next whitespace-separated word (Vim's `W` semantics —
/// simple word classes are enough for a one-line search query; CJK text with
/// no spaces counts as one word).
pub fn next_word_start(chars: &[char], pos: usize) -> usize {
    let n = chars.len();
    let mut i = pos;
    while i < n && !chars[i].is_whitespace() {
        i += 1;
    }
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// `b`: start of the previous whitespace-separated word (Vim's `B`).
pub fn prev_word_start(chars: &[char], pos: usize) -> usize {
    let mut i = pos.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn next_word_start_skips_word_then_spaces() {
        let c = chars("foo  bar baz");
        assert_eq!(next_word_start(&c, 0), 5); // foo → bar
        assert_eq!(next_word_start(&c, 5), 9); // bar → baz
        assert_eq!(next_word_start(&c, 9), 12); // baz → end
        assert_eq!(next_word_start(&c, 12), 12); // already at end
    }

    #[test]
    fn next_word_start_from_whitespace_lands_on_next_word() {
        let c = chars("foo  bar");
        assert_eq!(next_word_start(&c, 3), 5);
    }

    #[test]
    fn prev_word_start_skips_spaces_then_word() {
        let c = chars("foo  bar baz");
        assert_eq!(prev_word_start(&c, 12), 9); // end → baz
        assert_eq!(prev_word_start(&c, 9), 5); // baz → bar
        assert_eq!(prev_word_start(&c, 5), 0); // bar → foo
        assert_eq!(prev_word_start(&c, 0), 0); // already at start
    }

    #[test]
    fn prev_word_start_from_mid_word_goes_to_its_start() {
        let c = chars("foo bar");
        assert_eq!(prev_word_start(&c, 6), 4);
    }

    #[test]
    fn word_motions_treat_spaceless_cjk_as_one_word() {
        let c = chars("電磁誘導");
        assert_eq!(next_word_start(&c, 0), 4);
        assert_eq!(prev_word_start(&c, 4), 0);
    }

    #[test]
    fn word_motions_handle_empty_input() {
        let c = chars("");
        assert_eq!(next_word_start(&c, 0), 0);
        assert_eq!(prev_word_start(&c, 0), 0);
    }
}
