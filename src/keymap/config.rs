//! Keymap files: the schema apps keep their bindings in, its conversion to
//! [`KeyBindingConfig`], a writer for generated keymap files, and the
//! default `input` section.
//!
//! A keymap file has one table per section, each with a `keymap` array:
//!
//! ```toml
//! [browser]
//! keymap = [
//!   { on = "j", run = "move_down", desc = "Move down" },
//!   { on = ["g", "g"], run = "home", desc = "Go to first item" },
//! ]
//! ```
//!
//! Which sections exist, their defaults and how they combine into
//! [`KeyBindings`](super::KeyBindings) stay with the app, in its own
//! top-level config struct made of [`KeymapSection`]s.
//!
//! With the `serde` feature the types implement `Serialize` and
//! `Deserialize`. Unknown fields in a section or an entry are rejected, so a
//! misspelt `keymap` or an extra entry field fails to load instead of
//! silently dropping bindings.

use std::fmt::Write as _;

use super::bindings::KeyBindingConfig;
use super::token::parse_key;

/// One section of a keymap file: `[name]` with its `keymap` array.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, deny_unknown_fields))]
pub struct KeymapSection {
  pub keymap: Vec<KeymapEntry>,
}

/// One binding: the keys in `on` run `run`; `desc` is shown in the key help
/// and in which-key hints.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct KeymapEntry {
  pub on: KeymapOn,
  pub run: String,
  pub desc: String,
}

/// The keys of a binding: one key (`on = "j"`) or a sequence
/// (`on = ["g", "g"]`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum KeymapOn {
  One(String),
  Many(Vec<String>),
}

impl KeymapOn {
  /// The keys as written.
  pub fn keys(&self) -> &[String] {
    match self {
      Self::One(key) => std::slice::from_ref(key),
      Self::Many(keys) => keys,
    }
  }

  /// The keys as canonical tokens (see
  /// [`key_event_to_token`](super::key_event_to_token)), blank keys
  /// dropped. Spellings of the same keys give equal tokens: `<C-c>` and
  /// `ctrl-c`, `<CR>` and `enter`.
  pub fn tokens(&self) -> Vec<String> {
    self
      .keys()
      .iter()
      .filter_map(|key| parse_key(key))
      .collect()
  }
}

impl From<&str> for KeymapOn {
  fn from(key: &str) -> Self {
    Self::One(key.to_string())
  }
}

impl From<String> for KeymapOn {
  fn from(key: String) -> Self {
    Self::One(key)
  }
}

impl<const N: usize> From<[&str; N]> for KeymapOn {
  fn from(keys: [&str; N]) -> Self {
    Self::Many(keys.into_iter().map(str::to_string).collect())
  }
}

impl From<Vec<String>> for KeymapOn {
  fn from(keys: Vec<String>) -> Self {
    Self::Many(keys)
  }
}

impl KeymapEntry {
  pub fn new(on: impl Into<KeymapOn>, run: impl Into<String>, desc: impl Into<String>) -> Self {
    Self {
      on: on.into(),
      run: run.into(),
      desc: desc.into(),
    }
  }
}

/// Shorthand for [`KeymapEntry::new`] in default keymaps:
/// `key("j", "move_down", "Move down")`, `key(["g", "g"], "home", "Go to top")`.
pub fn key(on: impl Into<KeymapOn>, run: &str, desc: &str) -> KeymapEntry {
  KeymapEntry::new(on, run, desc)
}

impl From<&KeymapEntry> for KeyBindingConfig {
  fn from(entry: &KeymapEntry) -> Self {
    Self {
      on: entry.on.keys().to_vec(),
      action: entry.run.clone(),
      desc: entry.desc.clone(),
    }
  }
}

impl From<KeymapEntry> for KeyBindingConfig {
  fn from(entry: KeymapEntry) -> Self {
    let on = match entry.on {
      KeymapOn::One(key) => vec![key],
      KeymapOn::Many(keys) => keys,
    };
    Self {
      on,
      action: entry.run,
      desc: entry.desc,
    }
  }
}

impl KeymapSection {
  pub fn new(keymap: Vec<KeymapEntry>) -> Self {
    Self { keymap }
  }

  /// The entries as [`KeyBindingConfig`]s, for
  /// [`KeyBindings::from_sections`](super::KeyBindings::from_sections).
  pub fn binding_configs(&self) -> Vec<KeyBindingConfig> {
    self.keymap.iter().map(KeyBindingConfig::from).collect()
  }

  /// For each action of `defaults` this section does not bind at all, append
  /// its first default entry, so actions added by a newer version get a key
  /// while the user's own bindings stay as they are.
  pub fn append_missing_actions(&mut self, defaults: &KeymapSection) {
    for default in &defaults.keymap {
      if !self.keymap.iter().any(|entry| entry.run == default.run) {
        self.keymap.push(default.clone());
      }
    }
  }
}

impl From<Vec<KeymapEntry>> for KeymapSection {
  fn from(keymap: Vec<KeymapEntry>) -> Self {
    Self { keymap }
  }
}

impl FromIterator<KeymapEntry> for KeymapSection {
  fn from_iter<I: IntoIterator<Item = KeymapEntry>>(entries: I) -> Self {
    Self {
      keymap: entries.into_iter().collect(),
    }
  }
}

/// A keymap file with `sections` in the given order, each written by
/// [`push_keymap_section`].
pub fn format_keymap_sections<'a>(
  sections: impl IntoIterator<Item = (&'a str, &'a KeymapSection)>,
) -> String {
  let mut out = String::new();
  for (name, section) in sections {
    push_keymap_section(&mut out, name, section);
  }
  out
}

/// Append `[name]` and its `keymap` array to `out`, one inline table per
/// entry and a blank line after the section:
///
/// ```toml
/// [name]
/// keymap = [
///   { on = "j", run = "move_down", desc = "Move down" },
///   { on = ["g", "g"], run = "home", desc = "Go to top" },
/// ]
///
/// ```
pub fn push_keymap_section(out: &mut String, name: &str, section: &KeymapSection) {
  let _ = writeln!(out, "[{name}]");
  out.push_str("keymap = [\n");
  for entry in &section.keymap {
    let _ = writeln!(
      out,
      "  {{ on = {}, run = {}, desc = {} }},",
      format_keymap_on(&entry.on),
      toml_basic_string(&entry.run),
      toml_basic_string(&entry.desc)
    );
  }
  out.push_str("]\n\n");
}

fn format_keymap_on(on: &KeymapOn) -> String {
  match on {
    KeymapOn::One(key) => toml_basic_string(key),
    KeymapOn::Many(keys) => {
      let keys = keys
        .iter()
        .map(|key| toml_basic_string(key))
        .collect::<Vec<_>>()
        .join(", ");
      format!("[{keys}]")
    }
  }
}

/// `value` as a quoted TOML basic string, with `\`, `"` and control
/// characters escaped.
pub fn toml_basic_string(value: &str) -> String {
  let mut out = String::with_capacity(value.len() + 2);
  out.push('"');
  for ch in value.chars() {
    match ch {
      '\\' => out.push_str("\\\\"),
      '"' => out.push_str("\\\""),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      '\u{08}' => out.push_str("\\b"),
      '\u{0c}' => out.push_str("\\f"),
      ch if ch.is_control() => {
        let _ = write!(out, "\\u{:04X}", ch as u32);
      }
      ch => out.push(ch),
    }
  }
  out.push('"');
  out
}

/// How [`default_input_keymap`] lays out the parts that differ between apps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputKeymapOptions {
  /// Description of `f1` → `help`; `None` leaves `help` unbound.
  pub help: Option<String>,
  /// Put `help` right after `esc` → `cancel` instead of first.
  pub help_after_cancel: bool,
  /// Bind `ctrl-g` → `edit_in_editor` (last).
  pub edit_in_editor: bool,
}

impl Default for InputKeymapOptions {
  fn default() -> Self {
    Self {
      help: Some("Show key bindings".to_string()),
      help_after_cancel: false,
      edit_in_editor: false,
    }
  }
}

/// The default `input` section: the prompt actions of
/// [`handle_prompt_action`](crate::handle_prompt_action) on the usual keys
/// (`esc` cancels, `enter` submits, `ctrl-a`/`ctrl-e` jump, `ctrl-u`/`ctrl-k`
/// kill, `tab`/`backtab` cycle completions, `up`/`down` walk the history),
/// plus `help` and optionally `edit_in_editor` as set in `options`.
pub fn default_input_keymap(options: &InputKeymapOptions) -> KeymapSection {
  let help = options.help.as_deref().map(|desc| key("f1", "help", desc));
  let cancel = key("esc", "cancel", "Cancel input");
  let (first, second) = if options.help_after_cancel {
    (Some(cancel), help)
  } else {
    (help, Some(cancel))
  };
  let editor = options
    .edit_in_editor
    .then(|| key("ctrl-g", "edit_in_editor", "Edit input in $EDITOR"));
  first
    .into_iter()
    .chain(second)
    .chain([
      key("enter", "submit", "Submit input"),
      key("backspace", "backspace", "Delete before cursor"),
      key("delete", "delete", "Delete under cursor"),
      key("left", "move_left", "Move cursor left"),
      key("right", "move_right", "Move cursor right"),
      key("home", "move_start", "Move cursor to start"),
      key("ctrl-a", "move_start", "Move cursor to start"),
      key("end", "move_end", "Move cursor to end"),
      key("ctrl-e", "move_end", "Move cursor to end"),
      key("ctrl-u", "kill_before_cursor", "Delete before cursor"),
      key("ctrl-k", "kill_after_cursor", "Delete after cursor"),
      key("tab", "completion_next", "Select next completion"),
      key(
        "backtab",
        "completion_previous",
        "Select previous completion",
      ),
      key("up", "history_previous", "Previous command history"),
      key("down", "history_next", "Next command history"),
    ])
    .chain(editor)
    .collect()
}
