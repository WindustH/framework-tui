//! Command-line completion: the candidate list for the word being typed and
//! the helpers apps use to build it.

use super::prompt::PromptBuffer;

/// Completion candidates for the byte range `replace_start..replace_end` of
/// the prompt input.
///
/// `prefix` is the text already typed for the word being completed; it
/// decides the inline suggestion shown after the cursor. `append_space`
/// adds a space after an accepted candidate.
#[derive(Debug, Clone)]
pub struct CommandCompletion {
  pub replace_start: usize,
  pub replace_end: usize,
  pub prefix: String,
  pub candidates: Vec<String>,
  pub append_space: bool,
  pub selected: usize,
}

impl CommandCompletion {
  /// Build a completion; `selected` is clamped to the candidate list.
  pub fn new(
    replace_start: usize,
    replace_end: usize,
    prefix: impl Into<String>,
    candidates: Vec<String>,
    append_space: bool,
    selected: usize,
  ) -> Self {
    let selected = selected.min(candidates.len().saturating_sub(1));
    Self {
      replace_start,
      replace_end,
      prefix: prefix.into(),
      candidates,
      append_space,
      selected,
    }
  }

  pub fn selected_candidate(&self) -> Option<&String> {
    self.candidates.get(self.selected)
  }

  /// The rest of the selected candidate after what has been typed, drawn
  /// as an inline suggestion. Empty when the candidate does not extend the
  /// typed prefix.
  pub fn suggestion_suffix(&self) -> String {
    self.suggestion().to_string()
  }

  /// Borrowed form of [`Self::suggestion_suffix`].
  ///
  /// Matching follows [`filter_completion_candidates`]: a leading `:` in
  /// the prefix is ignored and ASCII letters match in either case.
  pub(crate) fn suggestion(&self) -> &str {
    let Some(candidate) = self.selected_candidate() else {
      return "";
    };
    let typed = if candidate.starts_with(':') {
      self.prefix.as_str()
    } else {
      self.prefix.trim_start_matches(':')
    };
    match candidate.get(..typed.len()) {
      Some(head) if head.eq_ignore_ascii_case(typed) => &candidate[typed.len()..],
      _ => "",
    }
  }

  /// Replace the completed range of `buffer` with the selected candidate
  /// and put the cursor after it. Returns whether the buffer changed.
  pub fn apply_to(&self, buffer: &mut PromptBuffer) -> bool {
    let Some(candidate) = self.selected_candidate() else {
      return false;
    };
    let input = &buffer.input;
    let (Some(before), Some(current), Some(after)) = (
      input.get(..self.replace_start),
      input.get(self.replace_start..self.replace_end),
      input.get(self.replace_end..),
    ) else {
      return false;
    };
    if current == candidate && !self.append_space {
      return false;
    }

    let mut next = String::with_capacity(input.len() + candidate.len() + 1);
    next.push_str(before);
    next.push_str(candidate);
    if self.append_space && !next.ends_with(' ') {
      next.push(' ');
    }
    let next_cursor = next.len();
    next.push_str(after);

    if next == buffer.input {
      return false;
    }
    buffer.input = next;
    buffer.cursor = next_cursor;
    true
  }
}

/// Byte offset where the whitespace-separated word ending at `cursor`
/// starts.
pub fn current_word_start(input: &str, cursor: usize) -> usize {
  input
    .get(..cursor.min(input.len()))
    .unwrap_or_default()
    .char_indices()
    .rev()
    .find(|(_, ch)| ch.is_whitespace())
    .map(|(idx, ch)| idx + ch.len_utf8())
    .unwrap_or(0)
}

/// Candidates starting with `prefix`, sorted and deduplicated. A leading
/// `:` in the prefix is ignored and ASCII letters match in either case.
pub fn filter_completion_candidates<I, S>(candidates: I, prefix: &str) -> Vec<String>
where
  I: IntoIterator<Item = S>,
  S: AsRef<str>,
{
  let prefix = prefix.trim_start_matches(':');
  let mut out = candidates
    .into_iter()
    .filter_map(|candidate| {
      let candidate = candidate.as_ref();
      candidate
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        .then(|| candidate.to_string())
    })
    .collect::<Vec<_>>();
  out.sort();
  out.dedup();
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  fn completion(prefix: &str, candidates: &[&str]) -> CommandCompletion {
    CommandCompletion::new(
      0,
      prefix.len(),
      prefix,
      candidates.iter().map(|value| value.to_string()).collect(),
      true,
      0,
    )
  }

  #[test]
  fn suggestion_follows_the_filter_rules() {
    assert_eq!(completion("q", &["quit"]).suggestion_suffix(), "uit");
    assert_eq!(completion("Q", &["quit"]).suggestion_suffix(), "uit");
    // A typed ':' is not part of the candidate (this used to suggest "it").
    assert_eq!(completion(":q", &["quit"]).suggestion_suffix(), "uit");
    assert_eq!(completion("", &["quit"]).suggestion_suffix(), "quit");
    // Candidates that do not extend the prefix get no inline suggestion.
    assert_eq!(completion("ab", &["xyz"]).suggestion_suffix(), "");
    assert_eq!(completion("é", &["ée"]).suggestion_suffix(), "e");
    assert_eq!(completion("q", &[]).suggestion_suffix(), "");
  }

  #[test]
  fn filter_ignores_case_and_leading_colon() {
    assert_eq!(
      filter_completion_candidates(["Quit", "query", "open", "quit", "quit"], ":qu"),
      ["Quit", "query", "quit"]
    );
    assert_eq!(
      filter_completion_candidates(["日本", "日"], "日"),
      ["日", "日本"]
    );
    assert_eq!(
      filter_completion_candidates(["é"], "e"),
      Vec::<String>::new()
    );
  }

  #[test]
  fn apply_replaces_the_word_and_moves_the_cursor() {
    let mut buffer = PromptBuffer::new(":q");
    assert!(completion(":q", &["quit"]).apply_to(&mut buffer));
    assert_eq!((buffer.input.as_str(), buffer.cursor), ("quit ", 5));
    // Already applied: nothing changes.
    let mut applied = completion("quit", &["quit"]);
    applied.replace_end = 5;
    assert!(!applied.apply_to(&mut buffer));
    // Ranges that do not fit the buffer are rejected.
    let mut stale = completion("quit", &["quit"]);
    stale.replace_end = 40;
    assert!(!stale.apply_to(&mut PromptBuffer::new("q")));
  }
}
