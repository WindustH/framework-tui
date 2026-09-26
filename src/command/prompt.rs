//! Single-line prompt input: the editable buffer and the prompt kinds.

use crate::text::display_width;

/// An open prompt: either a text prompt with its own label (rename, search,
/// ...) or the `:` command line.
#[derive(Debug, Clone)]
pub enum Prompt {
  Text {
    prefix: String,
    buffer: PromptBuffer,
  },
  Command {
    buffer: PromptBuffer,
  },
}

/// Single-line input with a cursor.
///
/// `cursor` is a byte offset into `input`. Both fields are public; every
/// method tolerates a cursor that is past the end or inside a multi-byte
/// character by snapping it back to the previous character boundary.
#[derive(Debug, Clone)]
pub struct PromptBuffer {
  pub input: String,
  pub cursor: usize,
}

impl PromptBuffer {
  /// A buffer holding `input` with the cursor at the end.
  pub fn new(input: impl Into<String>) -> Self {
    let input = input.into();
    let cursor = input.len();
    Self { input, cursor }
  }

  /// Replace the text and move the cursor to the end.
  pub fn set_input(&mut self, input: String) {
    self.input = input;
    self.cursor = self.input.len();
  }

  pub fn insert_char(&mut self, ch: char) {
    let cursor = self.valid_cursor();
    self.input.insert(cursor, ch);
    self.cursor = cursor + ch.len_utf8();
  }

  /// Insert pasted text. Line breaks and tabs become spaces and other
  /// control characters are dropped, keeping the input on one line.
  pub fn insert_str(&mut self, value: &str) {
    let value = sanitize_inline_input(value);
    if value.is_empty() {
      return;
    }
    let cursor = self.valid_cursor();
    self.input.insert_str(cursor, &value);
    self.cursor = cursor + value.len();
  }

  pub fn backspace(&mut self) {
    let cursor = self.valid_cursor();
    let previous = previous_boundary(&self.input, cursor);
    self.input.drain(previous..cursor);
    self.cursor = previous;
  }

  pub fn delete(&mut self) {
    let cursor = self.valid_cursor();
    let next = next_boundary(&self.input, cursor);
    self.input.drain(cursor..next);
    self.cursor = cursor;
  }

  pub fn move_left(&mut self) {
    self.cursor = previous_boundary(&self.input, self.valid_cursor());
  }

  pub fn move_right(&mut self) {
    self.cursor = next_boundary(&self.input, self.valid_cursor());
  }

  pub fn move_start(&mut self) {
    self.cursor = 0;
  }

  pub fn move_end(&mut self) {
    self.cursor = self.input.len();
  }

  pub fn kill_before_cursor(&mut self) {
    let cursor = self.valid_cursor();
    self.input.drain(..cursor);
    self.cursor = 0;
  }

  pub fn kill_after_cursor(&mut self) {
    let cursor = self.valid_cursor();
    self.input.truncate(cursor);
    self.cursor = cursor;
  }

  /// Terminal columns between the start of the input and the cursor.
  pub fn cursor_columns(&self) -> usize {
    display_width(&self.input[..self.valid_cursor()])
  }

  /// The cursor clamped to the input and snapped back to a char boundary.
  fn valid_cursor(&self) -> usize {
    let mut cursor = self.cursor.min(self.input.len());
    while !self.input.is_char_boundary(cursor) {
      cursor -= 1;
    }
    cursor
  }
}

impl Prompt {
  pub fn text(prefix: impl Into<String>, input: impl Into<String>) -> Self {
    Self::Text {
      prefix: prefix.into(),
      buffer: PromptBuffer::new(input),
    }
  }

  pub fn command(input: impl Into<String>) -> Self {
    Self::Command {
      buffer: PromptBuffer::new(input),
    }
  }

  pub fn buffer(&self) -> &PromptBuffer {
    match self {
      Prompt::Text { buffer, .. } | Prompt::Command { buffer } => buffer,
    }
  }

  pub fn buffer_mut(&mut self) -> &mut PromptBuffer {
    match self {
      Prompt::Text { buffer, .. } | Prompt::Command { buffer } => buffer,
    }
  }

  /// Label drawn before the input: the text prompt's prefix, or `:`.
  pub fn prefix(&self) -> &str {
    match self {
      Prompt::Text { prefix, .. } => prefix,
      Prompt::Command { .. } => ":",
    }
  }

  pub fn is_command(&self) -> bool {
    matches!(self, Prompt::Command { .. })
  }
}

/// Start of the character before `cursor` (a valid boundary).
fn previous_boundary(input: &str, cursor: usize) -> usize {
  input[..cursor]
    .char_indices()
    .next_back()
    .map_or(0, |(index, _)| index)
}

/// End of the character after `cursor` (a valid boundary).
fn next_boundary(input: &str, cursor: usize) -> usize {
  input[cursor..]
    .chars()
    .next()
    .map_or(cursor, |ch| cursor + ch.len_utf8())
}

fn sanitize_inline_input(value: &str) -> String {
  value
    .replace("\r\n", " ")
    .chars()
    .filter_map(|ch| match ch {
      '\r' | '\n' | '\t' => Some(' '),
      ch if ch.is_control() => None,
      ch => Some(ch),
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn edits_multibyte_input_by_character() {
    let mut buffer = PromptBuffer::new("日本語");
    buffer.backspace();
    assert_eq!(buffer.input, "日本");
    buffer.move_left();
    buffer.insert_char('é');
    assert_eq!(buffer.input, "日é本");
    assert_eq!(buffer.cursor_columns(), 3);
    buffer.delete();
    assert_eq!(buffer.input, "日é");
    buffer.move_start();
    buffer.move_left();
    buffer.delete();
    assert_eq!(buffer.input, "é");
    buffer.move_end();
    buffer.move_right();
    buffer.delete();
    assert_eq!((buffer.input.as_str(), buffer.cursor), ("é", 2));
  }

  /// Consumers own the public fields; a stale or char-index cursor must not
  /// panic (it used to, in `cursor_columns` on every frame).
  #[test]
  fn invalid_cursor_is_snapped_to_a_boundary() {
    let mut buffer = PromptBuffer::new("日本");
    buffer.cursor = 1;
    assert_eq!(buffer.cursor_columns(), 0);
    buffer.insert_char('x');
    assert_eq!(buffer.input, "x日本");

    let mut buffer = PromptBuffer::new("日本");
    buffer.cursor = 4;
    buffer.backspace();
    assert_eq!(buffer.input, "本");

    let mut buffer = PromptBuffer::new("abc");
    buffer.input.clear();
    assert_eq!(buffer.cursor_columns(), 0);
    buffer.backspace();
    buffer.delete();
    buffer.kill_before_cursor();
    buffer.kill_after_cursor();
    buffer.insert_str("ok");
    assert_eq!((buffer.input.as_str(), buffer.cursor), ("ok", 2));
  }

  #[test]
  fn paste_stays_on_one_line_and_cursor_matches_display() {
    let mut buffer = PromptBuffer::new("");
    buffer.insert_str("a\r\nb\tc\u{1b}d\n");
    assert_eq!(buffer.input, "a b cd ");
    assert_eq!(buffer.cursor, buffer.input.len());

    // Control chars that reach the buffer another way are not drawn, so the
    // cursor must not count them.
    let buffer = PromptBuffer::new("a\tb");
    assert_eq!(buffer.cursor_columns(), 2);
  }
}
