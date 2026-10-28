//! Key-help listings built from binding tables.

use super::bindings::{Binding, KeyBindings, KeyContext};

/// One row of a key-help listing: every key sequence bound to `action`
/// with the same description, e.g. keys `"g g, home"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHelpEntry {
  pub action: String,
  pub keys: String,
  pub description: String,
}

impl KeyBindings {
  /// Help rows for `context`, in matching order (context section and
  /// `global` for `Browser`/`Detail`, the input section for `Input`).
  pub fn help_entries(&self, context: KeyContext) -> Vec<KeyHelpEntry> {
    self.help_entries_filtered(context, |_| true)
  }

  /// Like [`Self::help_entries`], keeping only actions for which
  /// `available` returns true.
  pub fn help_entries_filtered(
    &self,
    context: KeyContext,
    available: impl Fn(&str) -> bool,
  ) -> Vec<KeyHelpEntry> {
    merge_help_entries(
      self
        .visible(context)
        .filter(|binding| available(&binding.action))
        .map(Binding::help_entry),
    )
  }
}

impl Binding {
  fn help_entry(&self) -> KeyHelpEntry {
    KeyHelpEntry {
      action: self.action.clone(),
      keys: self.sequence.join(" "),
      description: self.desc.clone(),
    }
  }
}

/// Merge help entries across binding tables: entries with the same action
/// and description combine their key lists ("g c, home"). Duplicate keys,
/// which appear when several tables share a global section, collapse to
/// one ("q, q" -> "q"). Order is preserved; the first occurrence keeps its
/// slot. Useful together with [`super::KeyDispatcher::dispatch_priority`]
/// to build one help dialog for a composite surface.
pub fn merge_help_entries(entries: impl IntoIterator<Item = KeyHelpEntry>) -> Vec<KeyHelpEntry> {
  let mut merged = Vec::<KeyHelpEntry>::new();
  for entry in entries {
    let existing = merged.iter_mut().find(|existing| {
      existing.action == entry.action && existing.description == entry.description
    });
    match existing {
      Some(existing) => merge_keys(&mut existing.keys, &entry.keys),
      None => merged.push(entry),
    }
  }
  merged
}

/// Append the keys of `incoming` that `keys` does not list yet.
fn merge_keys(keys: &mut String, incoming: &str) {
  let mut seen = keys.split(", ").map(str::to_string).collect::<Vec<_>>();
  for key in incoming.split(", ") {
    if seen.iter().any(|existing| existing == key) {
      continue;
    }
    if !keys.is_empty() {
      keys.push_str(", ");
    }
    keys.push_str(key);
    seen.push(key.to_string());
  }
}
