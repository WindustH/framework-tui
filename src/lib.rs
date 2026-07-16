//! Reusable TUI interaction building blocks for ratatui apps.
//!
//! `CommandState` owns command history, completion selection, and applying the
//! selected completion to a prompt buffer. `KeyDispatcher` owns multi-key
//! pending state and which-key hints. The input helpers apply common prompt
//! editing actions, the editor helpers run `$EDITOR` on temporary files, and
//! the `widgets` module renders prompt, completion, key-hint, and popup UI so
//! apps can share the same interaction style while supplying their own colors.

pub mod command;
pub mod editor;
pub mod input;
pub mod keymap;
pub mod widgets;

pub use command::{
  CommandCompletion, CommandHistoryCursor, CommandState, Prompt, PromptBuffer, current_word_start,
  filter_completion_candidates,
};
pub use editor::{EditorOptions, edit_text_in_editor, edit_text_in_editor_with_options};
pub use input::{PromptInputResult, handle_prompt_action, handle_prompt_key, handle_prompt_paste};
pub use keymap::{
  KeyBindingConfig, KeyBindings, KeyContext, KeyDispatcher, KeyHelpEntry, KeyHint, MatchResult,
  key_event_to_token,
};
pub use widgets::{
  CompletionListStyle, KeyHelpDialogStyle, KeyHintsStyle, PopupDialogStyle, PromptLineStyle,
  centered_popup_area, completion_rows, default_completion_selected_style, draw_completion_list,
  draw_key_help_dialog, draw_key_hints, draw_popup_dialog, draw_prompt_line, key_hint_columns,
  key_hint_rows,
};
