//! Walking command history from the prompt.

use super::prompt::PromptBuffer;

/// Position while browsing history. Stepping past the newest entry restores
/// the draft that was being typed before browsing started.
#[derive(Debug, Clone, Default)]
pub struct CommandHistoryCursor {
  index: Option<usize>,
  draft: Option<String>,
}

impl CommandHistoryCursor {
  pub fn reset(&mut self) {
    self.index = None;
    self.draft = None;
  }

  /// Load the next older entry into `buffer`, saving the draft on the
  /// first step. Stays on the oldest entry.
  pub fn previous(&mut self, history: &[String], buffer: &mut PromptBuffer) {
    let Some(last) = history.len().checked_sub(1) else {
      return;
    };
    let index = match self.index {
      Some(index) => index.saturating_sub(1).min(last),
      None => {
        self.draft = Some(buffer.input.clone());
        last
      }
    };
    self.index = Some(index);
    buffer.set_input(history[index].clone());
  }

  /// Load the next newer entry into `buffer`, or the saved draft once past
  /// the newest entry.
  pub fn next(&mut self, history: &[String], buffer: &mut PromptBuffer) {
    let Some(index) = self.index else {
      return;
    };
    if let Some(entry) = history.get(index + 1) {
      self.index = Some(index + 1);
      buffer.set_input(entry.clone());
    } else {
      self.index = None;
      buffer.set_input(self.draft.take().unwrap_or_default());
    }
  }
}
