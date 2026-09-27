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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn keys_and_tokens() {
    let one = KeymapOn::from("<C-c>");
    assert_eq!(one.keys(), ["<C-c>".to_string()]);
    assert_eq!(one.tokens(), vec!["ctrl-c".to_string()]);
    assert_eq!(one.tokens(), KeymapOn::from("ctrl-c").tokens());

    let many = KeymapOn::from(["g", " ", "<CR>"]);
    assert_eq!(many.keys().len(), 3);
    assert_eq!(many.tokens(), vec!["g".to_string(), "enter".to_string()]);
    assert!(KeymapOn::Many(Vec::new()).tokens().is_empty());
  }

  #[test]
  fn entries_convert_to_binding_configs() {
    let section = KeymapSection::new(vec![
      key("j", "down", "Move down"),
      key(["g", "g"], "top", "Go to top"),
    ]);
    let configs = section.binding_configs();
    assert_eq!(configs[0].on, vec!["j".to_string()]);
    assert_eq!(configs[1].on, vec!["g".to_string(), "g".to_string()]);
    assert_eq!(configs[1].action, "top");
    assert_eq!(configs[1].desc, "Go to top");
    let owned = KeyBindingConfig::from(section.keymap[1].clone());
    assert_eq!(owned.on, configs[1].on);
  }

  #[test]
  fn append_missing_actions_keeps_user_bindings() {
    let mut section = KeymapSection::new(vec![key("x", "quit", "Leave")]);
    let defaults = KeymapSection::new(vec![
      key("q", "quit", "Quit"),
      key("j", "down", "Move down"),
      key("down", "down", "Move down"),
    ]);
    section.append_missing_actions(&defaults);
    let keys = section
      .keymap
      .iter()
      .map(|entry| (entry.on.keys()[0].as_str(), entry.run.as_str()))
      .collect::<Vec<_>>();
    // Only the first default of a missing action is added.
    assert_eq!(keys, [("x", "quit"), ("j", "down")]);
  }

  #[test]
  fn writer_escapes_strings() {
    assert_eq!(toml_basic_string("a\"b\\c"), r#""a\"b\\c""#);
    assert_eq!(toml_basic_string("\t\n\u{1}"), r#""\t\n\u0001""#);
    let out = format_keymap_sections([(
      "browser",
      &KeymapSection::new(vec![key(["g", "\""], "run \\", "Tab\there")]),
    )]);
    assert_eq!(
      out,
      "[browser]\nkeymap = [\n  { on = [\"g\", \"\\\"\"], run = \"run \\\\\", desc = \"Tab\\there\" },\n]\n\n"
    );
  }

  #[test]
  fn input_keymap_layouts() {
    let actions = |section: &KeymapSection| {
      section
        .keymap
        .iter()
        .map(|entry| entry.run.clone())
        .collect::<Vec<_>>()
    };

    let plain = default_input_keymap(&InputKeymapOptions::default());
    assert_eq!(plain.keymap.len(), 17);
    assert_eq!(actions(&plain)[..3], ["help", "cancel", "submit"]);
    assert_eq!(plain.keymap[0].desc, "Show key bindings");

    let after_cancel = default_input_keymap(&InputKeymapOptions {
      help: Some("Show input key bindings".to_string()),
      help_after_cancel: true,
      ..InputKeymapOptions::default()
    });
    assert_eq!(actions(&after_cancel)[..3], ["cancel", "help", "submit"]);
    assert_eq!(after_cancel.keymap[1].desc, "Show input key bindings");

    let editor = default_input_keymap(&InputKeymapOptions {
      edit_in_editor: true,
      ..InputKeymapOptions::default()
    });
    assert_eq!(editor.keymap.len(), 18);
    assert_eq!(
      editor.keymap[17],
      key("ctrl-g", "edit_in_editor", "Edit input in $EDITOR")
    );

    let no_help = default_input_keymap(&InputKeymapOptions {
      help: None,
      ..InputKeymapOptions::default()
    });
    assert_eq!(no_help.keymap.len(), 16);
    assert!(!actions(&no_help).contains(&"help".to_string()));

    // Every action is one the prompt handles or the app's `help`.
    for action in actions(&editor) {
      assert!(
        [
          "help",
          "cancel",
          "submit",
          "backspace",
          "delete",
          "move_left",
          "move_right",
          "move_start",
          "move_end",
          "kill_before_cursor",
          "kill_after_cursor",
          "completion_next",
          "completion_previous",
          "history_previous",
          "history_next",
          "edit_in_editor",
        ]
        .contains(&action.as_str()),
        "{action}"
      );
    }
  }
}

/// Byte-for-byte checks against the keymap files the apps generated before
/// they moved to these types (captured from pdf-tui, gallery-tui and
/// music-tui's own writers, and calibre-tui's serde output), so users'
/// generated files do not change.
#[cfg(all(test, feature = "serde"))]
mod file_tests {
  use super::*;

  const PDF: &str = include_str!("testdata/pdf-tui-keymap.toml");
  const GALLERY: &str = include_str!("testdata/gallery-tui-keymap.toml");
  const MUSIC: &str = include_str!("testdata/music-tui-keymap.toml");
  const CALIBRE: &str = include_str!("testdata/calibre-tui-keymap.toml");

  /// The sections of a generated file, in file order.
  fn sections(file: &str) -> Vec<(String, KeymapSection)> {
    let table: toml::Table = toml::from_str(file).unwrap();
    file
      .lines()
      .filter_map(|line| line.strip_prefix('[')?.strip_suffix(']'))
      .map(|name| {
        let section = table[name].clone().try_into().unwrap();
        (name.to_string(), section)
      })
      .collect()
  }

  fn rewrite(file: &str) -> String {
    let sections = sections(file);
    format_keymap_sections(
      sections
        .iter()
        .map(|(name, section)| (name.as_str(), section)),
    )
  }

  fn section(file: &str, name: &str) -> KeymapSection {
    sections(file)
      .into_iter()
      .find(|(section, _)| section == name)
      .unwrap()
      .1
  }

  #[test]
  fn writer_reproduces_generated_files() {
    for (app, file) in [
      ("pdf-tui", PDF),
      ("gallery-tui", GALLERY),
      ("music-tui", MUSIC),
    ] {
      assert_eq!(rewrite(file), file, "{app}");
    }
  }

  #[test]
  fn input_defaults_match_each_app() {
    let pdf_and_music = default_input_keymap(&InputKeymapOptions {
      help: Some("Show input key bindings".to_string()),
      help_after_cancel: true,
      ..InputKeymapOptions::default()
    });
    assert_eq!(section(PDF, "input"), pdf_and_music);
    assert_eq!(section(MUSIC, "input"), pdf_and_music);
    let gallery = default_input_keymap(&InputKeymapOptions {
      edit_in_editor: true,
      ..InputKeymapOptions::default()
    });
    assert_eq!(section(GALLERY, "input"), gallery);

    let calibre: CalibreKeymap = toml::from_str(CALIBRE).unwrap();
    assert_eq!(
      calibre.input,
      default_input_keymap(&InputKeymapOptions::default())
    );
  }

  /// calibre-tui writes its keymap through serde (`toml::to_string_pretty`)
  /// with its own comment layer on top.
  #[derive(Debug, serde::Serialize, serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct CalibreKeymap {
    browser: KeymapSection,
    detail: KeymapSection,
    input: KeymapSection,
    global: KeymapSection,
  }

  #[test]
  fn serde_output_matches_calibre() {
    let parsed: CalibreKeymap = toml::from_str(CALIBRE).unwrap();
    assert_eq!(toml::to_string_pretty(&parsed).unwrap(), CALIBRE);
  }

  #[test]
  fn schema_round_trips() {
    let section = KeymapSection::new(vec![
      key("j", "down", "Move down"),
      key(["g", "g"], "top", "Go to top"),
    ]);
    let file = format_keymap_sections([("browser", &section)]);
    let table: toml::Table = toml::from_str(&file).unwrap();
    let parsed: KeymapSection = table["browser"].clone().try_into().unwrap();
    assert_eq!(parsed, section);

    // A missing `keymap` is an empty section.
    let empty: KeymapSection = toml::from_str("").unwrap();
    assert!(empty.keymap.is_empty());
  }

  #[test]
  fn unknown_fields_are_rejected() {
    let typo = toml::from_str::<KeymapSection>("keymapp = []");
    assert!(typo.is_err());
    let extra = toml::from_str::<KeymapSection>(
      r#"keymap = [{ on = "j", run = "down", desc = "", mode = "x" }]"#,
    );
    assert!(extra.is_err());
    let missing_desc = toml::from_str::<KeymapSection>(r#"keymap = [{ on = "j", run = "down" }]"#);
    assert!(missing_desc.is_err());
  }
}
