//! Binding tables and sequence matching.

use std::collections::{BTreeMap, BTreeSet};

use super::token::parse_key;

/// Which binding section applies. `Browser` and `Detail` also see the
/// `global` section; `Input` (prompt editing) sees only its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyContext {
  Browser,
  Detail,
  Input,
}

/// One configured binding: the keys in `on` pressed in order run `action`.
#[derive(Debug, Clone)]
pub struct KeyBindingConfig {
  pub on: Vec<String>,
  pub action: String,
  pub desc: String,
}

/// Outcome of matching a key sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchResult {
  /// No binding starts with the sequence.
  None,
  /// The sequence starts one or more longer bindings; the hints list the
  /// possible next keys.
  Prefix(Vec<KeyHint>),
  /// The sequence completes a binding.
  Action(String),
}

/// A possible next key while a multi-key sequence is pending, with the
/// descriptions of the bindings it leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHint {
  pub key: String,
  pub label: String,
}

/// Parsed key bindings for every context.
#[derive(Debug, Clone)]
pub struct KeyBindings {
  browser: Vec<Binding>,
  detail: Vec<Binding>,
  input: Vec<Binding>,
  global: Vec<Binding>,
  global_priority: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Binding {
  pub(super) action: String,
  pub(super) sequence: Vec<String>,
  pub(super) desc: String,
}

impl KeyBindings {
  /// Parse the four binding sections. Entries without any usable key are
  /// dropped.
  pub fn from_sections(
    browser: impl IntoIterator<Item = KeyBindingConfig>,
    detail: impl IntoIterator<Item = KeyBindingConfig>,
    input: impl IntoIterator<Item = KeyBindingConfig>,
    global: impl IntoIterator<Item = KeyBindingConfig>,
  ) -> Self {
    Self {
      browser: parse_entries(browser),
      detail: parse_entries(detail),
      input: parse_entries(input),
      global: parse_entries(global),
      global_priority: false,
    }
  }

  /// Let `global` bindings take priority over the context sections (opt-in).
  ///
  /// By default the context section and `global` are matched as one list:
  /// when both bind the same keys the `global` action wins, but a longer
  /// sequence in either list that starts with the pressed keys makes them a
  /// prefix, which hides a shorter binding (e.g. a context `g g` hides a
  /// global `g`). With priority, `global` is matched on its own first and
  /// the context section only sees keys `global` does not claim.
  pub fn with_global_priority(mut self) -> Self {
    self.global_priority = true;
    self
  }

  pub fn global_priority(&self) -> bool {
    self.global_priority
  }

  /// Match a complete key sequence (one token per key) in `context`.
  ///
  /// A sequence that is both a binding and the start of a longer binding
  /// matches as [`MatchResult::Prefix`]: the longer bindings win. Among
  /// bindings for the same keys the last one wins.
  pub fn match_sequence(&self, context: KeyContext, sequence: &[String]) -> MatchResult {
    let (first, second) = self.layers(context);
    if self.global_priority {
      match match_bindings(first.iter(), sequence) {
        MatchResult::None => match_bindings(second.iter(), sequence),
        result => result,
      }
    } else {
      match_bindings(first.iter().chain(second), sequence)
    }
  }

  /// Bindings visible in `context`, in matching (and help) order.
  pub(super) fn visible(&self, context: KeyContext) -> impl Iterator<Item = &Binding> {
    let (first, second) = self.layers(context);
    first.iter().chain(second)
  }

  /// The two binding lists for `context`, the higher-priority one first.
  fn layers(&self, context: KeyContext) -> (&[Binding], &[Binding]) {
    let section = match context {
      KeyContext::Browser => &self.browser,
      KeyContext::Detail => &self.detail,
      KeyContext::Input => return (&self.input, &[]),
    };
    if self.global_priority {
      (&self.global, section)
    } else {
      (section, &self.global)
    }
  }
}

fn parse_entries(entries: impl IntoIterator<Item = KeyBindingConfig>) -> Vec<Binding> {
  entries
    .into_iter()
    .filter_map(|entry| {
      let sequence = entry
        .on
        .iter()
        .filter_map(|key| parse_key(key))
        .collect::<Vec<_>>();
      (!sequence.is_empty()).then_some(Binding {
        action: entry.action,
        sequence,
        desc: entry.desc,
      })
    })
    .collect()
}

fn match_bindings<'a>(
  bindings: impl Iterator<Item = &'a Binding>,
  sequence: &[String],
) -> MatchResult {
  let mut exact = None;
  let mut next = BTreeMap::<&str, BTreeSet<&str>>::new();
  for binding in bindings {
    let Some(rest) = binding.sequence.strip_prefix(sequence) else {
      continue;
    };
    match rest.first() {
      None => exact = Some(binding.action.as_str()),
      Some(key) => {
        let labels = next.entry(key.as_str()).or_default();
        if !binding.desc.is_empty() {
          labels.insert(&binding.desc);
        }
      }
    }
  }

  if !next.is_empty() {
    let hints = next
      .into_iter()
      .map(|(key, labels)| KeyHint {
        key: key.to_string(),
        label: labels.into_iter().collect::<Vec<_>>().join(", "),
      })
      .collect();
    return MatchResult::Prefix(hints);
  }
  exact.map_or(MatchResult::None, |action| {
    MatchResult::Action(action.to_string())
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn config(on: &[&str], action: &str, desc: &str) -> KeyBindingConfig {
    KeyBindingConfig {
      on: on.iter().map(|key| key.to_string()).collect(),
      action: action.to_string(),
      desc: desc.to_string(),
    }
  }

  fn none() -> Vec<KeyBindingConfig> {
    Vec::new()
  }

  fn browser(entries: Vec<KeyBindingConfig>, global: Vec<KeyBindingConfig>) -> KeyBindings {
    KeyBindings::from_sections(entries, none(), none(), global)
  }

  fn keys(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|key| key.to_string()).collect()
  }

  /// The flattened [section, global] list is last-wins on exact conflicts,
  /// so a `global` entry beats a conflicting context entry even without the
  /// opt-in flag.
  #[test]
  fn default_is_last_wins_like_history() {
    let bindings = browser(
      vec![config(&["x"], "browser_action", "")],
      vec![config(&["x"], "global_action", "")],
    );
    assert_eq!(
      bindings.match_sequence(KeyContext::Browser, &keys(&["x"])),
      MatchResult::Action("global_action".to_string())
    );
  }

  /// A context-section prefix (multi-key sequence) shadows a `global` exact
  /// match because pending hints win whenever any binding can still be
  /// continued.
  #[test]
  fn default_section_prefix_shadows_global_exact() {
    let bindings = browser(
      vec![config(&["g", "g"], "gg_action", "")],
      vec![config(&["g"], "global_action", "")],
    );
    assert!(matches!(
      bindings.match_sequence(KeyContext::Browser, &keys(&["g"])),
      MatchResult::Prefix(_)
    ));
  }

  #[test]
  fn global_priority_is_opt_in() {
    let bindings = browser(
      vec![config(&["x"], "browser_action", "")],
      vec![config(&["x"], "global_action", "")],
    )
    .with_global_priority();
    assert_eq!(
      bindings.match_sequence(KeyContext::Browser, &keys(&["x"])),
      MatchResult::Action("global_action".to_string())
    );
    // Input context still only consults the input section.
    assert_eq!(
      bindings.match_sequence(KeyContext::Input, &keys(&["x"])),
      MatchResult::None
    );

    // Opt-in also removes the prefix shadowing: a global exact match fires
    // immediately instead of being shadowed by section prefixes.
    let shadowed = browser(
      vec![config(&["g", "g"], "gg_action", "")],
      vec![config(&["g"], "global_action", "")],
    )
    .with_global_priority();
    assert_eq!(
      shadowed.match_sequence(KeyContext::Browser, &keys(&["g"])),
      MatchResult::Action("global_action".to_string())
    );
  }

  #[test]
  fn prefix_hints_list_next_keys_with_descriptions() {
    let bindings = browser(
      vec![
        config(&["g", "g"], "top", "Go to top"),
        config(&["g", "e"], "end", "Go to end"),
        config(&["g", "e", "x"], "undescribed", ""),
        config(&["<C-x>", "Enter"], "submit", "Submit"),
      ],
      none(),
    );
    assert_eq!(
      bindings.match_sequence(KeyContext::Browser, &keys(&["g"])),
      MatchResult::Prefix(vec![
        // An undescribed binding no longer leaves a stray ", " in the label.
        KeyHint {
          key: "e".to_string(),
          label: "Go to end".to_string()
        },
        KeyHint {
          key: "g".to_string(),
          label: "Go to top".to_string()
        },
      ])
    );
    assert_eq!(
      bindings.match_sequence(KeyContext::Browser, &keys(&["ctrl-x", "enter"])),
      MatchResult::Action("submit".to_string())
    );
    assert_eq!(
      bindings.match_sequence(KeyContext::Browser, &keys(&["q"])),
      MatchResult::None
    );
  }

  #[test]
  fn entries_without_usable_keys_are_dropped() {
    let bindings = browser(vec![config(&["", "  "], "nothing", "")], none());
    assert_eq!(bindings.visible(KeyContext::Browser).count(), 0);
  }
}
