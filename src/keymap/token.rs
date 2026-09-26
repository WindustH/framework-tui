//! Key tokens: the canonical strings that configured keys and crossterm key
//! events are both turned into, so that matching is plain string equality.
//!
//! Tokens are single characters (`j`, `G`, `?`), named keys (`enter`, `esc`,
//! `tab`, `backtab`, `backspace`, `delete`, `insert`, `space`, `left`,
//! `right`, `up`, `down`, `home`, `end`, `pgup`, `pgdn`, `f1`...) and those
//! with a `ctrl-` or `alt-` modifier. `ctrl-` tokens are always lowercase.

use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Canonical token for one configured key, or `None` for a blank entry.
///
/// Accepts plain names (`j`, `G`, `enter`, `PageDown`, `ctrl-x`, `alt-x`)
/// and vim-style notation (`<CR>`, `<C-x>`, `<A-x>`, `<M-x>`, `<S-Tab>`).
pub(super) fn parse_key(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return None;
  }
  let token = match trimmed
    .strip_prefix('<')
    .and_then(|rest| rest.strip_suffix('>'))
  {
    Some(inner) => parse_bracketed(inner),
    None => parse_plain(trimmed),
  };
  Some(token)
}

/// `<C-x>`, `<A-x>`, `<M-x>`, `<S-Tab>`, `<Enter>`, ...
fn parse_bracketed(inner: &str) -> String {
  if let Some(rest) = strip_prefix_ignore_case(inner, &["c-", "ctrl-"]) {
    return format!("ctrl-{}", key_name(rest).to_ascii_lowercase());
  }
  if let Some(rest) = strip_prefix_ignore_case(inner, &["a-", "m-", "alt-", "meta-"]) {
    return format!("alt-{}", key_name(rest));
  }
  key_name(inner).into_owned()
}

/// `j`, `G`, `enter`, `ctrl-x`, `alt-x`. Modifier forms are lowercased.
fn parse_plain(value: &str) -> String {
  let lower = value.to_ascii_lowercase();
  for modifier in ["ctrl-", "alt-"] {
    if let Some(rest) = lower.strip_prefix(modifier) {
      return format!("{modifier}{}", key_name(rest));
    }
  }
  key_name(value).into_owned()
}

fn strip_prefix_ignore_case<'a>(value: &'a str, prefixes: &[&str]) -> Option<&'a str> {
  prefixes.iter().find_map(|prefix| {
    value
      .get(..prefix.len())
      .filter(|head| head.eq_ignore_ascii_case(prefix))
      .map(|_| &value[prefix.len()..])
  })
}

/// Canonical spelling of a key without modifiers. A single character keeps
/// its case (`g` and `G` are different keys). Named keys match in any case
/// and accept common aliases. Anything else is kept as written, so apps can
/// bind and dispatch their own pseudo keys (such as `mouse_left`).
fn key_name(name: &str) -> Cow<'_, str> {
  let mut chars = name.chars();
  if chars.next().is_some() && chars.next().is_none() {
    return Cow::Borrowed(name);
  }
  let lower = name.to_ascii_lowercase();
  let canonical = match lower.as_str() {
    "space" => "space",
    "enter" | "return" | "cr" => "enter",
    "esc" | "escape" => "esc",
    "tab" => "tab",
    "backtab" | "s-tab" | "shift-tab" => "backtab",
    "backspace" | "bs" => "backspace",
    "delete" | "del" => "delete",
    "insert" => "insert",
    "left" => "left",
    "right" => "right",
    "up" => "up",
    "down" => "down",
    "home" => "home",
    "end" => "end",
    "pgup" | "pageup" | "page-up" => "pgup",
    "pgdn" | "pgdown" | "pagedown" | "page-down" => "pgdn",
    key if is_function_key(key) => return Cow::Owned(lower),
    _ => return Cow::Borrowed(name),
  };
  Cow::Borrowed(canonical)
}

fn is_function_key(name: &str) -> bool {
  name
    .strip_prefix('f')
    .is_some_and(|number| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Token for a key press, or `None` for releases and keys without a token
/// (modifier keys, media keys, ...). Shift is folded into the character
/// (`G`, `?`) and dropped for other keys. Ctrl wins over Alt when both are
/// held, except that symbols typed with AltGr (which Windows reports as
/// Ctrl+Alt) are plain characters.
pub fn key_event_to_token(event: KeyEvent) -> Option<String> {
  if event.kind != KeyEventKind::Press {
    return None;
  }
  if let Some(ch) = altgr_char(&event) {
    return Some(ch.to_string());
  }

  let name: Cow<'static, str> = match event.code {
    KeyCode::Backspace => "backspace".into(),
    KeyCode::Enter => "enter".into(),
    KeyCode::Left => "left".into(),
    KeyCode::Right => "right".into(),
    KeyCode::Up => "up".into(),
    KeyCode::Down => "down".into(),
    KeyCode::Home => "home".into(),
    KeyCode::End => "end".into(),
    KeyCode::PageUp => "pgup".into(),
    KeyCode::PageDown => "pgdn".into(),
    KeyCode::Tab => "tab".into(),
    KeyCode::BackTab => "backtab".into(),
    KeyCode::Delete => "delete".into(),
    KeyCode::Insert => "insert".into(),
    KeyCode::Esc => "esc".into(),
    KeyCode::Char(' ') => "space".into(),
    KeyCode::Char(ch) => ch.to_string().into(),
    KeyCode::F(number) => format!("f{number}").into(),
    _ => return None,
  };

  let token = if event.modifiers.contains(KeyModifiers::CONTROL) {
    format!("ctrl-{}", name.to_ascii_lowercase())
  } else if event.modifiers.contains(KeyModifiers::ALT) {
    format!("alt-{name}")
  } else {
    name.into_owned()
  };
  Some(token)
}

/// A character typed with AltGr.
///
/// Windows reports AltGr as Ctrl+Alt, so on European layouts `@`, `{`, `[`,
/// `\`, `€` or `ł` arrive as Ctrl+Alt chords. Symbols and non-ASCII letters
/// under exactly Ctrl+Alt (plus Shift) are therefore text; Ctrl+Alt with an
/// ASCII letter, digit or space stays a shortcut.
pub(crate) fn altgr_char(event: &KeyEvent) -> Option<char> {
  let KeyCode::Char(ch) = event.code else {
    return None;
  };
  let chord = event.modifiers.difference(KeyModifiers::SHIFT);
  let is_text = !ch.is_ascii_alphanumeric() && ch != ' ' && !ch.is_control();
  (chord == KeyModifiers::CONTROL | KeyModifiers::ALT && is_text).then_some(ch)
}

/// The character a key press types into a text field, if any.
pub(crate) fn typed_char(event: &KeyEvent) -> Option<char> {
  match event.code {
    KeyCode::Char(ch) if event.modifiers.difference(KeyModifiers::SHIFT).is_empty() => Some(ch),
    _ => altgr_char(event),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(value: &str) -> String {
    parse_key(value).unwrap()
  }

  fn press(code: KeyCode, modifiers: KeyModifiers) -> Option<String> {
    key_event_to_token(KeyEvent::new(code, modifiers))
  }

  /// Spellings used by the pdf/gallery/music/calibre default keymaps, the
  /// vim-style forms the parser always handled, and their tokens. These
  /// must not change.
  #[test]
  fn existing_spellings_keep_their_tokens() {
    let cases = [
      ("j", "j"),
      ("G", "G"),
      ("\\", "\\"),
      ("{", "{"),
      ("<", "<"),
      ("space", "space"),
      ("enter", "enter"),
      ("esc", "esc"),
      ("backspace", "backspace"),
      ("delete", "delete"),
      ("backtab", "backtab"),
      ("pgup", "pgup"),
      ("pageup", "pgup"),
      ("PageDown", "pgdn"),
      ("f1", "f1"),
      ("ctrl-x", "ctrl-x"),
      ("Ctrl-X", "ctrl-x"),
      ("alt-x", "alt-x"),
      ("alt-X", "alt-x"),
      ("mouse_left", "mouse_left"),
      ("<space>", "space"),
      ("<Enter>", "enter"),
      ("<return>", "enter"),
      ("<Esc>", "esc"),
      ("<S-Tab>", "backtab"),
      ("shift-tab", "backtab"),
      ("Shift-Tab", "backtab"),
      ("<PageUp>", "pgup"),
      ("<pgdn>", "pgdn"),
      ("<C-x>", "ctrl-x"),
      ("<C-X>", "ctrl-x"),
      ("<c-[>", "ctrl-["),
      ("<C-Space>", "ctrl-space"),
      ("<A-x>", "alt-x"),
      ("<A-X>", "alt-X"),
      ("<a-enter>", "alt-enter"),
      ("<F12>", "f12"),
      ("<x>", "x"),
      ("<ctrl-x>", "ctrl-x"),
      ("  q  ", "q"),
    ];
    for (spelling, token) in cases {
      assert_eq!(parse(spelling), token, "{spelling:?}");
    }
    assert_eq!(parse_key("   "), None);
  }

  /// Named keys used to be case-sensitive and modifiers skipped the alias
  /// table, so these bindings silently never fired.
  #[test]
  fn named_keys_are_normalized_everywhere() {
    let cases = [
      ("Enter", "enter"),
      ("ESC", "esc"),
      ("F5", "f5"),
      ("Left", "left"),
      ("pgdown", "pgdn"),
      ("<BS>", "backspace"),
      ("<Backspace>", "backspace"),
      ("<CR>", "enter"),
      ("<Del>", "delete"),
      ("<Insert>", "insert"),
      ("<A-Enter>", "alt-enter"),
      ("<M-x>", "alt-x"),
      ("<C-PageUp>", "ctrl-pgup"),
      ("<C-Return>", "ctrl-enter"),
      ("<Ctrl-X>", "ctrl-x"),
      ("ctrl-pagedown", "ctrl-pgdn"),
      ("alt-return", "alt-enter"),
    ];
    for (spelling, token) in cases {
      assert_eq!(parse(spelling), token, "{spelling:?}");
    }
  }

  #[test]
  fn every_event_token_parses_to_itself() {
    let codes = [
      KeyCode::Backspace,
      KeyCode::Enter,
      KeyCode::Left,
      KeyCode::Right,
      KeyCode::Up,
      KeyCode::Down,
      KeyCode::Home,
      KeyCode::End,
      KeyCode::PageUp,
      KeyCode::PageDown,
      KeyCode::Tab,
      KeyCode::BackTab,
      KeyCode::Delete,
      KeyCode::Insert,
      KeyCode::Esc,
      KeyCode::F(7),
      KeyCode::Char(' '),
      KeyCode::Char('g'),
      KeyCode::Char('G'),
      KeyCode::Char('-'),
    ];
    for code in codes {
      for modifiers in [KeyModifiers::NONE, KeyModifiers::CONTROL, KeyModifiers::ALT] {
        let token = press(code, modifiers).unwrap();
        // Plain `alt-X` has always been lowercased; `<A-X>` keeps the case.
        let spelling = match token.strip_prefix("alt-") {
          Some(key) if key != key.to_ascii_lowercase() => format!("<A-{key}>"),
          _ => token.clone(),
        };
        assert_eq!(parse(&spelling), token, "{code:?} {modifiers:?}");
      }
    }
  }

  #[test]
  fn event_tokens() {
    assert_eq!(press(KeyCode::Char('G'), KeyModifiers::SHIFT).unwrap(), "G");
    assert_eq!(
      press(
        KeyCode::Char('X'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
      )
      .unwrap(),
      "ctrl-x"
    );
    assert_eq!(
      press(
        KeyCode::Char('x'),
        KeyModifiers::CONTROL | KeyModifiers::ALT
      )
      .unwrap(),
      "ctrl-x"
    );
    assert_eq!(
      press(KeyCode::BackTab, KeyModifiers::SHIFT).unwrap(),
      "backtab"
    );
    assert_eq!(press(KeyCode::CapsLock, KeyModifiers::NONE), None);
    let release = KeyEvent::new_with_kind(
      KeyCode::Char('q'),
      KeyModifiers::NONE,
      KeyEventKind::Release,
    );
    assert_eq!(key_event_to_token(release), None);
  }

  /// Windows reports AltGr+q on a German layout as Ctrl+Alt+'@'.
  #[test]
  fn altgr_characters_are_text() {
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    assert_eq!(press(KeyCode::Char('@'), altgr).unwrap(), "@");
    assert_eq!(press(KeyCode::Char('{'), altgr).unwrap(), "{");
    assert_eq!(
      press(KeyCode::Char('Ł'), altgr | KeyModifiers::SHIFT).unwrap(),
      "Ł"
    );
    assert_eq!(press(KeyCode::Char(' '), altgr).unwrap(), "ctrl-space");
    assert_eq!(
      press(KeyCode::Char('@'), KeyModifiers::CONTROL).unwrap(),
      "ctrl-@"
    );

    assert_eq!(
      typed_char(&KeyEvent::new(KeyCode::Char('€'), altgr)),
      Some('€')
    );
    assert_eq!(typed_char(&KeyEvent::new(KeyCode::Char('a'), altgr)), None);
    assert_eq!(
      typed_char(&KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)),
      Some('A')
    );
    assert_eq!(
      typed_char(&KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT)),
      None
    );
    assert_eq!(
      typed_char(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
      None
    );
  }
}
