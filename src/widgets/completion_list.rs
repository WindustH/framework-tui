//! The completion candidate list shown above the command prompt.

use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span, Text},
  widgets::{Block, Paragraph},
};

use super::overlay_background;
use crate::command::CommandCompletion;
use crate::text::{display_width, spaces, truncate_to_width};

#[derive(Debug, Clone)]
pub struct CompletionListStyle {
  pub base: Style,
  pub selected: Style,
}

impl Default for CompletionListStyle {
  fn default() -> Self {
    completion_list_style(Color::White)
  }
}

/// List style with `foreground` text on the overlay background.
pub fn completion_list_style(foreground: Color) -> CompletionListStyle {
  CompletionListStyle {
    base: Style::default().fg(foreground).bg(overlay_background()),
    selected: default_completion_selected_style(),
  }
}

pub fn default_completion_selected_style() -> Style {
  Style::default()
    .fg(Color::Black)
    .bg(Color::White)
    .add_modifier(Modifier::BOLD)
}

/// Rows the list needs for `completion`, at most `max_rows`; 0 without
/// candidates.
pub fn completion_rows(completion: Option<&CommandCompletion>, max_rows: usize) -> u16 {
  completion.map_or(0, |completion| {
    u16::try_from(completion.candidates.len().min(max_rows)).unwrap_or(u16::MAX)
  })
}

/// Draw the candidates one per row, marking the selected one. The list
/// scrolls to keep the selection visible.
pub fn draw_completion_list(
  frame: &mut Frame,
  completion: &CommandCompletion,
  area: Rect,
  style: &CompletionListStyle,
) {
  if completion.candidates.is_empty() || area.is_empty() {
    return;
  }

  frame.render_widget(Block::default().style(style.base), area);

  let width = usize::from(area.width);
  let visible = usize::from(area.height);
  let selected = completion.selected.min(completion.candidates.len() - 1);
  let start = selected.saturating_sub(visible - 1);
  let lines = (start..start + visible)
    .map(|index| {
      let Some(candidate) = completion.candidates.get(index) else {
        return Line::from(Span::styled(spaces(width), style.base));
      };
      let (marker, row_style) = if index == selected {
        ("> ", style.selected)
      } else {
        ("  ", style.base)
      };
      let mut text = truncate_to_width(&format!("{marker}{candidate}"), width).into_owned();
      text.push_str(&spaces(width.saturating_sub(display_width(&text))));
      Line::from(Span::styled(text, row_style))
    })
    .collect::<Vec<_>>();

  frame.render_widget(Paragraph::new(Text::from(lines)).style(style.base), area);
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  #[test]
  fn completion_style_uses_shared_reset_background() {
    let style = completion_list_style(Color::Cyan);
    assert_eq!(style.base.fg, Some(Color::Cyan));
    assert_eq!(style.base.bg, Some(Color::Reset));
    assert_eq!(style.selected, default_completion_selected_style());
  }

  /// Rows used to be padded and cut by char count: combining marks left
  /// the selected row's highlight short, and wide or long candidates
  /// overflowed so the ellipsis was clipped away.
  #[test]
  fn rows_fill_the_width_exactly() {
    let completion = CommandCompletion::new(
      0,
      0,
      "",
      vec!["cafe\u{301}".to_string(), "日本語日本語日本語".to_string()],
      false,
      0,
    );
    let style = CompletionListStyle::default();
    for selected in 0..2 {
      let mut completion = completion.clone();
      completion.selected = selected;
      let mut terminal = Terminal::new(TestBackend::new(12, 2)).unwrap();
      terminal
        .draw(|frame| draw_completion_list(frame, &completion, frame.area(), &style))
        .unwrap();
      let buffer = terminal.backend().buffer();
      let y = u16::try_from(selected).unwrap();
      // Visible cells, skipping those covered by a wide character.
      let mut cells = Vec::new();
      let mut x = 0;
      while x < 12 {
        let cell = &buffer[(x, y)];
        cells.push(cell);
        x += u16::try_from(display_width(cell.symbol()).max(1)).unwrap();
      }
      assert!(
        cells.iter().all(|cell| cell.bg == Color::White),
        "row {selected}"
      );
      let row = cells.iter().map(|cell| cell.symbol()).collect::<String>();
      let expected = ["> cafe\u{301}      ", "> 日本語... "][selected];
      assert_eq!(row, expected);
    }
    assert_eq!(completion_rows(Some(&completion), 1), 1);
    assert_eq!(completion_rows(None, 5), 0);
  }
}
