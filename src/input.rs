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
