//! Multi-key dispatch: the pending sequence and its which-key hints.

use super::bindings::{KeyBindings, KeyContext, KeyHint, MatchResult};

/// Feeds key tokens one at a time and tracks a partially typed sequence.
///
/// While a sequence is pending, [`Self::hints`] lists the possible next
/// keys for a which-key display. A key that does not continue the pending
/// sequence drops it and is then matched on its own, so a mistyped prefix
/// never swallows the following key.
#[derive(Debug, Clone, Default)]
pub struct KeyDispatcher {
  pending: Vec<String>,
  hints: Vec<KeyHint>,
}

impl KeyDispatcher {
  pub fn dispatch(
    &mut self,
    bindings: &KeyBindings,
    context: KeyContext,
    token: impl Into<String>,
  ) -> MatchResult {
    self.dispatch_priority(&[bindings], context, token)
  }

  /// Dispatch against a priority queue of bindings: the first entry that
  /// matches (action or prefix) wins; later entries only see keys nobody
  /// before them claimed. Useful for composite surfaces where several
  /// widgets are visible at once and the focused widget's bindings go
  /// first. Multi-key sequences stay with the queue: a sequence started by
  /// any entry continues against the whole queue, and a broken
  /// continuation falls back to the newest token as a fresh key.
  pub fn dispatch_priority(
    &mut self,
    bindings: &[&KeyBindings],
    context: KeyContext,
    token: impl Into<String>,
  ) -> MatchResult {
    let token = token.into();
    if self.pending.is_empty() {
      return self.dispatch_sequence(bindings, context, token);
    }
    match self.dispatch_sequence(bindings, context, token.clone()) {
      MatchResult::None => {
        // The pending sequence broke: retry the newest token as a fresh key.
        self.clear();
        self.dispatch_sequence(bindings, context, token)
      }
      result => result,
    }
  }

  /// Match the pending keys plus `token`, keeping the pending state only
  /// while the sequence is a prefix.
  fn dispatch_sequence(
    &mut self,
    bindings: &[&KeyBindings],
    context: KeyContext,
    token: String,
  ) -> MatchResult {
    self.pending.push(token);
    let result = bindings
      .iter()
      .map(|candidate| candidate.match_sequence(context, &self.pending))
      .find(|result| !matches!(result, MatchResult::None))
      .unwrap_or(MatchResult::None);
    match &result {
      MatchResult::Prefix(hints) => self.hints.clone_from(hints),
      MatchResult::Action(_) => self.clear(),
      MatchResult::None => {
        self.pending.pop();
      }
    }
    result
  }

  /// Drop the pending sequence and its hints.
  pub fn clear(&mut self) {
    self.pending.clear();
    self.hints.clear();
  }

  /// Keys typed so far in the pending sequence.
  pub fn pending(&self) -> &[String] {
    &self.pending
  }

  /// Possible next keys while a sequence is pending; empty otherwise.
  pub fn hints(&self) -> &[KeyHint] {
    &self.hints
  }
}
