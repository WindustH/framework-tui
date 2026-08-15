use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span, Text},
  widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{CommandCompletion, KeyHelpEntry, KeyHint, Prompt};

#[derive(Debug, Clone)]
pub struct PromptLineStyle {
  pub base: Style,
  pub prefix: Style,
  pub suggestion: Style,
}

#[derive(Debug, Clone)]
pub struct CompletionListStyle {
  pub base: Style,
  pub selected: Style,
}

#[derive(Debug, Clone)]
pub struct KeyHintsStyle {
  pub base: Style,
  pub key: Style,
  pub separator: Style,
  pub description: Style,
  pub separator_text: String,
  pub columns: usize,
}

#[derive(Debug, Clone)]
pub struct KeyHelpDialogStyle {
  pub popup: PopupDialogStyle,
  pub key: Style,
  pub description: Style,
  pub muted: Style,
  pub close_hint: String,
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

#[derive(Debug, Clone)]
pub struct PopupDialogStyle {
  pub base: Style,
  pub border: Style,
  pub min_width: u16,
  pub max_width: u16,
  pub min_height: u16,
  pub max_height: u16,
  pub horizontal_margin: u16,
  pub vertical_margin: u16,
}

impl Default for PopupDialogStyle {
  fn default() -> Self {
    Self {
      base: Style::default(),
      border: Style::default(),
      min_width: 40,
      max_width: 96,
      min_height: 6,
      max_height: 12,
      horizontal_margin: 4,
      vertical_margin: 2,
    }
  }
}

pub fn centered_popup_area(area: Rect, style: &PopupDialogStyle) -> Option<Rect> {
  if area.width < style.min_width.min(20) || area.height < style.min_height {
    return None;
  }
  let available_width = area.width.saturating_sub(style.horizontal_margin).max(1);
  let available_height = area.height.saturating_sub(style.vertical_margin).max(1);
  let width = available_width
    .min(style.max_width)
    .max(available_width.min(style.min_width));
  let height = available_height
    .min(style.max_height)
    .max(available_height.min(style.min_height));
  Some(Rect::new(
    area.x + area.width.saturating_sub(width) / 2,
    area.y + area.height.saturating_sub(height) / 2,
    width,
    height,
  ))
}

pub fn draw_popup_dialog(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  text: Text<'static>,
  style: &PopupDialogStyle,
) -> Option<Rect> {
  draw_popup_dialog_scrolled(frame, area, title, text, style, 0)
}

/// Like [`draw_popup_dialog`] but scrolls the text content vertically by
/// `scroll` rows (the title and borders stay fixed).
pub fn draw_popup_dialog_scrolled(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  text: Text<'static>,
  style: &PopupDialogStyle,
  scroll: usize,
) -> Option<Rect> {
  let popup = centered_popup_area(area, style)?;
  frame.render_widget(Clear, popup);
  frame.render_widget(Block::default().style(style.base), popup);
  frame.render_widget(
    Paragraph::new(text)
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title(title.to_string())
          .border_style(style.border),
      )
      .style(style.base)
      .scroll((scroll as u16, 0))
      .wrap(Wrap { trim: true }),
    popup,
  );
  Some(popup)
}

pub fn draw_key_help_dialog(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  entries: &[KeyHelpEntry],
  style: &KeyHelpDialogStyle,
) -> Option<Rect> {
  draw_key_help_dialog_scrolled(frame, area, title, entries, style, 0)
}

/// Like [`draw_key_help_dialog`] but scrolls the entry list by `scroll`
/// rows. The `close_hint` line always stays pinned at the bottom of the
/// dialog; only the binding entries scroll.
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
    .map(|entry| entry.keys.chars().count())
    .max()
    .unwrap_or(0)
    .min(style.max_key_width);
  let mut lines = Vec::with_capacity(entries.len().saturating_add(2));
  if entries.is_empty() {
    lines.push(Line::from(Span::styled(
      "No bindings available",
      style.muted,
    )));
  } else {
    for entry in entries {
      let keys = truncate_for_width(&entry.keys, key_width.max(1));
      let padding = " ".repeat(key_width.saturating_sub(keys.chars().count()) + 2);
      lines.push(Line::from(vec![
        Span::styled(keys, style.key),
        Span::styled(padding, style.popup.base),
        Span::styled(entry.description.clone(), style.description),
      ]));
    }
  }
  if !style.close_hint.is_empty() {
    lines.push(Line::from(Span::styled(
      style.close_hint.clone(),
      style.muted,
    )));
  }
  draw_popup_dialog_scrolled(frame, area, title, Text::from(lines), &style.popup, scroll)
}

pub fn draw_prompt_line(
  frame: &mut Frame,
  prompt: &Prompt,
  completion: Option<&CommandCompletion>,
  area: Rect,
  style: &PromptLineStyle,
) -> Option<(u16, u16)> {
  if area.width == 0 || area.height == 0 {
    return None;
  }

  let mut spans = vec![
    Span::styled(prompt_prefix(prompt), style.prefix),
    Span::styled(prompt.buffer().input.clone(), style.base),
  ];
  if prompt.is_command()
    && prompt.buffer().cursor == prompt.buffer().input.len()
    && let Some(completion) = completion
  {
    let suggestion = completion.suggestion_suffix();
    if !suggestion.is_empty() {
      spans.push(Span::styled(suggestion, style.suggestion));
    }
  }

  frame.render_widget(Paragraph::new(Line::from(spans)).style(style.base), area);
  let input_width = prompt.buffer().cursor_columns() as u16;
  let prefix_width = prompt_prefix(prompt).chars().count() as u16;
  let cursor_x = prefix_width
    .saturating_add(input_width)
    .min(area.width.saturating_sub(1));
  let position = (area.x.saturating_add(cursor_x), area.y);
  frame.set_cursor_position(position);
  Some(position)
}

pub fn completion_rows(completion: Option<&CommandCompletion>, max_rows: usize) -> u16 {
  completion
    .filter(|completion| !completion.candidates.is_empty())
    .map(|completion| completion.candidates.len().min(max_rows) as u16)
    .unwrap_or(0)
}

pub fn draw_completion_list(
  frame: &mut Frame,
  completion: &CommandCompletion,
  area: Rect,
  style: &CompletionListStyle,
) {
  if completion.candidates.is_empty() || area.width == 0 || area.height == 0 {
    return;
  }

  frame.render_widget(Block::default().style(style.base), area);

  let visible = area.height as usize;
  let selected = completion.selected.min(completion.candidates.len() - 1);
  let start = selected.saturating_sub(visible.saturating_sub(1));
  let mut lines = Vec::with_capacity(visible);
  for row in 0..visible {
    let index = start + row;
    let Some(candidate) = completion.candidates.get(index) else {
      lines.push(Line::from(Span::styled(
        " ".repeat(area.width as usize),
        style.base,
      )));
      continue;
    };
    let selected_row = index == selected;
    let row_style = if selected_row {
      style.selected
    } else {
      style.base
    };
    let marker = if selected_row { "> " } else { "  " };
    let mut text = truncate_for_width(&format!("{marker}{candidate}"), area.width as usize);
    let used = text.chars().count();
    if used < area.width as usize {
      text.push_str(&" ".repeat(area.width as usize - used));
    }
    lines.push(Line::from(Span::styled(text, row_style)));
  }

  frame.render_widget(Paragraph::new(Text::from(lines)).style(style.base), area);
}

pub fn key_hint_columns(configured: usize, width: u16) -> usize {
  let configured = configured.max(1);
  let max_by_width = (usize::from(width) / 24).max(1);
  configured.min(max_by_width).max(1)
}

pub fn key_hint_rows(count: usize, columns: usize) -> u16 {
  count.div_ceil(columns.max(1)).max(1) as u16
}

pub fn draw_key_hints(frame: &mut Frame, hints: &[KeyHint], area: Rect, style: &KeyHintsStyle) {
  if hints.is_empty() || area.width == 0 || area.height == 0 {
    return;
  }

  frame.render_widget(Block::default().style(style.base), area);

  let columns = key_hint_columns(style.columns, area.width);
  let rows = key_hint_rows(hints.len(), columns).min(area.height);
  let cell_width = (area.width as usize / columns.max(1)).max(1);
  let mut lines = Vec::with_capacity(rows as usize);
  for row in 0..rows as usize {
    let mut spans = Vec::new();
    for col in 0..columns {
      let index = row * columns + col;
      if let Some(hint) = hints.get(index) {
        push_key_hint_cell(&mut spans, hint, cell_width, style);
      } else if col + 1 < columns {
        spans.push(Span::styled(" ".repeat(cell_width), style.base));
      }
    }
    lines.push(Line::from(spans));
  }

  frame.render_widget(Paragraph::new(Text::from(lines)).style(style.base), area);
}

pub fn default_completion_selected_style() -> Style {
  Style::default()
    .fg(Color::Black)
    .bg(Color::White)
    .add_modifier(Modifier::BOLD)
}

fn push_key_hint_cell(
  spans: &mut Vec<Span<'static>>,
  hint: &KeyHint,
  cell_width: usize,
  style: &KeyHintsStyle,
) {
  let key = truncate_for_width(&hint.key, cell_width);
  let key_width = key.chars().count();
  spans.push(Span::styled(key, style.key));

  let mut used = key_width;
  if used < cell_width {
    let separator = truncate_for_width(&style.separator_text, cell_width - used);
    used += separator.chars().count();
    spans.push(Span::styled(separator, style.separator));
  }
  if used < cell_width {
    let desc = truncate_for_width(&hint.label, cell_width - used);
    used += desc.chars().count();
    spans.push(Span::styled(desc, style.description));
  }
  if used < cell_width {
    spans.push(Span::styled(" ".repeat(cell_width - used), style.base));
  }
}

fn prompt_prefix(prompt: &Prompt) -> &str {
  prompt.prefix()
}

fn truncate_for_width(value: &str, width: usize) -> String {
  if width == 0 {
    return String::new();
  }
  if width <= 3 {
    return value.chars().take(width).collect();
  }
  let mut out = String::new();
  for ch in value.chars() {
    if out.chars().count() + 1 >= width {
      out.push_str("...");
      return out;
    }
    out.push(ch);
  }
  out
}
