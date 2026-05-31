//! Single-block text editor buffer.
//!
//! Holds a `Vec<char>` plus a cursor index; exposes the small set of
//! cursor + edit operations the TUI needs. Block text in outl is
//! single-line in phase 1 — multi-line support lives behind the same
//! API for when block content grows (Yrs already supports it).

/// Edit buffer with cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditBuffer {
    /// Character contents.
    pub chars: Vec<char>,
    /// Cursor position in chars (0..=chars.len()).
    pub cursor: usize,
}

impl EditBuffer {
    /// Build from a string with the cursor placed at the end.
    pub fn from_text(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    /// Empty buffer with cursor at 0.
    pub fn empty() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
        }
    }

    /// Render as `String`.
    pub fn as_string(&self) -> String {
        self.chars.iter().collect()
    }

    /// Length in characters.
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Insert one character at the cursor.
    pub fn insert_char(&mut self, ch: char) {
        self.chars.insert(self.cursor, ch);
        self.cursor += 1;
    }

    /// Insert a string at the cursor.
    pub fn insert_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.insert_char(ch);
        }
    }

    /// Insert a pair like `(`/`)`, leaving the cursor between them.
    pub fn insert_pair(&mut self, open: char, close: char) {
        self.chars.insert(self.cursor, open);
        self.chars.insert(self.cursor + 1, close);
        self.cursor += 1;
    }

    /// Backspace inside an empty doubled pair like `[[|]]` or `((|))`
    /// — delete both opener and closer in one shot so the user
    /// doesn't have to backspace four times to undo an aborted ref.
    /// Returns `true` when the pair was collapsed, `false` when no
    /// such pair surrounds the cursor (caller should fall back to
    /// the normal one-char backspace).
    pub fn delete_pair_back(&mut self) -> bool {
        if self.cursor < 2 || self.cursor + 2 > self.chars.len() {
            return false;
        }
        let left = (self.chars[self.cursor - 2], self.chars[self.cursor - 1]);
        let right = (self.chars[self.cursor], self.chars[self.cursor + 1]);
        let is_brackets = left == ('[', '[') && right == (']', ']');
        let is_parens = left == ('(', '(') && right == (')', ')');
        if !is_brackets && !is_parens {
            return false;
        }
        // Remove [opener, opener, closer, closer] around the cursor.
        for _ in 0..2 {
            self.chars.remove(self.cursor); // both closers (shift left)
        }
        for _ in 0..2 {
            self.cursor -= 1;
            self.chars.remove(self.cursor); // both openers
        }
        true
    }

    /// Delete the character before the cursor (Backspace).
    /// Returns `true` if a character was removed.
    pub fn delete_back(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
            true
        } else {
            false
        }
    }

    /// Delete the character at the cursor (Delete).
    pub fn delete_forward(&mut self) -> bool {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
            true
        } else {
            false
        }
    }

    /// Move cursor left one character.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right one character.
    pub fn move_right(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor to the start of the line.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) {
        self.cursor = self.chars.len();
    }

    /// Move cursor to the previous word boundary.
    pub fn move_word_left(&mut self) {
        while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
    }

    /// Move cursor to the next word boundary.
    pub fn move_word_right(&mut self) {
        let len = self.chars.len();
        while self.cursor < len && !self.chars[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
        while self.cursor < len && self.chars[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
    }

    /// Delete from the cursor to the start of the current word.
    pub fn delete_word_back(&mut self) {
        let start = self.cursor;
        self.move_word_left();
        self.chars.drain(self.cursor..start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_text_places_cursor_at_end() {
        let b = EditBuffer::from_text("hello");
        assert_eq!(b.cursor, 5);
        assert_eq!(b.as_string(), "hello");
    }

    #[test]
    fn insert_char_advances_cursor() {
        let mut b = EditBuffer::from_text("hllo");
        b.cursor = 1;
        b.insert_char('e');
        assert_eq!(b.as_string(), "hello");
        assert_eq!(b.cursor, 2);
    }

    #[test]
    fn insert_pair_leaves_cursor_between() {
        let mut b = EditBuffer::empty();
        b.insert_pair('[', ']');
        assert_eq!(b.as_string(), "[]");
        assert_eq!(b.cursor, 1);
    }

    #[test]
    fn delete_back_at_zero_is_noop() {
        let mut b = EditBuffer::from_text("abc");
        b.cursor = 0;
        assert!(!b.delete_back());
        assert_eq!(b.as_string(), "abc");
    }

    #[test]
    fn delete_back_removes_prev_char() {
        let mut b = EditBuffer::from_text("abc");
        assert!(b.delete_back());
        assert_eq!(b.as_string(), "ab");
        assert_eq!(b.cursor, 2);
    }

    #[test]
    fn move_word_boundaries() {
        let mut b = EditBuffer::from_text("hello world today");
        b.move_word_left();
        assert_eq!(b.cursor, 12); // start of "today"
        b.move_word_left();
        assert_eq!(b.cursor, 6); // start of "world"
        b.move_word_left();
        assert_eq!(b.cursor, 0); // start of "hello"
    }

    #[test]
    fn delete_word_back_removes_word() {
        let mut b = EditBuffer::from_text("hello world");
        b.delete_word_back();
        assert_eq!(b.as_string(), "hello ");
    }

    #[test]
    fn insert_str_handles_multi_char() {
        let mut b = EditBuffer::empty();
        b.insert_str("[[ref]]");
        assert_eq!(b.as_string(), "[[ref]]");
        assert_eq!(b.cursor, 7);
    }

    #[test]
    fn delete_pair_back_collapses_empty_brackets() {
        let mut b = EditBuffer::from_text("foo [[]]");
        b.cursor = 6; // between [[ and ]]
        assert!(b.delete_pair_back());
        assert_eq!(b.as_string(), "foo ");
        assert_eq!(b.cursor, 4);
    }

    #[test]
    fn delete_pair_back_collapses_empty_parens() {
        let mut b = EditBuffer::from_text("see (())");
        b.cursor = 6; // between (( and ))
        assert!(b.delete_pair_back());
        assert_eq!(b.as_string(), "see ");
        assert_eq!(b.cursor, 4);
    }

    #[test]
    fn delete_pair_back_skips_when_pair_has_content() {
        let mut b = EditBuffer::from_text("[[ave]]");
        b.cursor = 5; // between "ave" and ]]
        assert!(!b.delete_pair_back());
        assert_eq!(b.as_string(), "[[ave]]");
        assert_eq!(b.cursor, 5);
    }

    #[test]
    fn delete_pair_back_skips_at_buffer_edges() {
        let mut b = EditBuffer::from_text("[[]]");
        b.cursor = 0;
        assert!(!b.delete_pair_back());
        b.cursor = 4;
        assert!(!b.delete_pair_back());
    }

    #[test]
    fn delete_pair_back_skips_cross_mixed_pairs() {
        let mut b = EditBuffer::from_text("[[))");
        b.cursor = 2;
        assert!(!b.delete_pair_back());
        assert_eq!(b.as_string(), "[[))");
    }
}
