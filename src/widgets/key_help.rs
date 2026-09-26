//! The key-help dialog: a popup listing key bindings.

use ratatui::{
  Frame,
  layout::Rect,
  style::{Modifier, Style},
  text::{Line, Span, Text},
};

use super::popup::{PopupDialogStyle, render_popup};
use crate::keymap::KeyHelpEntry;
use crate::text::{display_width, spaces, truncate_to_width};

#[derive(Debug, Clone)]
pub struct KeyHelpDialogStyle {
  pub popup: PopupDialogStyle,
  pub key: Style,
  pub description: Style,
  pub muted: Style,
  /// Last line of the listing; empty for none.
  pub close_hint: String,
  /// Widest the key column may grow; longer key lists are cut with `...`.
  pub max_key_width: usize,
}

impl Default for KeyHelpDialogStyle {
  fn default() -> Self {
    Self {
      popup: PopupDialogStyle {
        min_height: 8,
        max_height: 34,
        ..PopupDialogStyle::default()
      },
      key: Style::default().add_modifier(Modifier::BOLD),
      description: Style::default(),
      muted: Style::default().add_modifier(Modifier::DIM),
      close_hint: "esc / q / enter / f1 close".to_string(),
      max_key_width: 24,
    }
  }
}

/// Draw a popup listing `entries` as aligned key and description columns.
/// Returns the popup rectangle, or `None` when `area` is too small.
pub fn draw_key_help_dialog(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  entries: &[KeyHelpEntry],
  style: &KeyHelpDialogStyle,
) -> Option<Rect> {
  draw_key_help_dialog_scrolled(frame, area, title, entries, style, 0)
}

/// Like [`draw_key_help_dialog`] but scrolled down by `scroll` rows. The
/// `close_hint` is the last row of the scrolled content, so the listing
/// spans `entries.len() + 1` rows (more if long descriptions wrap).
pub fn draw_key_help_dialog_scrolled(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  entries: &[KeyHelpEntry],
  style: &KeyHelpDialogStyle,
  scroll: usize,
) -> Option<Rect> {
  let key_width = entries
    .iter()
    .map(|entry| display_width(&entry.keys))
    .max()
    .unwrap_or(0)
    .min(style.max_key_width);
  let mut lines = Vec::with_capacity(entries.len() + 1);
  if entries.is_empty() {
    lines.push(Line::from(Span::styled(
      "No bindings available",
      style.muted,
    )));
  }
  for entry in entries {
    let keys = truncate_to_width(&entry.keys, key_width.max(1));
    let padding = key_width.saturating_sub(display_width(&keys)) + 2;
    lines.push(Line::from(vec![
      Span::styled(keys, style.key),
      Span::styled(spaces(padding), style.popup.base),
      Span::styled(entry.description.as_str(), style.description),
    ]));
  }
  if !style.close_hint.is_empty() {
    lines.push(Line::from(Span::styled(
      style.close_hint.as_str(),
      style.muted,
    )));
  }
  render_popup(frame, area, title, Text::from(lines), &style.popup, scroll)
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  fn entry(keys: &str, description: &str) -> KeyHelpEntry {
    KeyHelpEntry {
      action: String::new(),
      keys: keys.to_string(),
      description: description.to_string(),
    }
  }

  #[test]
  fn long_keys_are_cut_and_descriptions_stay_aligned() {
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    let entries = [entry("q", "Quit"), entry(&"x ".repeat(20), "Long")];
    let style = KeyHelpDialogStyle {
      max_key_width: 10,
      ..KeyHelpDialogStyle::default()
    };
    terminal
      .draw(|frame| {
        draw_key_help_dialog(frame, frame.area(), "Keys", &entries, &style);
      })
      .unwrap();
    let buffer = terminal.backend().buffer();
    let row = |y: u16| {
      (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
    };
    let quit = row(2);
    let long = row(3);
    assert_eq!(quit.find("Quit"), long.find("Long"), "{quit:?}\n{long:?}");
    assert!(long.contains("x x x x...  Long"), "{long:?}");
  }
}
