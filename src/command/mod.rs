//! Prompt input, command history and command completion.
//!
//! [`Prompt`] and [`PromptBuffer`] hold what is being typed.
//! [`CommandState`] lives next to the prompt for the whole session: it keeps
//! the command history, the history browsing position and the current
//! [`CommandCompletion`]. The app computes completions itself (usually with
//! [`current_word_start`] and [`filter_completion_candidates`]) and hands
//! them to [`CommandState::set_completion_preserving_selection`].

mod completion;
mod history;
mod prompt;

pub use completion::{CommandCompletion, current_word_start, filter_completion_candidates};
pub use history::CommandHistoryCursor;
pub use prompt::{Prompt, PromptBuffer};

/// History, history cursor and completion state for a command prompt.
#[derive(Debug, Clone, Default)]
pub struct CommandState {
  history: Vec<String>,
  history_cursor: CommandHistoryCursor,
  completion: Option<CommandCompletion>,
}

impl CommandState {
  pub fn history(&self) -> &[String] {
    &self.history
  }

  pub fn completion(&self) -> Option<&CommandCompletion> {
    self.completion.as_ref()
  }

  pub fn completion_mut(&mut self) -> Option<&mut CommandCompletion> {
    self.completion.as_mut()
  }

  pub fn clear_completion(&mut self) {
    self.completion = None;
  }

  pub fn reset_history_cursor(&mut self) {
    self.history_cursor.reset();
  }

  /// Forget the history position and the completion, e.g. when a prompt
  /// opens or closes.
  pub fn reset_prompt_state(&mut self) {
    self.reset_history_cursor();
    self.clear_completion();
  }

  /// Record a submitted command. Empty commands and immediate repeats are
  /// skipped.
  pub fn push_history(&mut self, command: impl Into<String>) {
    let command = command.into();
    if !command.is_empty() && self.history.last() != Some(&command) {
      self.history.push(command);
    }
    self.reset_history_cursor();
  }

  pub fn history_previous(&mut self, buffer: &mut PromptBuffer) {
    self.history_cursor.previous(&self.history, buffer);
  }

  pub fn history_next(&mut self, buffer: &mut PromptBuffer) {
    self.history_cursor.next(&self.history, buffer);
  }

  /// Replace the completion. If the previously selected candidate is still
  /// offered it stays selected; a completion without candidates clears it.
  pub fn set_completion_preserving_selection(&mut self, completion: Option<CommandCompletion>) {
    let previous = self.completion.take();
    let previous = previous
      .as_ref()
      .and_then(CommandCompletion::selected_candidate);
    self.completion = completion
      .filter(|completion| !completion.candidates.is_empty())
      .map(|mut completion| {
        let kept = previous.and_then(|previous| {
          completion
            .candidates
            .iter()
            .position(|candidate| candidate == previous)
        });
        let selected = kept.unwrap_or(completion.selected);
        completion.selected = selected.min(completion.candidates.len() - 1);
        completion
      });
  }

  pub fn select_next_completion(&mut self) {
    if let Some(completion) = self.completion_with_candidates() {
      completion.selected = (completion.selected + 1) % completion.candidates.len();
    }
  }

  pub fn select_previous_completion(&mut self) {
    if let Some(completion) = self.completion_with_candidates() {
      let len = completion.candidates.len();
      completion.selected = (completion.selected + len - 1) % len;
    }
  }

  /// Apply the selected completion to `buffer`. Returns whether the buffer
  /// changed.
  pub fn apply_completion(&mut self, buffer: &mut PromptBuffer) -> bool {
    let changed = self
      .completion
      .as_ref()
      .is_some_and(|completion| completion.apply_to(buffer));
    if changed {
      self.reset_history_cursor();
    }
    changed
  }

  fn completion_with_candidates(&mut self) -> Option<&mut CommandCompletion> {
    self
      .completion
      .as_mut()
      .filter(|completion| !completion.candidates.is_empty())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn completion(candidates: &[&str], selected: usize) -> CommandCompletion {
    CommandCompletion::new(
      0,
      0,
      "",
      candidates.iter().map(|value| value.to_string()).collect(),
      true,
      selected,
    )
  }

  #[test]
  fn selection_follows_the_candidate_when_the_list_changes() {
    let mut state = CommandState::default();
    state.set_completion_preserving_selection(Some(completion(&["open", "quit", "sort"], 1)));
    state.set_completion_preserving_selection(Some(completion(&["quit", "sort"], 0)));
    assert_eq!(state.completion().unwrap().selected, 0);

    // The selected candidate disappears and the list shrinks under an
    // out-of-range selection from a literal-constructed completion.
    let mut shrunk = completion(&["sort"], 0);
    shrunk.selected = 7;
    state.set_completion_preserving_selection(Some(shrunk));
    assert_eq!(state.completion().unwrap().selected, 0);

    state.set_completion_preserving_selection(Some(completion(&[], 0)));
    assert!(state.completion().is_none());
  }

  #[test]
  fn selection_wraps_in_both_directions() {
    let mut state = CommandState::default();
    state.set_completion_preserving_selection(Some(completion(&["a", "b", "c"], 0)));
    state.select_previous_completion();
    assert_eq!(state.completion().unwrap().selected, 2);
    state.select_next_completion();
    assert_eq!(state.completion().unwrap().selected, 0);
  }

  #[test]
  fn history_skips_empty_and_repeated_commands() {
    let mut state = CommandState::default();
    state.push_history("quit");
    state.push_history("quit");
    state.push_history("");
    state.push_history("open");
    assert_eq!(state.history(), ["quit", "open"]);
  }
}
