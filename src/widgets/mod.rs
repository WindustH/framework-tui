//! Drawing helpers for the shared interaction UI: the prompt line, the
//! completion list, which-key hints, popup dialogs and the key-help dialog.
//!
//! Each widget takes a style struct so apps supply their own colors. Width
//! calculations count terminal columns, so wide (CJK) text lines up.

mod completion_list;
mod key_help;
mod key_hints;
mod popup;
mod prompt_line;

use ratatui::style::Color;

pub use completion_list::{
  CompletionListStyle, completion_list_style, completion_rows, default_completion_selected_style,
  draw_completion_list,
};
pub use key_help::{KeyHelpDialogStyle, draw_key_help_dialog, draw_key_help_dialog_scrolled};
pub use key_hints::{KeyHintsStyle, draw_key_hints, key_hint_columns, key_hint_rows};
pub use popup::{
  PopupDialogStyle, centered_popup_area, draw_popup_dialog, draw_popup_dialog_scrolled,
};
pub use prompt_line::{PromptLineStyle, draw_prompt_line};

/// Keep overlay surfaces on the terminal's configured default background.
pub const fn overlay_background() -> Color {
  Color::Reset
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn overlay_background_uses_terminal_reset() {
    assert_eq!(overlay_background(), Color::Reset);
    assert_eq!(PopupDialogStyle::default().base.bg, Some(Color::Reset));
  }
}
