//! Reusable command-line and keybinding building blocks for ratatui apps.
//!
//! `CommandState` owns command history, completion selection, and applying the
//! selected completion to a prompt buffer. `KeyDispatcher` owns multi-key
//! pending state and which-key hints. The `widgets` module renders the matching
//! prompt, completion, and key-hint UI so apps can share the same interaction
//! style while supplying their own colors.

pub mod command;
pub mod keymap;
pub mod widgets;

pub use command::{
  CommandCompletion, CommandHistoryCursor, CommandState, Prompt, PromptBuffer, current_word_start,
  filter_completion_candidates,
};
pub use keymap::{
  KeyBindingConfig, KeyBindings, KeyContext, KeyDispatcher, KeyHint, MatchResult,
  key_event_to_token,
};
pub use widgets::{
  CompletionListStyle, KeyHintsStyle, PromptLineStyle, completion_rows,
  default_completion_selected_style, draw_completion_list, draw_key_hints, draw_prompt_line,
  key_hint_columns, key_hint_rows,
};
