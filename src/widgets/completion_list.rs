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
