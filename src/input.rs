//! Key and paste handling for an open prompt and for the key-help dialog.
//!
//! Prompt keys are looked up in the `input` section of the app's
//! [`KeyBindings`]; the actions understood by [`handle_prompt_action`] are
//! `submit`, `cancel`, `backspace`, `delete`, `move_left`, `move_right`,
//! `move_start`, `move_end`, `kill_before_cursor`, `kill_after_cursor`,
//! `completion_next`, `completion_previous`, `history_previous`,
//! `history_next` and `edit_in_editor`. Anything else comes back as
//! [`PromptInputResult::UnknownAction`] for the app to handle (e.g. `help`).

use crossterm::event::{KeyEvent, KeyEventKind};

use crate::command::{CommandState, Prompt, PromptBuffer};
use crate::keymap::{KeyBindings, KeyContext, MatchResult, key_event_to_token, typed_char};

/// Rows moved by the help dialog's `page_up` / `page_down`.
const HELP_PAGE_ROWS: usize = 10;

/// What a prompt key or paste did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptInputResult {
  /// Not a prompt key; the app may handle it.
  Unhandled,
  /// The input, cursor or completion selection may have changed.
  Changed,
  Cancel,
  Submit,
  /// The user asked to edit the input in `$EDITOR`.
  EditInEditor {
    input: String,
  },
  /// A bound action this module does not implement.
  UnknownAction(String),
}

/// Result of a key press while a scrollable key-help dialog is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpDialogInput {
  /// The scroll position changed (a scroll action fired).
  Scrolled,
  /// A non-scroll key closed the dialog.
  Closed,
  /// The event produced no state change (key release, unmappable key).
  Unhandled,
}

/// Handle a key press for an open key-help dialog, with the scroll keys
/// driven by the caller's `Browser` key bindings:
///
/// - `scroll_up` / `scroll_down` scroll by one row,
/// - `page_up` / `page_down` scroll by ten rows,
/// - any other key (bound or not) closes the dialog.
///
/// `scroll` is clamped to `0..=max_scroll`.
pub fn handle_help_dialog_key(
  scroll: &mut usize,
  max_scroll: usize,
  bindings: &KeyBindings,
  key: KeyEvent,
) -> HelpDialogInput {
  let Some(token) = key_event_to_token(key) else {
    return HelpDialogInput::Unhandled;
  };
  let MatchResult::Action(action) = bindings.match_sequence(KeyContext::Browser, &[token]) else {
    return HelpDialogInput::Closed;
  };
  let next = match action.as_str() {
    "scroll_up" => scroll.saturating_sub(1),
    "scroll_down" => scroll.saturating_add(1),
    "page_up" => scroll.saturating_sub(HELP_PAGE_ROWS),
    "page_down" => scroll.saturating_add(HELP_PAGE_ROWS),
    _ => return HelpDialogInput::Closed,
  };
  *scroll = next.min(max_scroll);
  HelpDialogInput::Scrolled
}

/// Insert pasted text into the prompt (see [`PromptBuffer::insert_str`]).
pub fn handle_prompt_paste(
  prompt: &mut Prompt,
  command_state: &mut CommandState,
  value: &str,
) -> PromptInputResult {
  edit(prompt, command_state, |buffer| buffer.insert_str(value))
}

/// Handle a key press for an open prompt.
///
/// The key is looked up as a single key in the `input` section of
/// `bindings` and its action applied with [`handle_prompt_action`].
/// Unbound printable keys (optionally with Shift, or typed with AltGr) are
/// inserted as text. Input bindings are single keys: a key that only starts
/// a longer input binding is typed as text as well.
pub fn handle_prompt_key(
  prompt: &mut Prompt,
  command_state: &mut CommandState,
  bindings: &KeyBindings,
  key: KeyEvent,
) -> PromptInputResult {
  if key.kind != KeyEventKind::Press {
    return PromptInputResult::Unhandled;
  }
  if let Some(token) = key_event_to_token(key)
    && let MatchResult::Action(action) = bindings.match_sequence(KeyContext::Input, &[token])
  {
    return handle_prompt_action(prompt, command_state, &action);
  }
  match typed_char(&key) {
    Some(ch) => edit(prompt, command_state, |buffer| buffer.insert_char(ch)),
    None => PromptInputResult::Unhandled,
  }
}

/// Apply a prompt action by name (see the module docs for the list).
pub fn handle_prompt_action(
  prompt: &mut Prompt,
  command_state: &mut CommandState,
  action: &str,
) -> PromptInputResult {
  match action {
    "cancel" => PromptInputResult::Cancel,
    "submit" => {
      if prompt.is_command() && command_state.apply_completion(prompt.buffer_mut()) {
        PromptInputResult::Changed
      } else {
        PromptInputResult::Submit
      }
    }
    "backspace" => edit(prompt, command_state, PromptBuffer::backspace),
    "delete" => edit(prompt, command_state, PromptBuffer::delete),
    "kill_before_cursor" => edit(prompt, command_state, PromptBuffer::kill_before_cursor),
    "kill_after_cursor" => edit(prompt, command_state, PromptBuffer::kill_after_cursor),
    "move_left" => move_cursor(prompt, PromptBuffer::move_left),
    "move_right" => move_cursor(prompt, PromptBuffer::move_right),
    "move_start" => move_cursor(prompt, PromptBuffer::move_start),
    "move_end" => move_cursor(prompt, PromptBuffer::move_end),
    "completion_next" => {
      command_state.select_next_completion();
      PromptInputResult::Changed
    }
    "completion_previous" => {
      command_state.select_previous_completion();
      PromptInputResult::Changed
    }
    "history_previous" | "history_next" if !prompt.is_command() => PromptInputResult::Unhandled,
    "history_previous" => {
      command_state.history_previous(prompt.buffer_mut());
      PromptInputResult::Changed
    }
    "history_next" => {
      command_state.history_next(prompt.buffer_mut());
      PromptInputResult::Changed
    }
    "edit_in_editor" => {
      command_state.clear_completion();
      PromptInputResult::EditInEditor {
        input: prompt.buffer().input.clone(),
      }
    }
    other => PromptInputResult::UnknownAction(other.to_string()),
  }
}

/// Change the input text; history browsing starts over from the new text.
fn edit(
  prompt: &mut Prompt,
  command_state: &mut CommandState,
  change: impl FnOnce(&mut PromptBuffer),
) -> PromptInputResult {
  change(prompt.buffer_mut());
  command_state.reset_history_cursor();
  PromptInputResult::Changed
}

fn move_cursor(prompt: &mut Prompt, motion: fn(&mut PromptBuffer)) -> PromptInputResult {
  motion(prompt.buffer_mut());
  PromptInputResult::Changed
}
