use std::collections::{BTreeMap, BTreeSet};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyContext {
  Browser,
  Detail,
  Input,
}

#[derive(Debug, Clone)]
pub struct KeyBindings {
  browser: Vec<Binding>,
  detail: Vec<Binding>,
  input: Vec<Binding>,
  global: Vec<Binding>,
  /// Opt-in: when true, `global` bindings take priority over the
  /// per-context sections in every non-input context. The default keeps
  /// the historical context-first matching order.
  global_priority: bool,
}

#[derive(Debug, Clone, Default)]
pub struct KeyDispatcher {
  pending: Vec<String>,
  hints: Vec<KeyHint>,
}

#[derive(Debug, Clone)]
struct Binding {
  action: String,
  sequence: Vec<String>,
  desc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchResult {
  None,
  Prefix(Vec<KeyHint>),
  Action(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHint {
  pub key: String,
  pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHelpEntry {
  pub action: String,
  pub keys: String,
  pub description: String,
}

#[derive(Debug, Clone)]
pub struct KeyBindingConfig {
  pub on: Vec<String>,
  pub action: String,
  pub desc: String,
}

impl KeyBindings {
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

  /// Prefer `global` bindings over the per-context sections in non-input
  /// contexts (opt-in). A key bound in both places resolves to the `global`
  /// action instead of the context action.
  pub fn with_global_priority(mut self) -> Self {
    self.global_priority = true;
    self
  }

  pub fn global_priority(&self) -> bool {
    self.global_priority
  }

  pub fn match_sequence(&self, context: KeyContext, sequence: &[String]) -> MatchResult {
    match context {
      KeyContext::Browser => self.match_contexted(&self.browser, sequence),
      KeyContext::Detail => self.match_contexted(&self.detail, sequence),
      KeyContext::Input => match_bindings(self.input.iter(), sequence),
    }
  }

  /// Shared logic for the two non-input contexts: `global_priority`
  /// decides whether the global section is consulted first or as a
  /// fallback after the context section.
  fn match_contexted(&self, section: &[Binding], sequence: &[String]) -> MatchResult {
    if self.global_priority {
      match match_bindings(self.global.iter(), sequence) {
        MatchResult::None => match_bindings(section.iter(), sequence),
        result => result,
      }
    } else {
      match_bindings(section.iter().chain(self.global.iter()), sequence)
    }
  }

  /// Section order for help display: context section followed by global,
  /// unless `global_priority` is set (then global comes first, mirroring
  /// matching order).
  fn context_entries<'a>(&'a self, section: &'a [Binding]) -> Vec<&'a Binding> {
    if self.global_priority {
      self
        .global
        .iter()
        .chain(section.iter())
        .collect::<Vec<_>>()
    } else {
      section
        .iter()
        .chain(self.global.iter())
        .collect::<Vec<_>>()
    }
  }

  pub fn help_entries(&self, context: KeyContext) -> Vec<KeyHelpEntry> {
    self.help_entries_filtered(context, |_| true)
  }

  pub fn help_entries_filtered(
    &self,
    context: KeyContext,
    available: impl Fn(&str) -> bool,
  ) -> Vec<KeyHelpEntry> {
    let bindings = match context {
      KeyContext::Browser => self.context_entries(&self.browser),
      KeyContext::Detail => self.context_entries(&self.detail),
      KeyContext::Input => self.input.iter().collect::<Vec<_>>(),
    };
    collect_help_entries(
      bindings
        .into_iter()
        .filter(|binding| available(&binding.action)),
    )
  }
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
    let result = self.dispatch_sequence(bindings, context, token.clone());
    if !matches!(result, MatchResult::None) || self.pending.is_empty() {
      return result;
    }
    // The pending sequence broke: retry the newest token as a fresh key.
    self.clear();
    self.dispatch_sequence(bindings, context, token)
  }

  fn dispatch_sequence(
    &mut self,
    bindings: &[&KeyBindings],
    context: KeyContext,
    token: String,
  ) -> MatchResult {
    let mut sequence = self.pending.clone();
    sequence.push(token);
    for candidate in bindings {
      match candidate.match_sequence(context, &sequence) {
        MatchResult::Action(action) => {
          self.clear();
          return MatchResult::Action(action);
        }
        MatchResult::Prefix(hints) => {
          self.pending = sequence;
          self.hints = hints.clone();
          return MatchResult::Prefix(hints);
        }
        MatchResult::None => continue,
      }
    }
    MatchResult::None
  }

  pub fn clear(&mut self) {
    self.pending.clear();
    self.hints.clear();
  }

  pub fn pending(&self) -> &[String] {
    &self.pending
  }

  pub fn hints(&self) -> &[KeyHint] {
    &self.hints
  }
}

fn parse_entries(entries: impl IntoIterator<Item = KeyBindingConfig>) -> Vec<Binding> {
  entries
    .into_iter()
    .filter_map(|entry| {
      let sequence = parse_on(entry.on);
      if sequence.is_empty() {
        return None;
      }
      Some(Binding {
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
  let mut next = BTreeMap::<String, BTreeSet<String>>::new();
  for binding in bindings {
    if !binding.sequence.starts_with(sequence) {
      continue;
    }
    if binding.sequence.len() == sequence.len() {
      exact = Some(binding.action.clone());
    } else if let Some(key) = binding.sequence.get(sequence.len()) {
      next
        .entry(key.clone())
        .or_default()
        .insert(binding.desc.clone());
    }
  }

  if let Some(action) = exact
    && next.is_empty()
  {
    return MatchResult::Action(action);
  }

  if !next.is_empty() {
    let hints = next
      .into_iter()
      .map(|(key, labels)| KeyHint {
        key,
        label: labels.into_iter().collect::<Vec<_>>().join(", "),
      })
      .collect();
    return MatchResult::Prefix(hints);
  }

  MatchResult::None
}

fn collect_help_entries<'a>(bindings: impl Iterator<Item = &'a Binding>) -> Vec<KeyHelpEntry> {
  merge_help_entries(bindings.map(|binding| KeyHelpEntry {
    action: binding.action.clone(),
    keys: binding.sequence.join(" "),
    description: binding.desc.clone(),
  }))
}

/// Merge help entries across binding tables: entries with the same action
/// and description combine their key lists ("g c, home"). Duplicate keys
/// — which appear when several tables share a global section — collapse
/// to one ("q, q" -> "q"). Order is preserved — first occurrence wins
/// the slot. Useful together with [`KeyDispatcher::dispatch_priority`]
/// to build one help dialog for a composite surface.
pub fn merge_help_entries(entries: impl IntoIterator<Item = KeyHelpEntry>) -> Vec<KeyHelpEntry> {
  fn push_unique(keys: &mut String, seen: &mut Vec<String>, key: &str) {
    if seen.iter().any(|existing| existing == key) {
      return;
    }
    seen.push(key.to_string());
    if !keys.is_empty() {
      keys.push_str(", ");
    }
    keys.push_str(key);
  }

  let mut merged = Vec::<KeyHelpEntry>::new();
  for entry in entries {
    if let Some(index) = merged
      .iter()
      .position(|existing| existing.action == entry.action && existing.description == entry.description)
    {
      let mut keys = std::mem::take(&mut merged[index].keys);
      let mut seen: Vec<String> = keys.split(", ").map(str::to_string).collect();
      for key in entry.keys.split(", ") {
        push_unique(&mut keys, &mut seen, key);
      }
      merged[index].keys = keys;
    } else {
      merged.push(entry);
    }
  }
  merged
}

fn parse_on(on: Vec<String>) -> Vec<String> {
  on.iter().filter_map(|value| parse_key(value)).collect()
}

fn parse_key(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return None;
  }

  let canonical = if let Some(inner) = trimmed.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
    match inner.to_ascii_lowercase().as_str() {
      "space" => "space".to_string(),
      "enter" | "return" => "enter".to_string(),
      "esc" | "escape" => "esc".to_string(),
      "tab" => "tab".to_string(),
      "backtab" | "s-tab" => "backtab".to_string(),
      "left" => "left".to_string(),
      "right" => "right".to_string(),
      "up" => "up".to_string(),
      "down" => "down".to_string(),
      "home" => "home".to_string(),
      "end" => "end".to_string(),
      "pageup" | "page-up" | "pgup" => "pgup".to_string(),
      "pagedown" | "page-down" | "pgdn" => "pgdn".to_string(),
      "c-[" => "ctrl-[".to_string(),
      key if key.starts_with("c-") => format!("ctrl-{}", inner[2..].to_ascii_lowercase()),
      key if key.starts_with("a-") => format!("alt-{}", &inner[2..]),
      key if key.starts_with('f') => key.to_string(),
      _ => inner.to_string(),
    }
  } else {
    match trimmed.to_ascii_lowercase().as_str() {
      "pageup" => "pgup".to_string(),
      "pagedown" => "pgdn".to_string(),
      other if other.starts_with("ctrl-") => other.to_string(),
      other if other.starts_with("alt-") => other.to_string(),
      _ => trimmed.to_string(),
    }
  };

  Some(canonical)
}

pub fn key_event_to_token(event: KeyEvent) -> Option<String> {
  if event.kind != KeyEventKind::Press {
    return None;
  }

  let base = match event.code {
    KeyCode::Backspace => "backspace".to_string(),
    KeyCode::Enter => "enter".to_string(),
    KeyCode::Left => "left".to_string(),
    KeyCode::Right => "right".to_string(),
    KeyCode::Up => "up".to_string(),
    KeyCode::Down => "down".to_string(),
    KeyCode::Home => "home".to_string(),
    KeyCode::End => "end".to_string(),
    KeyCode::PageUp => "pgup".to_string(),
    KeyCode::PageDown => "pgdn".to_string(),
    KeyCode::Tab => "tab".to_string(),
    KeyCode::BackTab => "backtab".to_string(),
    KeyCode::Delete => "delete".to_string(),
    KeyCode::Insert => "insert".to_string(),
    KeyCode::Esc => "esc".to_string(),
    KeyCode::Char(' ') => "space".to_string(),
    KeyCode::Char(ch) => ch.to_string(),
    KeyCode::F(number) => format!("f{number}"),
    _ => return None,
  };

  if event.modifiers.contains(KeyModifiers::CONTROL) {
    Some(format!("ctrl-{}", base.to_ascii_lowercase()))
  } else if event.modifiers.contains(KeyModifiers::ALT) {
    Some(format!("alt-{base}"))
  } else {
    Some(base)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn config(on: &str, action: &str) -> KeyBindingConfig {
    KeyBindingConfig {
      on: vec![on.to_string()],
      action: action.to_string(),
      desc: String::new(),
    }
  }

  fn bindings() -> KeyBindings {
    KeyBindings::from_sections(
      [config("x", "browser_action")],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      [config("x", "global_action")],
    )
  }

  fn sequence(token: &str) -> Vec<String> {
    vec![token.to_string()]
  }

  fn entry(action: &str, keys: &str, description: &str) -> KeyHelpEntry {
    KeyHelpEntry {
      action: action.to_string(),
      keys: keys.to_string(),
      description: description.to_string(),
    }
  }

  #[test]
  fn merge_help_entries_collapses_duplicate_keys() {
    // Three panes sharing one global section each report "q" for quit.
    let merged = merge_help_entries([
      entry("quit", "q", "Quit"),
      entry("quit", "q", "Quit"),
      entry("quit", "q", "Quit"),
    ]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].keys, "q");

    // Mixed: distinct keys and sequences are kept, repeats collapse.
    let merged = merge_help_entries([
      entry("top", "g c", "Jump to playing"),
      entry("top", "home", "Jump to playing"),
      entry("top", "g c", "Jump to playing"),
    ]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].keys, "g c, home");
  }

  /// Historical semantics: the flattened [section, global] iteration is
  /// last-wins on exact conflicts, so a `global` entry beats a conflicting
  /// context entry even without the opt-in flag.
  #[test]
  fn default_is_last_wins_like_history() {
    let bindings = bindings();
    assert_eq!(
      bindings.match_sequence(KeyContext::Browser, &sequence("x")),
      MatchResult::Action("global_action".to_string())
    );
  }

  /// Historical semantics: a context-section prefix (multi-key sequence)
  /// shadows a `global` exact match because pending hints win whenever any
  /// binding can still be continued.
  #[test]
  fn default_section_prefix_shadows_global_exact() {
    let bindings = KeyBindings::from_sections(
      [KeyBindingConfig {
        on: vec!["g".to_string(), "g".to_string()],
        action: "gg_action".to_string(),
        desc: String::new(),
      }],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      [config("g", "global_action")],
    );
    assert!(matches!(
      bindings.match_sequence(KeyContext::Browser, &sequence("g")),
      MatchResult::Prefix(_)
    ));
  }

  #[test]
  fn global_priority_is_opt_in() {
    let bindings = bindings().with_global_priority();
    assert_eq!(
      bindings.match_sequence(KeyContext::Browser, &sequence("x")),
      MatchResult::Action("global_action".to_string())
    );
    // Input context still only consults the input section.
    assert_eq!(
      bindings.match_sequence(KeyContext::Input, &sequence("x")),
      MatchResult::None
    );
    // Help order mirrors matching order.
    let entries = bindings.help_entries(KeyContext::Browser);
    assert_eq!(entries[0].action, "global_action");

    // Opt-in also removes the historical prefix shadowing: a global exact
    // match fires immediately instead of being shadowed by section prefixes.
    let shadowed = KeyBindings::from_sections(
      [KeyBindingConfig {
        on: vec!["g".to_string(), "g".to_string()],
        action: "gg_action".to_string(),
        desc: String::new(),
      }],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      [config("g", "global_action")],
    )
    .with_global_priority();
    assert_eq!(
      shadowed.match_sequence(KeyContext::Browser, &sequence("g")),
      MatchResult::Action("global_action".to_string())
    );
  }

  #[test]
  fn priority_queue_falls_through_to_later_bindings() {
    let focused = KeyBindings::from_sections(
      [config("x", "focused_action")],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
    let neighbor = KeyBindings::from_sections(
      [config("f", "neighbor_action")],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
    let queue = [&focused, &neighbor];
    let mut dispatcher = KeyDispatcher::default();
    // `f` is claimed by the neighbor when the focused pane has nothing.
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "f"),
      MatchResult::Action("neighbor_action".to_string())
    );
  }

  #[test]
  fn priority_queue_first_match_wins() {
    let focused = KeyBindings::from_sections(
      [config("x", "focused_action")],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
    let neighbor = KeyBindings::from_sections(
      [config("x", "neighbor_action")],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
    let queue = [&focused, &neighbor];
    let mut dispatcher = KeyDispatcher::default();
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "x"),
      MatchResult::Action("focused_action".to_string())
    );
  }

  #[test]
  fn priority_queue_keeps_multi_key_sequences() {
    let focused = KeyBindings::from_sections(
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
    let neighbor = KeyBindings::from_sections(
      [KeyBindingConfig {
        on: vec!["g".to_string(), "g".to_string()],
        action: "neighbor_gg".to_string(),
        desc: String::new(),
      }],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
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
      MatchResult::Action("neighbor_gg".to_string())
    );
  }

  #[test]
  fn priority_queue_broken_sequence_retries_fresh_token() {
    let focused = KeyBindings::from_sections(
      [KeyBindingConfig {
        on: vec!["g".to_string(), "g".to_string()],
        action: "focused_gg".to_string(),
        desc: String::new(),
      }],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
    let neighbor = KeyBindings::from_sections(
      [config("x", "neighbor_action")],
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
      Vec::<KeyBindingConfig>::new(),
    );
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
    // A key that only matches fresh after a broken prefix still fires.
    assert!(matches!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "g"),
      MatchResult::Prefix(_)
    ));
    assert_eq!(
      dispatcher.dispatch_priority(&queue, KeyContext::Browser, "x"),
      MatchResult::Action("neighbor_action".to_string())
    );
  }
}
