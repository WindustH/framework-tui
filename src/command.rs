use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub enum Prompt {
  Text {
    prefix: String,
    buffer: PromptBuffer,
  },
  Command {
    buffer: PromptBuffer,
  },
}

#[derive(Debug, Clone)]
pub struct PromptBuffer {
  pub input: String,
  pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct CommandCompletion {
  pub replace_start: usize,
  pub replace_end: usize,
  pub prefix: String,
  pub candidates: Vec<String>,
  pub append_space: bool,
  pub selected: usize,
}

#[derive(Debug, Clone, Default)]
pub struct CommandHistoryCursor {
  index: Option<usize>,
  draft: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CommandState {
  history: Vec<String>,
  history_cursor: CommandHistoryCursor,
  completion: Option<CommandCompletion>,
}

impl PromptBuffer {
  pub fn new(input: impl Into<String>) -> Self {
    let input = input.into();
    let cursor = input.len();
    Self { input, cursor }
  }

  pub fn set_input(&mut self, input: String) {
    self.input = input;
    self.cursor = self.input.len();
  }

  pub fn insert_char(&mut self, ch: char) {
    self.input.insert(self.cursor, ch);
    self.cursor += ch.len_utf8();
  }

  pub fn insert_str(&mut self, value: &str) {
    let value = sanitize_inline_input(value);
    if value.is_empty() {
      return;
    }
    self.input.insert_str(self.cursor, &value);
    self.cursor += value.len();
  }

  pub fn backspace(&mut self) {
    if self.cursor == 0 {
      return;
    }
    let previous = previous_boundary(&self.input, self.cursor);
    self.input.drain(previous..self.cursor);
    self.cursor = previous;
  }

  pub fn delete(&mut self) {
    if self.cursor >= self.input.len() {
      return;
    }
    let next = next_boundary(&self.input, self.cursor);
    self.input.drain(self.cursor..next);
  }

  pub fn move_left(&mut self) {
    self.cursor = previous_boundary(&self.input, self.cursor);
  }

  pub fn move_right(&mut self) {
    self.cursor = next_boundary(&self.input, self.cursor);
  }

  pub fn move_start(&mut self) {
    self.cursor = 0;
  }

  pub fn move_end(&mut self) {
    self.cursor = self.input.len();
  }

  pub fn kill_before_cursor(&mut self) {
    self.input.drain(..self.cursor);
    self.cursor = 0;
  }

  pub fn kill_after_cursor(&mut self) {
    self.input.truncate(self.cursor);
  }

  pub fn cursor_columns(&self) -> usize {
    UnicodeWidthStr::width(&self.input[..self.cursor])
  }
}

impl Prompt {
  pub fn text(prefix: impl Into<String>, input: impl Into<String>) -> Self {
    Self::Text {
      prefix: prefix.into(),
      buffer: PromptBuffer::new(input),
    }
  }

  pub fn command(input: impl Into<String>) -> Self {
    Self::Command {
      buffer: PromptBuffer::new(input),
    }
  }

  pub fn buffer(&self) -> &PromptBuffer {
    match self {
      Prompt::Text { buffer, .. } | Prompt::Command { buffer } => buffer,
    }
  }

  pub fn buffer_mut(&mut self) -> &mut PromptBuffer {
    match self {
      Prompt::Text { buffer, .. } | Prompt::Command { buffer } => buffer,
    }
  }

  pub fn prefix(&self) -> &str {
    match self {
      Prompt::Text { prefix, .. } => prefix,
      Prompt::Command { .. } => ":",
    }
  }

  pub fn is_command(&self) -> bool {
    matches!(self, Prompt::Command { .. })
  }
}

impl CommandCompletion {
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

  pub fn suggestion_suffix(&self) -> String {
    let Some(candidate) = self.selected_candidate() else {
      return String::new();
    };
    candidate
      .chars()
      .skip(self.prefix.chars().count())
      .collect()
  }

  pub fn apply_to(&self, buffer: &mut PromptBuffer) -> bool {
    let Some(candidate) = self.selected_candidate() else {
      return false;
    };
    if self.replace_start > self.replace_end || self.replace_end > buffer.input.len() {
      return false;
    }
    let Some(prefix) = buffer.input.get(..self.replace_start) else {
      return false;
    };
    let Some(current) = buffer.input.get(self.replace_start..self.replace_end) else {
      return false;
    };
    let Some(suffix) = buffer.input.get(self.replace_end..) else {
      return false;
    };

    let mut next = prefix.to_string();
    next.push_str(candidate);
    if self.append_space && !next.ends_with(' ') {
      next.push(' ');
    }
    let next_cursor = next.len();
    next.push_str(suffix);

    if next == buffer.input || (current == candidate && !self.append_space) {
      return false;
    }
    buffer.input = next;
    buffer.cursor = next_cursor.min(buffer.input.len());
    true
  }
}

impl CommandHistoryCursor {
  pub fn reset(&mut self) {
    self.index = None;
    self.draft = None;
  }

  pub fn previous(&mut self, history: &[String], buffer: &mut PromptBuffer) {
    if history.is_empty() {
      return;
    }
    let index = match self.index {
      Some(0) => 0,
      Some(index) => index.saturating_sub(1),
      None => {
        self.draft = Some(buffer.input.clone());
        history.len() - 1
      }
    };
    self.index = Some(index);
    buffer.set_input(history[index].clone());
  }

  pub fn next(&mut self, history: &[String], buffer: &mut PromptBuffer) {
    let Some(index) = self.index else {
      return;
    };
    if index + 1 < history.len() {
      let next = index + 1;
      self.index = Some(next);
      buffer.set_input(history[next].clone());
    } else {
      self.index = None;
      let draft = self.draft.take().unwrap_or_default();
      buffer.set_input(draft);
    }
  }
}

impl CommandState {
  pub fn history(&self) -> &[String] {
    &self.history
  }

  pub fn completion(&self) -> Option<&CommandCompletion> {
    self.completion.as_ref()
  }

  pub fn completion_mut(&mut self) -> Option<&mut CommandCompletion> {
    self.completion.as_mut()
  }

  pub fn clear_completion(&mut self) {
    self.completion = None;
  }

  pub fn reset_history_cursor(&mut self) {
    self.history_cursor.reset();
  }

  pub fn reset_prompt_state(&mut self) {
    self.reset_history_cursor();
    self.clear_completion();
  }

  pub fn push_history(&mut self, command: impl Into<String>) {
    let command = command.into();
    if !command.is_empty() && self.history.last() != Some(&command) {
      self.history.push(command);
    }
    self.reset_history_cursor();
  }

  pub fn history_previous(&mut self, buffer: &mut PromptBuffer) {
    self.history_cursor.previous(&self.history, buffer);
  }

  pub fn history_next(&mut self, buffer: &mut PromptBuffer) {
    self.history_cursor.next(&self.history, buffer);
  }

  pub fn set_completion_preserving_selection(&mut self, mut completion: Option<CommandCompletion>) {
    let previous = self.completion.as_ref();
    if let (Some(previous), Some(completion)) = (previous, completion.as_mut())
      && let Some(candidate) = previous.selected_candidate()
      && let Some(index) = completion
        .candidates
        .iter()
        .position(|value| value == candidate)
    {
      completion.selected = index;
    }
    self.completion = completion.filter(|completion| !completion.candidates.is_empty());
  }

  pub fn select_next_completion(&mut self) {
    let Some(completion) = self.completion.as_mut() else {
      return;
    };
    if completion.candidates.is_empty() {
      return;
    }
    completion.selected = (completion.selected + 1) % completion.candidates.len();
  }

  pub fn select_previous_completion(&mut self) {
    let Some(completion) = self.completion.as_mut() else {
      return;
    };
    if completion.candidates.is_empty() {
      return;
    }
    completion.selected =
      (completion.selected + completion.candidates.len() - 1) % completion.candidates.len();
  }

  pub fn apply_completion(&mut self, buffer: &mut PromptBuffer) -> bool {
    let changed = self
      .completion
      .as_ref()
      .map(|completion| completion.apply_to(buffer))
      .unwrap_or(false);
    if changed {
      self.reset_history_cursor();
    }
    changed
  }
}

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

pub fn filter_completion_candidates<I, S>(candidates: I, prefix: &str) -> Vec<String>
where
  I: IntoIterator<Item = S>,
  S: AsRef<str>,
{
  let normalized_prefix = prefix.trim_start_matches(':').to_ascii_lowercase();
  let mut out = candidates
    .into_iter()
    .filter_map(|candidate| {
      let candidate = candidate.as_ref();
      candidate
        .to_ascii_lowercase()
        .starts_with(&normalized_prefix)
        .then(|| candidate.to_string())
    })
    .collect::<Vec<_>>();
  out.sort();
  out.dedup();
  out
}

fn previous_boundary(input: &str, cursor: usize) -> usize {
  let cursor = cursor.min(input.len());
  input
    .get(..cursor)
    .and_then(|prefix| prefix.char_indices().last().map(|(idx, _)| idx))
    .unwrap_or(0)
}

fn next_boundary(input: &str, cursor: usize) -> usize {
  let cursor = cursor.min(input.len());
  input
    .get(cursor..)
    .and_then(|suffix| suffix.char_indices().nth(1).map(|(idx, _)| cursor + idx))
    .unwrap_or(input.len())
}

fn sanitize_inline_input(value: &str) -> String {
  value
    .chars()
    .map(|ch| match ch {
      '\r' | '\n' => ' ',
      ch => ch,
    })
    .collect()
}
