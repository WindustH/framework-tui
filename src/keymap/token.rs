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
