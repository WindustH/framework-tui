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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::keymap::KeyBindingConfig;

  fn config(on: &[&str], action: &str) -> KeyBindingConfig {
    KeyBindingConfig {
      on: on.iter().map(|key| key.to_string()).collect(),
      action: action.to_string(),
      desc: String::new(),
    }
  }

  fn browser(entries: Vec<KeyBindingConfig>) -> KeyBindings {
    KeyBindings::from_sections(entries, Vec::new(), Vec::new(), Vec::new())
  }

  fn action(name: &str) -> MatchResult {
    MatchResult::Action(name.to_string())
  }

  #[test]
  fn priority_queue_falls_through_to_later_bindings() {
    let focused = browser(vec![config(&["x"], "focused_action")]);
    let neighbor = browser(vec![config(&["f"], "neighbor_action")]);
    let queue = [&focused, &neighbor];
    let mut dispatcher = KeyDispatcher::default();
    // `f` is claimed by the neighbor when the focused pane has nothing.
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "f"),
      action("neighbor_action")
    );
  }

  #[test]
  fn priority_queue_first_match_wins() {
    let focused = browser(vec![config(&["x"], "focused_action")]);
    let neighbor = browser(vec![config(&["x"], "neighbor_action")]);
    let queue = [&focused, &neighbor];
    let mut dispatcher = KeyDispatcher::default();
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "x"),
      action("focused_action")
    );
  }

  #[test]
  fn priority_queue_keeps_multi_key_sequences() {
    let focused = browser(Vec::new());
    let neighbor = browser(vec![config(&["g", "g"], "neighbor_gg")]);
    let queue = [&focused, &neighbor];
    let mut dispatcher = KeyDispatcher::default();
    assert!(matches!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "g"),
      MatchResult::Prefix(_)
    ));
    // The continuation resolves against the whole queue, not just the list
    // that produced the prefix.
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "g"),
      action("neighbor_gg")
    );
  }

  #[test]
  fn priority_queue_broken_sequence_retries_fresh_token() {
    let focused = browser(vec![config(&["g", "g"], "focused_gg")]);
    let neighbor = browser(vec![config(&["x"], "neighbor_action")]);
    let queue = [&focused, &neighbor];
    let mut dispatcher = KeyDispatcher::default();
    assert!(matches!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "g"),
      MatchResult::Prefix(_)
    ));
    // `z` continues no sequence but matches nothing fresh either.
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "z"),
      MatchResult::None
    );
    assert!(dispatcher.pending().is_empty());
    assert!(dispatcher.hints().is_empty());
    // A key that only matches fresh after a broken prefix still fires.
    assert!(matches!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "g"),
      MatchResult::Prefix(_)
    ));
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "x"),
      action("neighbor_action")
    );
    assert!(dispatcher.pending().is_empty());
  }

  #[test]
  fn pending_sequence_and_hints_follow_the_dispatch() {
    let bindings = browser(vec![config(&["g", "c", "x"], "deep")]);
    let mut dispatcher = KeyDispatcher::default();
    dispatcher.dispatch(&bindings, KeyContext::Browser, "g");
    dispatcher.dispatch(&bindings, KeyContext::Browser, "c");
    assert_eq!(dispatcher.pending(), ["g", "c"]);
    assert_eq!(dispatcher.hints()[0].key, "x");
    // A broken sequence is dropped even when the fresh key matches nothing.
    assert_eq!(
      dispatcher.dispatch(&bindings, KeyContext::Browser, "q"),
      MatchResult::None
    );
    assert!(dispatcher.pending().is_empty());
    assert!(dispatcher.hints().is_empty());
  }
}
