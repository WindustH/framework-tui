//! Centered popup dialogs.

use ratatui::{
  Frame,
  layout::Rect,
  style::Style,
  text::Text,
  widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::overlay_background;

/// Colors and size limits of a popup dialog. The margins are kept free
/// around the popup when the area is small.
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
    let base = Style::default().bg(overlay_background());
    Self {
      base,
      border: base,
      min_width: 40,
      max_width: 96,
      min_height: 6,
      max_height: 12,
      horizontal_margin: 4,
      vertical_margin: 2,
    }
  }
}

/// The popup rectangle centered in `area`, or `None` when `area` is too
/// small (narrower than `min_width` capped at 20 columns, or lower than
/// `min_height`). The result always lies inside `area`.
pub fn centered_popup_area(area: Rect, style: &PopupDialogStyle) -> Option<Rect> {
  if area.is_empty() || area.width < style.min_width.min(20) || area.height < style.min_height {
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
    area.x + (area.width - width) / 2,
    area.y + (area.height - height) / 2,
    width,
    height,
  ))
}

/// Draw `text` in a bordered popup centered in `area`. Returns the popup
/// rectangle, or `None` when `area` is too small to show it.
pub fn draw_popup_dialog(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  text: Text<'static>,
  style: &PopupDialogStyle,
) -> Option<Rect> {
  render_popup(frame, area, title, text, style, 0)
}

/// Like [`draw_popup_dialog`] but scrolls the text content vertically by
/// `scroll` rows (the title and borders stay fixed). Long lines wrap, and
/// wrapped rows count as rows for scrolling.
pub fn draw_popup_dialog_scrolled(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  text: Text<'static>,
  style: &PopupDialogStyle,
  scroll: usize,
) -> Option<Rect> {
  render_popup(frame, area, title, text, style, scroll)
}

/// [`draw_popup_dialog_scrolled`] for borrowed text.
pub(super) fn render_popup(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  text: Text<'_>,
  style: &PopupDialogStyle,
  scroll: usize,
) -> Option<Rect> {
  let popup = centered_popup_area(area, style)?;
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title)
    .border_style(style.border);
  let scroll = u16::try_from(scroll).unwrap_or(u16::MAX);
  frame.render_widget(Clear, popup);
  frame.render_widget(Block::default().style(style.base), popup);
  frame.render_widget(
    Paragraph::new(text)
      .block(block)
      .style(style.base)
      .scroll((scroll, 0))
      .wrap(Wrap { trim: true }),
    popup,
  );
  Some(popup)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn popup_fits_inside_the_area() {
    let style = PopupDialogStyle::default();
    assert_eq!(
      centered_popup_area(Rect::new(0, 0, 120, 40), &style),
      Some(Rect::new(12, 14, 96, 12))
    );
    assert_eq!(centered_popup_area(Rect::new(0, 0, 19, 40), &style), None);
    assert_eq!(centered_popup_area(Rect::new(0, 0, 40, 5), &style), None);

    // Zero limits used to yield a 1x1 popup outside an empty area.
    let unbounded = PopupDialogStyle {
      min_width: 0,
      min_height: 0,
      ..PopupDialogStyle::default()
    };
    assert_eq!(
      centered_popup_area(Rect::new(80, 24, 0, 0), &unbounded),
      None
    );
    assert_eq!(
      centered_popup_area(Rect::new(80, 24, 0, 5), &unbounded),
      None
    );

    // Margins larger than the area still keep the popup inside it.
    let wide_margins = PopupDialogStyle {
      horizontal_margin: 50,
      vertical_margin: 50,
      ..PopupDialogStyle::default()
    };
    let area = Rect::new(3, 2, 30, 10);
    let popup = centered_popup_area(area, &wide_margins).unwrap();
    assert_eq!(popup.intersection(area), popup);
  }
}
