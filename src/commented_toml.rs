//! Add application-supplied comments to serialized TOML without duplicating table comments.
use std::{collections::BTreeSet, fmt::Write as _};
pub fn add_comments(
  body: &str,
  header: &[&str],
  comment_for: fn(&str) -> Option<&'static str>,
  normalize_table: fn(&str) -> String,
) -> String {
  let mut out = String::new();
  let mut seen_comments = BTreeSet::new();
  for line in header {
    push_toml_comment(&mut out, line);
  }
  out.push('\n');

  let mut table = String::new();
  for line in body.lines() {
    let trimmed = line.trim();
    if let Some(header) = toml_table_header(trimmed) {
      table = normalize_table(header);
      if seen_comments.insert(table.clone())
        && let Some(comment) = comment_for(&table)
      {
        push_toml_comment(&mut out, comment);
      }
    } else if let Some(key) = toml_field_key(trimmed) {
      let comment_key = if table.is_empty() {
        key.to_string()
      } else {
        format!("{table}.{key}")
      };
      if seen_comments.insert(comment_key.clone())
        && let Some(comment) = comment_for(&comment_key)
      {
        push_toml_comment(&mut out, comment);
      }
    }
    out.push_str(line);
    out.push('\n');
  }
  out
}

fn push_toml_comment(out: &mut String, comment: &str) {
  for line in comment.lines() {
    let _ = writeln!(out, "# {line}");
  }
}

/// Table name of a `[table]` or `[[array.of.tables]]` header line.
fn toml_table_header(line: &str) -> Option<&str> {
  match line.strip_prefix("[[") {
    Some(rest) => rest.strip_suffix("]]"),
    None => line.strip_prefix('[')?.strip_suffix(']'),
  }
}

fn toml_field_key(line: &str) -> Option<&str> {
  if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
    return None;
  }
  let (key, _) = line.split_once('=')?;
  let key = key.trim();
  (!key.is_empty()).then_some(key)
}
