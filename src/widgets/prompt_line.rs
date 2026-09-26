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

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  fn style() -> PromptLineStyle {
    PromptLineStyle {
      base: Style::default(),
      prefix: Style::default(),
      suggestion: Style::default(),
    }
  }

  /// Renders the prompt in a `width`x1 terminal; returns the visible text
  /// and the cursor column.
  fn render(prompt: &Prompt, completion: Option<&CommandCompletion>, width: u16) -> (String, u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    let mut cursor = None;
    terminal
      .draw(|frame| cursor = draw_prompt_line(frame, prompt, completion, frame.area(), &style()))
      .unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    let mut x = 0;
    while x < width {
      let symbol = buffer[(x, 0)].symbol();
      text.push_str(symbol);
      x += u16::try_from(display_width(symbol).max(1)).unwrap();
    }
    (text.trim_end().to_string(), cursor.unwrap().0)
  }

  #[test]
  fn short_input_is_drawn_as_is_with_the_suggestion() {
    let prompt = Prompt::command("q");
    let completion = CommandCompletion::new(0, 1, "q", vec!["quit".to_string()], true, 0);
    assert_eq!(
      render(&prompt, Some(&completion), 20),
      (":quit".to_string(), 2)
    );

    let mut prompt = Prompt::command("qu");
    prompt.buffer_mut().move_left();
    assert_eq!(
      render(&prompt, Some(&completion), 20),
      (":qu".to_string(), 2)
    );
  }

  /// A wide prefix used to be measured in chars, putting the cursor one
  /// column early per wide character.
  #[test]
  fn cursor_accounts_for_wide_prefix_and_input() {
    let prompt = Prompt::text("搜索: ", "日本");
    assert_eq!(render(&prompt, None, 20), ("搜索: 日本".to_string(), 10));
  }

  /// Input wider than the line used to stay put with the cursor pinned to
  /// the last column, hiding what was being typed.
  #[test]
  fn long_input_scrolls_to_keep_the_cursor_visible() {
    let prompt = Prompt::text("> ", "abcdefghijklmnop");
    assert_eq!(render(&prompt, None, 10), ("hijklmnop".to_string(), 9));

    let mut prompt = prompt;
    prompt.buffer_mut().move_start();
    assert_eq!(render(&prompt, None, 10), ("> abcdefgh".to_string(), 2));

    // A wide character cut by the scroll edge leaves a blank cell.
    let prompt = Prompt::text("> ", "日本語日本語");
    assert_eq!(render(&prompt, None, 10), (" 語日本語".to_string(), 9));
  }

  #[test]
  fn empty_area_draws_nothing() {
    let mut terminal = Terminal::new(TestBackend::new(10, 1)).unwrap();
    terminal
      .draw(|frame| {
        let area = Rect::new(0, 0, 0, 1);
        assert_eq!(
          draw_prompt_line(frame, &Prompt::command(""), None, area, &style()),
          None
        );
      })
      .unwrap();
  }
}
