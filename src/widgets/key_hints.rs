//! Which-key hints: the possible next keys of a pending key sequence, laid
//! out in columns.

use ratatui::{
  Frame,
  layout::Rect,
  style::Style,
  text::{Line, Span, Text},
  widgets::{Block, Paragraph},
};

use crate::keymap::KeyHint;
use crate::text::{display_width, spaces, truncate_to_width};

/// Narrowest cell [`key_hint_columns`] will lay out.
const MIN_CELL_WIDTH: usize = 24;

#[derive(Debug, Clone)]
pub struct KeyHintsStyle {
  pub base: Style,
  pub key: Style,
  pub separator: Style,
  pub description: Style,
  /// Drawn between a key and its description.
  pub separator_text: String,
  /// Preferred number of columns; fewer are used on narrow areas.
  pub columns: usize,
}

/// Columns for `width`: the `configured` count, reduced so that each cell
/// is at least 24 columns wide, and at least 1.
pub fn key_hint_columns(configured: usize, width: u16) -> usize {
  let max_by_width = (usize::from(width) / MIN_CELL_WIDTH).max(1);
  configured.clamp(1, max_by_width)
}

/// Rows needed for `count` hints in `columns` columns (at least 1).
pub fn key_hint_rows(count: usize, columns: usize) -> u16 {
  u16::try_from(count.div_ceil(columns.max(1)).max(1)).unwrap_or(u16::MAX)
}

/// Draw `hints` row by row in equal-width cells: key, separator,
/// description. Hints that do not fit in `area` are left out.
pub fn draw_key_hints(frame: &mut Frame, hints: &[KeyHint], area: Rect, style: &KeyHintsStyle) {
  if hints.is_empty() || area.is_empty() {
    return;
  }

  frame.render_widget(Block::default().style(style.base), area);

  let columns = key_hint_columns(style.columns, area.width);
  let rows = usize::from(key_hint_rows(hints.len(), columns).min(area.height));
  let cell_width = (usize::from(area.width) / columns).max(1);
  let lines = hints
    .chunks(columns)
    .take(rows)
    .map(|row| {
      let mut spans = Vec::with_capacity(columns * 4);
      for hint in row {
        push_key_hint_cell(&mut spans, hint, cell_width, style);
      }
      // Blank the empty cells of a short last row, except the last column.
      for _ in row.len()..columns.saturating_sub(1) {
        spans.push(Span::styled(spaces(cell_width), style.base));
      }
      Line::from(spans)
    })
    .collect::<Vec<_>>();

  frame.render_widget(Paragraph::new(Text::from(lines)).style(style.base), area);
}

fn push_key_hint_cell<'a>(
  spans: &mut Vec<Span<'a>>,
  hint: &'a KeyHint,
  cell_width: usize,
  style: &'a KeyHintsStyle,
) {
  let parts = [
    (hint.key.as_str(), style.key),
    (style.separator_text.as_str(), style.separator),
    (hint.label.as_str(), style.description),
  ];
  let mut used = 0;
  for (text, part_style) in parts {
    if used >= cell_width {
      break;
    }
    let text = truncate_to_width(text, cell_width - used);
    used += display_width(&text);
    spans.push(Span::styled(text, part_style));
  }
  if used < cell_width {
    spans.push(Span::styled(spaces(cell_width - used), style.base));
  }
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  fn hint(key: &str, label: &str) -> KeyHint {
    KeyHint {
      key: key.to_string(),
      label: label.to_string(),
    }
  }

  #[test]
  fn layout_helpers() {
    assert_eq!(key_hint_columns(3, 80), 3);
    assert_eq!(key_hint_columns(3, 50), 2);
    assert_eq!(key_hint_columns(0, 10), 1);
    assert_eq!(key_hint_rows(0, 3), 1);
    assert_eq!(key_hint_rows(7, 3), 3);
    assert_eq!(key_hint_rows(7, 0), 7);
  }

  /// A cell's text used to overflow by two columns when cut, pushing the
  /// following columns out of line.
  #[test]
  fn cells_keep_their_columns() {
    let style = KeyHintsStyle {
      base: Style::default(),
      key: Style::default(),
      separator: Style::default(),
      description: Style::default(),
      separator_text: " -> ".to_string(),
      columns: 2,
    };
    let hints = [
      hint("g", "a description much longer than one cell"),
      hint("h", "second"),
      hint("日本", "wide"),
    ];
    let mut terminal = Terminal::new(TestBackend::new(48, 2)).unwrap();
    terminal
      .draw(|frame| draw_key_hints(frame, &hints, frame.area(), &style))
      .unwrap();
    let buffer = terminal.backend().buffer();
    let row = |y: u16| {
      let mut text = String::new();
      let mut x = 0;
      while x < 48 {
        let symbol = buffer[(x, y)].symbol();
        text.push_str(symbol);
        x += u16::try_from(display_width(symbol).max(1)).unwrap();
      }
      text
    };
    assert_eq!(
      row(0),
      format!("{:<48}", "g -> a description mu...h -> second")
    );
    assert_eq!(row(1), format!("日本 -> wide{}", " ".repeat(36)));
  }
}
