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
  /// An application-defined section name.
  Section(&'static str),
  /// The table's selected section, useful when dispatching across pane tables.
  Active,
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
  sections: BTreeMap<&'static str, Vec<Binding>>,
  section_globals: BTreeMap<&'static str, Vec<Binding>>,
  active: KeyContext,
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
    Self::from_named_sections(
      [
        ("browser", browser.into_iter().collect()),
        ("detail", detail.into_iter().collect()),
        ("input", input.into_iter().collect()),
      ],
      global,
    )
  }

  /// Parse application-defined sections. Input remains isolated from global keys.
  pub fn from_named_sections(
    sections: impl IntoIterator<Item = (&'static str, Vec<KeyBindingConfig>)>,
    global: impl IntoIterator<Item = KeyBindingConfig>,
  ) -> Self {
    Self {
      sections: sections
        .into_iter()
        .map(|(name, entries)| (name, parse_entries(entries)))
        .collect(),
      section_globals: BTreeMap::new(),
      active: KeyContext::Browser,
      global: parse_entries(global),
      global_priority: false,
    }
  }

  pub fn with_active_section(mut self, name: &'static str) -> Self {
    self.active = KeyContext::Section(name);
    self
  }

  /// Global keys have priority except exact sequences explicitly rebound locally.
  /// Compare normalized tokens once, so dispatch and help share the same rule.
  pub fn with_local_overrides(mut self) -> Self {
    for (name, section) in &self.sections {
      self.section_globals.insert(
        name,
        self
          .global
          .iter()
          .filter(|global| {
            !section
              .iter()
              .any(|local| local.sequence == global.sequence)
          })
          .cloned()
          .collect(),
      );
    }
    self.global_priority = true;
    self
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
    let context = if context == KeyContext::Active {
      self.active
    } else {
      context
    };
    let name = match context {
      KeyContext::Browser | KeyContext::Active => "browser",
      KeyContext::Detail => "detail",
      KeyContext::Input => "input",
      KeyContext::Section(name) => name,
    };
    let section = self
      .sections
      .get(name)
      .map(Vec::as_slice)
      .unwrap_or_default();
    if name == "input" {
      return (section, &[]);
    }
    let global = self
      .section_globals
      .get(name)
      .unwrap_or(&self.global)
      .as_slice();
    if self.global_priority {
      (global, section)
    } else {
      (section, global)
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
