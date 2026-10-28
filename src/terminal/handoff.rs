//! Handing the terminal to another program, such as `$EDITOR`, and taking
//! it back.

use std::{fmt::Display, path::Path};

use super::{InputReader, SuspendTerminal, discard_pending_events};
use crate::editor::{EditorOptions, edit_text_in_editor_with_options};

/// What happened while another program had the terminal.
#[derive(Debug)]
#[must_use]
pub struct HandOff<R, E> {
  /// What the program produced.
  pub output: R,
  /// The first error from suspending or resuming the terminal. After a
  /// failed resume the screen may be unusable, so apps usually return it.
  pub terminal: Result<(), E>,
}

/// Run `run` while it owns the terminal.
///
/// In order: pause `input` (so it cannot steal the program's keystrokes),
/// suspend `terminal`, call `run`, resume `terminal`, drop the input that
/// was typed meanwhile (only if the resume worked) and restart `input` in a
/// new generation. `run` is skipped when the terminal cannot be suspended;
/// the output is then `None`. The terminal is resumed in every case.
pub fn run_outside_tui<T, R>(
  terminal: &mut T,
  input: Option<&InputReader>,
  run: impl FnOnce() -> R,
) -> HandOff<Option<R>, T::Error>
where
  T: SuspendTerminal + ?Sized,
{
  if let Some(input) = input {
    input.pause();
  }
  let suspended = terminal.suspend();
  let output = suspended.is_ok().then(run);
  let resumed = terminal.resume();
  if resumed.is_ok() {
    discard_pending_events();
  }
  if let Some(input) = input {
    input.resume();
  }
  HandOff {
    output,
    terminal: suspended.and(resumed),
  }
}

/// Edit `initial` in the user's editor (see
/// [`edit_text_in_editor_with_options`]) with the terminal handed over as
/// by [`run_outside_tui`].
///
/// The output is the edited text, or why editing failed, including a
/// terminal that could not be suspended.
pub fn edit_text_outside_tui<T>(
  terminal: &mut T,
  input: Option<&InputReader>,
  initial: &str,
  cache_dir: &Path,
  options: &EditorOptions,
) -> HandOff<Result<String, String>, T::Error>
where
  T: SuspendTerminal + ?Sized,
  T::Error: Display,
{
  let handoff = run_outside_tui(terminal, input, || {
    edit_text_in_editor_with_options(initial, cache_dir, options)
  });
  let output = match (handoff.output, &handoff.terminal) {
    (Some(edited), _) => edited,
    (None, Err(error)) => Err(format!(
      "could not hand the terminal to the editor: {error}"
    )),
    (None, Ok(())) => Err("could not hand the terminal to the editor".to_string()),
  };
  HandOff {
    output,
    terminal: handoff.terminal,
  }
}
