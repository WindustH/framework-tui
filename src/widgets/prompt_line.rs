//! The one-line prompt: prefix, input, inline completion suggestion and the
//! terminal cursor.

use ratatui::{
  Frame,
  layout::Rect,
  style::Style,
  text::{Line, Span},
  widgets::Paragraph,
};

use crate::command::{CommandCompletion, Prompt};
use crate::text::{display_width, skip_columns, spaces};

#[derive(Debug, Clone)]
pub struct PromptLineStyle {
  pub base: Style,
  pub prefix: Style,
  pub suggestion: Style,
}

/// Draw `prompt` on the first row of `area` and place the terminal cursor.
///
/// For a command prompt with the cursor at the end, the rest of the
/// selected completion is shown after the input in the `suggestion` style.
/// When the text before the cursor is wider than `area`, the line scrolls
/// left so the cursor stays on screen. Returns the cursor position, or
/// `None` for an empty `area`.
pub fn draw_prompt_line(
  frame: &mut Frame,
  prompt: &Prompt,
  completion: Option<&CommandCompletion>,
  area: Rect,
  style: &PromptLineStyle,
) -> Option<(u16, u16)> {
  if area.is_empty() {
    return None;
  }

  let buffer = prompt.buffer();
  let prefix = prompt.prefix();
  let suggestion = match completion {
    Some(completion) if prompt.is_command() && buffer.cursor == buffer.input.len() => {
      completion.suggestion()
    }
    _ => "",
  };

  let cursor_column = display_width(prefix) + buffer.cursor_columns();
  let last_column = usize::from(area.width - 1);
  let scroll = cursor_column.saturating_sub(last_column);

  let mut skip = scroll;
  let mut spans = Vec::with_capacity(4);
  for (text, text_style) in [
    (prefix, style.prefix),
    (buffer.input.as_str(), style.base),
    (suggestion, style.suggestion),
  ] {
    let (visible, blank) = skip_columns(text, &mut skip);
    if blank > 0 {
      spans.push(Span::styled(spaces(blank), text_style));
    }
    if !visible.is_empty() {
      spans.push(Span::styled(visible, text_style));
    }
  }
  frame.render_widget(Paragraph::new(Line::from(spans)).style(style.base), area);

  // `cursor_column - scroll` is at most `last_column`, which fits in u16.
  let cursor_x = u16::try_from(cursor_column - scroll).unwrap_or(area.width - 1);
  let position = (area.x.saturating_add(cursor_x), area.y);
  frame.set_cursor_position(position);
  Some(position)
}
