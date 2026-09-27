//! Owning the terminal: raw mode and the alternate screen on a chosen output
//! stream, suspended around other programs and restored at the end.

use std::{
  io::{self, BufWriter, IsTerminal, Write},
  mem, thread,
};

use crossterm::{
  event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
  queue,
  terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
  },
};
use ratatui::{CompletedFrame, Frame, Terminal, backend::CrosstermBackend};

use super::state;

/// The stream the UI is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalOutput {
  Stdout,
  Stderr,
}

impl TerminalOutput {
  /// Stdout when it is a terminal, otherwise stderr: with
  /// `app | xargs ...` or `$(app)` the UI still reaches the screen and
  /// the pipe receives only what the app prints itself.
  pub fn stdout_if_terminal() -> Self {
    if io::stdout().is_terminal() {
      Self::Stdout
    } else {
      Self::Stderr
    }
  }
}

impl Write for TerminalOutput {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    match self {
      Self::Stdout => io::stdout().write(buf),
      Self::Stderr => io::stderr().write(buf),
    }
  }

  fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
    match self {
      Self::Stdout => io::stdout().write_all(buf),
      Self::Stderr => io::stderr().write_all(buf),
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    match self {
      Self::Stdout => io::stdout().flush(),
      Self::Stderr => io::stderr().flush(),
    }
  }
}

/// How [`TerminalSession::enter`] sets up the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalOptions {
  /// Where the UI is drawn. Default: stderr, which keeps stdout free for
  /// the app's own output.
  pub output: TerminalOutput,
  /// Bytes buffered before a write reaches the stream. Every frame and
  /// every mode switch ends with a flush; `0` writes straight through.
  /// Default: 8 KiB.
  pub buffer_capacity: usize,
  /// Report mouse events. Default: on.
  pub mouse_capture: bool,
  /// Deliver pastes as one `Event::Paste`. Default: on.
  pub bracketed_paste: bool,
}

impl Default for TerminalOptions {
  fn default() -> Self {
    Self {
      output: TerminalOutput::Stderr,
      buffer_capacity: 8 * 1024,
      mouse_capture: true,
      bracketed_paste: true,
    }
  }
}

/// The ratatui backend of a [`TerminalSession`].
pub type SessionBackend = CrosstermBackend<BufWriter<TerminalOutput>>;

/// Something that can hand the terminal to another program and take it
/// back: a [`TerminalSession`], or an app type wrapping one that has extra
/// state to tear down and rebuild (such as terminal images).
pub trait SuspendTerminal {
  type Error;

  /// Give the terminal back to the shell: leave raw mode and the alternate
  /// screen. Does nothing while already suspended.
  fn suspend(&mut self) -> Result<(), Self::Error>;

  /// Take the terminal again after [`suspend`](Self::suspend) and make the
  /// next frame repaint everything. Does nothing unless suspended.
  fn resume(&mut self) -> Result<(), Self::Error>;
}

/// The terminal in TUI mode: raw mode, the alternate screen and optionally
/// mouse capture and bracketed paste, with a ratatui [`Terminal`] drawing
/// on the chosen output.
///
/// Dropping the session restores the terminal, except while unwinding from
/// a panic: then the panic hook ([`install_panic_hook`]) has restored it
/// already, and flushing a half-built frame would print over the panic
/// message, so the buffered output is discarded (the terminal is restored
/// here after all if no hook did it). Only one session should exist at a
/// time.
///
/// [`install_panic_hook`]: super::install_panic_hook
pub struct TerminalSession {
  /// `None` only once dropped during a panic.
  terminal: Option<Terminal<SessionBackend>>,
  options: TerminalOptions,
  suspended: bool,
  restored: bool,
}

impl TerminalSession {
  /// Enter TUI mode. If a step fails, the steps already taken are undone
  /// before the error is returned.
  pub fn enter(options: TerminalOptions) -> io::Result<Self> {
    enable_raw_mode()?;
    state::activate(&options);
    let setup = (|| {
      let mut writer = BufWriter::with_capacity(options.buffer_capacity, options.output);
      queue_enter(&mut writer, &options)?;
      writer.flush()?;
      Terminal::new(CrosstermBackend::new(writer))
    })();
    match setup {
      Ok(terminal) => Ok(Self {
        terminal: Some(terminal),
        options,
        suspended: false,
        restored: false,
      }),
      Err(error) => {
        let _ = disable_raw_mode();
        let mut output = options.output;
        let _ = queue_leave(&mut output, options.mouse_capture, options.bracketed_paste)
          .and_then(|()| output.flush());
        state::deactivate();
        Err(error)
      }
    }
  }

  pub fn options(&self) -> &TerminalOptions {
    &self.options
  }

  /// The ratatui terminal, e.g. for a renderer that draws through it.
  pub fn terminal_mut(&mut self) -> &mut Terminal<SessionBackend> {
    self
      .terminal
      .as_mut()
      .expect("the terminal lives until the session is dropped")
  }

  /// The backend, for writing escape sequences next to ratatui's output.
  pub fn backend_mut(&mut self) -> &mut SessionBackend {
    self.terminal_mut().backend_mut()
  }

  /// Draw a frame (see [`Terminal::draw`]).
  pub fn draw<F>(&mut self, render: F) -> io::Result<CompletedFrame<'_>>
  where
    F: FnOnce(&mut Frame),
  {
    self.terminal_mut().draw(render)
  }

  /// Clear the screen and repaint every cell on the next draw.
  ///
  /// Unlike [`Terminal::clear`] this does not ask the terminal for the
  /// cursor position, a query that goes to stdout: it would leak into piped
  /// output and time out when stdout is not the terminal.
  pub fn clear(&mut self) -> io::Result<()> {
    let terminal = self.terminal_mut();
    queue!(terminal.backend_mut(), Clear(ClearType::All))?;
    terminal.backend_mut().flush()?;
    // Both buffers blank: the next frame is diffed against an empty screen.
    terminal.current_buffer_mut().reset();
    terminal.swap_buffers();
    Ok(())
  }

  /// Whether another program has the terminal (see [`Self::suspend`]).
  pub fn is_suspended(&self) -> bool {
    self.suspended
  }

  /// Whether [`Self::restore`] has run.
  pub fn is_restored(&self) -> bool {
    self.restored
  }

  /// Hand the terminal to another program, such as an editor: leave raw
  /// mode and the alternate screen and show the cursor. Every step is
  /// tried even when an earlier one fails; the first error is returned,
  /// and the session counts as suspended either way.
  pub fn suspend(&mut self) -> io::Result<()> {
    if self.suspended || self.restored {
      return Ok(());
    }
    self.suspended = true;
    state::set_suspended(true);
    self.leave()
  }

  /// Take the terminal back after [`Self::suspend`]: re-enter raw mode and
  /// the alternate screen, clear it, and repaint everything on the next
  /// draw (see [`Self::clear`]).
  pub fn resume(&mut self) -> io::Result<()> {
    if !self.suspended || self.restored {
      return Ok(());
    }
    enable_raw_mode()?;
    // From here on the terminal is (partly) in TUI mode again, so
    // `restore` has to undo it.
    self.suspended = false;
    state::set_suspended(false);
    let options = self.options;
    let backend = self.backend_mut();
    queue_enter(backend, &options)?;
    self.clear()
  }

  /// Give the terminal back for good. Every step is tried even when an
  /// earlier one fails; the first error is returned. Later calls, and
  /// dropping the session, do nothing.
  pub fn restore(&mut self) -> io::Result<()> {
    if self.restored {
      return Ok(());
    }
    self.restored = true;
    let result = if self.suspended {
      Ok(())
    } else {
      self.suspended = true;
      self.leave()
    };
    state::deactivate();
    result
  }

  fn leave(&mut self) -> io::Result<()> {
    let options = self.options;
    let raw_mode = disable_raw_mode();
    let terminal = self.terminal_mut();
    let cursor = terminal.show_cursor();
    let backend = terminal.backend_mut();
    let screen = queue_leave(backend, options.mouse_capture, options.bracketed_paste)
      .and_then(|()| backend.flush());
    raw_mode.and(cursor).and(screen)
  }
}

impl SuspendTerminal for TerminalSession {
  type Error = io::Error;

  fn suspend(&mut self) -> io::Result<()> {
    TerminalSession::suspend(self)
  }

  fn resume(&mut self) -> io::Result<()> {
    TerminalSession::resume(self)
  }
}

impl Drop for TerminalSession {
  fn drop(&mut self) {
    if thread::panicking() {
      // Never flush what is still buffered (a half-built frame) over the
      // panic message; restore directly unless the panic hook did.
      mem::forget(self.terminal.take());
      super::emergency_restore();
      return;
    }
    let _ = self.restore();
  }
}

fn queue_enter(writer: &mut impl Write, options: &TerminalOptions) -> io::Result<()> {
  queue!(writer, EnterAlternateScreen)?;
  if options.mouse_capture {
    queue!(writer, EnableMouseCapture)?;
  }
  if options.bracketed_paste {
    queue!(writer, EnableBracketedPaste)?;
  }
  Ok(())
}

/// Leave the alternate screen and undo mouse capture and bracketed paste if
/// they were enabled; not flushed.
pub(super) fn queue_leave(writer: &mut impl Write, mouse: bool, paste: bool) -> io::Result<()> {
  queue!(writer, LeaveAlternateScreen)?;
  if mouse {
    queue!(writer, DisableMouseCapture)?;
  }
  if paste {
    queue!(writer, DisableBracketedPaste)?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn written(write: impl FnOnce(&mut Vec<u8>) -> io::Result<()>) -> String {
    let mut out = Vec::new();
    write(&mut out).unwrap();
    String::from_utf8(out).unwrap()
  }

  #[test]
  fn enter_and_leave_follow_the_options() {
    let all = TerminalOptions::default();
    assert_eq!(
      written(|out| queue_enter(out, &all)),
      "\x1b[?1049h\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h\x1b[?2004h"
    );
    assert_eq!(
      written(|out| queue_leave(out, true, true)),
      "\x1b[?1049l\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l\x1b[?2004l"
    );

    let plain = TerminalOptions {
      mouse_capture: false,
      bracketed_paste: false,
      ..all
    };
    assert_eq!(written(|out| queue_enter(out, &plain)), "\x1b[?1049h");
    assert_eq!(written(|out| queue_leave(out, false, false)), "\x1b[?1049l");
  }

  #[test]
  fn defaults_draw_on_stderr() {
    let options = TerminalOptions::default();
    assert_eq!(options.output, TerminalOutput::Stderr);
    assert!(options.mouse_capture && options.bracketed_paste);
  }
}
