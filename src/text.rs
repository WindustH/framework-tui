//! Display-width helpers shared by the prompt buffer and the widgets.
//!
//! ratatui drops control characters when it draws a span, so every width
//! here counts them as zero columns. That keeps cursor positions, padding
//! and truncation in step with what actually reaches the terminal.

use std::borrow::Cow;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const ELLIPSIS: &str = "...";

/// Terminal columns `text` occupies once rendered.
pub(crate) fn display_width(text: &str) -> usize {
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
pub(crate) fn truncate_to_width(text: &str, width: usize) -> Cow<'_, str> {
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
pub(crate) fn skip_columns<'a>(text: &'a str, columns: &mut usize) -> (&'a str, usize) {
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
pub(crate) fn spaces(count: usize) -> Cow<'static, str> {
  const SPACES: &str = "                                                                ";
  SPACES
    .get(..count)
    .map_or_else(|| Cow::Owned(" ".repeat(count)), Cow::Borrowed)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn width_counts_wide_chars_and_ignores_control_chars() {
    assert_eq!(display_width("abc"), 3);
    assert_eq!(display_width("中文"), 4);
    assert_eq!(display_width("a\tb\r\nc"), 3);
  }

  #[test]
  fn truncation_never_exceeds_the_width() {
    // Fits exactly: unchanged (this used to come back as "abcdefghi...").
    assert_eq!(truncate_to_width("abcdefghij", 10), "abcdefghij");
    assert_eq!(truncate_to_width("abcdefghijk", 10), "abcdefg...");
    assert_eq!(truncate_to_width("abcdef", 3), "abc");
    assert_eq!(truncate_to_width("abcdef", 0), "");
    for width in 0..12 {
      for text in ["abcdefghijk", "中文字符测试", "a中b文c"] {
        assert!(display_width(&truncate_to_width(text, width)) <= width);
      }
    }
    // A wide char that would straddle the limit is left out.
    assert_eq!(truncate_to_width("中文字符", 6), "中...");
    assert_eq!(truncate_to_width("中文字符", 3), "中");
  }

  #[test]
  fn skip_columns_handles_wide_chars_and_carries_over() {
    let mut columns = 2;
    assert_eq!(skip_columns("abcd", &mut columns), ("cd", 0));
    assert_eq!(columns, 0);

    let mut columns = 1;
    assert_eq!(skip_columns("中文", &mut columns), ("文", 1));

    let mut columns = 5;
    assert_eq!(skip_columns("abc", &mut columns), ("", 0));
    assert_eq!(columns, 2);
    assert_eq!(skip_columns("defg", &mut columns), ("fg", 0));

    let mut columns = 1;
    assert_eq!(skip_columns("e\u{301}x", &mut columns), ("x", 0));
  }

  #[test]
  fn spaces_borrow_short_runs() {
    assert!(matches!(spaces(4), Cow::Borrowed("    ")));
    assert_eq!(spaces(100).len(), 100);
  }
}
