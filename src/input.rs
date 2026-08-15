use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{CommandState, KeyBindings, KeyContext, MatchResult, Prompt, key_event_to_token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptInputResult {
  Unhandled,
  Changed,
  Cancel,
  Submit,
  EditInEditor { input: String },
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
/// driven by the caller's key bindings:
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
  if key.kind != KeyEventKind::Press {
    return HelpDialogInput::Unhandled;
  }
  let Some(token) = key_event_to_token(key) else {
    return HelpDialogInput::Unhandled;
  };
  let sequence = [token];
  match bindings.match_sequence(KeyContext::Browser, &sequence) {
    MatchResult::Action(action) => {
      let delta: i32 = match action.as_str() {
        "scroll_up" => -1,
        "scroll_down" => 1,
        "page_up" => -10,
        "page_down" => 10,
        _ => return HelpDialogInput::Closed,
      };
      let next = if delta < 0 {
        scroll.saturating_sub(delta.unsigned_abs() as usize)
      } else {
        scroll.saturating_add(delta as usize)
      };
      *scroll = next.min(max_scroll);
      HelpDialogInput::Scrolled
    }
    MatchResult::Prefix(_) | MatchResult::None => HelpDialogInput::Closed,
  }
}

pub fn handle_prompt_paste(
  prompt: &mut Prompt,
  command_state: &mut CommandState,
  value: &str,
) -> PromptInputResult {
  prompt.buffer_mut().insert_str(value);
  command_state.reset_history_cursor();
  PromptInputResult::Changed
}

pub fn handle_prompt_key(
  prompt: &mut Prompt,
  command_state: &mut CommandState,
  bindings: &KeyBindings,
  key: KeyEvent,
) -> PromptInputResult {
  if key.kind != KeyEventKind::Press {
    return PromptInputResult::Unhandled;
  }

  if let Some(token) = key_event_to_token(key) {
    match bindings.match_sequence(KeyContext::Input, &[token]) {
      MatchResult::Action(action) => return handle_prompt_action(prompt, command_state, &action),
      MatchResult::Prefix(_) => return PromptInputResult::Unhandled,
      MatchResult::None => {}
    }
  }

  match key.code {
    KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
      prompt.buffer_mut().insert_char(ch);
      command_state.reset_history_cursor();
      PromptInputResult::Changed
    }
    _ => PromptInputResult::Unhandled,
  }
}

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
    "backspace" => {
      prompt.buffer_mut().backspace();
      command_state.reset_history_cursor();
      PromptInputResult::Changed
    }
    "delete" => {
      prompt.buffer_mut().delete();
      command_state.reset_history_cursor();
      PromptInputResult::Changed
    }
    "move_left" => {
      prompt.buffer_mut().move_left();
      PromptInputResult::Changed
    }
    "move_right" => {
      prompt.buffer_mut().move_right();
      PromptInputResult::Changed
    }
    "move_start" => {
      prompt.buffer_mut().move_start();
      PromptInputResult::Changed
    }
    "move_end" => {
      prompt.buffer_mut().move_end();
      PromptInputResult::Changed
    }
    "kill_before_cursor" => {
      prompt.buffer_mut().kill_before_cursor();
      command_state.reset_history_cursor();
      PromptInputResult::Changed
    }
    "kill_after_cursor" => {
      prompt.buffer_mut().kill_after_cursor();
      command_state.reset_history_cursor();
      PromptInputResult::Changed
    }
    "completion_next" => {
      command_state.select_next_completion();
      PromptInputResult::Changed
    }
    "completion_previous" => {
      command_state.select_previous_completion();
      PromptInputResult::Changed
    }
    "history_previous" => {
      if prompt.is_command() {
        command_state.history_previous(prompt.buffer_mut());
        PromptInputResult::Changed
      } else {
        PromptInputResult::Unhandled
      }
    }
    "history_next" => {
      if prompt.is_command() {
        command_state.history_next(prompt.buffer_mut());
        PromptInputResult::Changed
      } else {
        PromptInputResult::Unhandled
      }
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
