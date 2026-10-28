//! Shared theme color syntax. Unknown colors fall back to terminal defaults.
use ratatui::style::Color;

/// Accept named colors, light/bright aliases, ANSI indices and #rrggbb.
/// Separators and case are ignored; white always means ratatui's White.
pub fn parse_color(value: &str) -> Color {
  let value = value.trim();
  if let Some(hex) = value.strip_prefix('#') {
    if hex.len() == 6
      && hex.bytes().all(|c| c.is_ascii_hexdigit())
      && let Ok(rgb) = u32::from_str_radix(hex, 16)
    {
      let [_, r, g, b] = rgb.to_be_bytes();
      return Color::Rgb(r, g, b);
    }
    return Color::Reset;
  }
  let mut name = [0u8; 32];
  let mut length = 0;
  for byte in value.bytes().filter(|b| !matches!(b, b' ' | b'_' | b'-')) {
    if length == name.len() {
      return Color::Reset;
    }
    name[length] = byte.to_ascii_lowercase();
    length += 1;
  }
  let name = std::str::from_utf8(&name[..length]).unwrap_or("");
  match name {
    "reset" | "default" => Color::Reset,
    "black" => Color::Black,
    "red" => Color::Red,
    "green" => Color::Green,
    "yellow" => Color::Yellow,
    "blue" => Color::Blue,
    "magenta" => Color::Magenta,
    "cyan" => Color::Cyan,
    "gray" | "grey" => Color::Gray,
    "darkgray" | "darkgrey" | "brightblack" | "lightblack" => Color::DarkGray,
    "lightred" | "brightred" => Color::LightRed,
    "lightgreen" | "brightgreen" => Color::LightGreen,
    "lightyellow" | "brightyellow" => Color::LightYellow,
    "lightblue" | "brightblue" => Color::LightBlue,
    "lightmagenta" | "brightmagenta" => Color::LightMagenta,
    "lightcyan" | "brightcyan" => Color::LightCyan,
    "white" | "brightwhite" | "lightwhite" => Color::White,
    _ => name
      .strip_prefix("ansi:")
      .unwrap_or(name)
      .parse::<u8>()
      .map(Color::Indexed)
      .unwrap_or(Color::Reset),
  }
}
