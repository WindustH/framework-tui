//! Display-width helpers shared by the prompt buffer and the widgets.
//!
//! ratatui drops control characters when it draws a span, so every width
//! here counts them as zero columns. That keeps cursor positions, padding
//! and truncation in step with what actually reaches the terminal.

use std::borrow::Cow;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const ELLIPSIS: &str = "...";

/// Terminal columns `text` occupies once rendered.
pub fn display_width(text: &str) -> usize {
  text
    .split(char::is_control)
    .map(UnicodeWidthStr::width)
    .sum()
}

fn char_width(ch: char) -> usize {
  if ch.is_control() {
    0
  } else {
    ch.width().unwrap_or(0)
  }
}

/// Fit `text` into `width` columns. Text that is too wide is cut and ends in
/// `...`, unless `width` is too narrow to hold the ellipsis, in which case
/// it is simply cut.
pub fn truncate_to_width(text: &str, width: usize) -> Cow<'_, str> {
  if display_width(text) <= width {
    return Cow::Borrowed(text);
  }
  let (budget, ellipsis) = if width > ELLIPSIS.len() {
    (width - ELLIPSIS.len(), ELLIPSIS)
  } else {
    (width, "")
  };
  let mut end = text.len();
  let mut used = 0;
  for (index, ch) in text.char_indices() {
    used += char_width(ch);
    if used > budget {
      end = index;
      break;
    }
  }
  Cow::Owned(format!("{}{ellipsis}", &text[..end]))
}

/// Drop the first `*columns` display columns of `text`.
///
/// Returns the remaining text and how many blank columns stand in for a
/// wide character that the cut split in half. `*columns` is reduced by the
/// columns `text` consumed, so the call can be chained across spans.
pub fn skip_columns<'a>(text: &'a str, columns: &mut usize) -> (&'a str, usize) {
  if *columns == 0 {
    return (text, 0);
  }
  for (index, ch) in text.char_indices() {
    let width = char_width(ch);
    if *columns == 0 {
      if width == 0 {
        // Combining marks belong to the character that was skipped.
        continue;
      }
      return (&text[index..], 0);
    }
    if width > *columns {
      let blank = width - *columns;
      *columns = 0;
      return (&text[index + ch.len_utf8()..], blank);
    }
    *columns -= width;
  }
  ("", 0)
}

/// `count` spaces, borrowed for the common short paddings.
pub fn spaces(count: usize) -> Cow<'static, str> {
  const SPACES: &str = "                                                                ";
  SPACES
    .get(..count)
    .map_or_else(|| Cow::Owned(" ".repeat(count)), Cow::Borrowed)
}
