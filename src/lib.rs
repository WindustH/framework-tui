//! Reusable interaction building blocks for ratatui apps.
//!
//! - [`command`]: the prompt buffer, command history and command
//!   completion ([`Prompt`], [`PromptBuffer`], [`CommandState`]).
//! - [`input`]: applies key presses and pastes to an open prompt through
//!   the app's key bindings, and scrolls or closes the key-help dialog.
//! - [`keymap`]: configurable key bindings with per-context sections,
//!   multi-key sequences, which-key hints and help listings
//!   ([`KeyBindings`], [`KeyDispatcher`]), and the keymap file schema
//!   ([`KeymapSection`]; serde support with the `serde` feature).
//! - [`editor`]: edits text in `$EDITOR` through a temporary file.
//! - [`terminal`]: owns the terminal ([`TerminalSession`]), restores it on
//!   panics, reads input on a pausable thread ([`InputReader`]) and hands
//!   the terminal to other programs ([`edit_text_outside_tui`]).
//! - [`widgets`]: draws the prompt line, completion list, key hints, popup
//!   dialogs and the key-help dialog with app-supplied colors.

pub mod command;
pub mod editor;
pub mod input;
pub mod keymap;
pub mod terminal;
mod text;
pub mod widgets;

pub use command::{
  CommandCompletion, CommandHistoryCursor, CommandState, Prompt, PromptBuffer, current_word_start,
  filter_completion_candidates,
};
pub use editor::{EditorOptions, edit_text_in_editor, edit_text_in_editor_with_options};
pub use input::{
  HelpDialogInput, PromptInputResult, handle_help_dialog_key, handle_prompt_action,
  handle_prompt_key, handle_prompt_paste,
};
pub use keymap::{
  InputKeymapOptions, KeyBindingConfig, KeyBindings, KeyContext, KeyDispatcher, KeyHelpEntry,
  KeyHint, KeymapEntry, KeymapOn, KeymapSection, MatchResult, default_input_keymap,
  format_keymap_sections, key_event_to_token, merge_help_entries, push_keymap_section,
  toml_basic_string,
};
pub use terminal::{
  HandOff, InputEvent, InputReader, PanicOrigin, SessionBackend, SuspendTerminal, TerminalOptions,
  TerminalOutput, TerminalSession, TerminationSignal, discard_pending_events,
  edit_text_outside_tui, emergency_restore, install_panic_hook, run_outside_tui,
  watch_termination_signals,
};
pub use widgets::{
  CompletionListStyle, KeyHelpDialogStyle, KeyHintsStyle, PopupDialogStyle, PromptLineStyle,
  centered_popup_area, completion_list_style, completion_rows, default_completion_selected_style,
  draw_completion_list, draw_key_help_dialog, draw_key_help_dialog_scrolled, draw_key_hints,
  draw_popup_dialog, draw_popup_dialog_scrolled, draw_prompt_line, key_hint_columns, key_hint_rows,
  overlay_background,
};
