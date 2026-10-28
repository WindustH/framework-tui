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
