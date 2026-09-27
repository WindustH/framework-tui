//! What the process knows about the session that owns the terminal, for the
//! code paths that cannot reach the [`TerminalSession`]: the panic hook and
//! [`emergency_restore`].

use std::{
  io::{self, Write},
  panic::{self, PanicHookInfo},
  sync::{
    Mutex, Once, PoisonError,
    atomic::{AtomicU8, Ordering},
  },
  thread::{self, ThreadId},
};

use crossterm::{cursor::Show, queue, terminal::disable_raw_mode};

#[cfg(doc)]
use super::session::TerminalSession;
use super::session::{TerminalOptions, TerminalOutput, queue_leave};

const ACTIVE: u8 = 1;
const SUSPENDED: u8 = 1 << 1;
const STDOUT: u8 = 1 << 2;
const MOUSE: u8 = 1 << 3;
const PASTE: u8 = 1 << 4;

static STATE: AtomicU8 = AtomicU8::new(0);
/// The thread that entered the active session.
static OWNER: Mutex<Option<ThreadId>> = Mutex::new(None);

pub(super) fn activate(options: &TerminalOptions) {
  *OWNER.lock().unwrap_or_else(PoisonError::into_inner) = Some(thread::current().id());
  let mut state = ACTIVE;
  if options.output == TerminalOutput::Stdout {
    state |= STDOUT;
  }
  if options.mouse_capture {
    state |= MOUSE;
  }
  if options.bracketed_paste {
    state |= PASTE;
  }
  STATE.store(state, Ordering::SeqCst);
}

pub(super) fn deactivate() {
  STATE.store(0, Ordering::SeqCst);
  *OWNER.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

pub(super) fn set_suspended(suspended: bool) {
  if suspended {
    STATE.fetch_or(SUSPENDED, Ordering::SeqCst);
  } else {
    STATE.fetch_and(!SUSPENDED, Ordering::SeqCst);
  }
}

fn session_active() -> bool {
  STATE.load(Ordering::SeqCst) & ACTIVE != 0
}

fn owns_session(thread: ThreadId) -> bool {
  *OWNER.lock().unwrap_or_else(PoisonError::into_inner) == Some(thread)
}

/// Put the terminal back into its normal state without access to the
/// [`TerminalSession`]: leave raw mode, the alternate screen, mouse capture
/// and bracketed paste (as far as the session enabled them) and show the
/// cursor. Returns whether a session was active.
///
/// Meant for paths that cannot reach the session, such as the panic hook or
/// a termination-signal handler ([`watch_termination_signals`]) that is
/// about to exit the process. The session no longer counts as active
/// afterwards, so a second call does nothing; while the session is
/// suspended (another program owns the terminal) nothing is written.
///
/// It writes straight to the session's output stream and takes the same
/// locks as normal terminal I/O, so it is not async-signal-safe: call it
/// from a thread or task that handles signals, never from inside a raw
/// `sigaction` handler.
///
/// [`watch_termination_signals`]: super::watch_termination_signals
pub fn emergency_restore() -> bool {
  let state = STATE.swap(0, Ordering::SeqCst);
  if state & ACTIVE == 0 {
    return false;
  }
  if state & SUSPENDED == 0 {
    let _ = disable_raw_mode();
    let mut output = if state & STDOUT != 0 {
      TerminalOutput::Stdout
    } else {
      TerminalOutput::Stderr
    };
    let _ = write_emergency_leave(&mut output, state & MOUSE != 0, state & PASTE != 0);
  }
  true
}

fn write_emergency_leave(output: &mut impl Write, mouse: bool, paste: bool) -> io::Result<()> {
  queue_leave(output, mouse, paste)?;
  queue!(output, Show)?;
  output.flush()
}

/// Which thread a panic happened on, for the report callback of
/// [`install_panic_hook`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanicOrigin {
  /// The thread that entered the [`TerminalSession`] (normally `main`).
  /// After the report the terminal is restored and the panic message is
  /// printed as usual.
  SessionThread,
  /// Any other thread. The message is not printed, because the terminal
  /// still shows the UI; the report is its only record.
  Background,
}

/// Install a panic hook that keeps panic messages readable while a
/// [`TerminalSession`] owns the terminal.
///
/// While a session is active, every panic is first passed to `report` (for
/// example to log it). A panic on the session's thread then restores the
/// terminal ([`emergency_restore`]) and prints the message through the
/// previously installed hook; otherwise the message would land on the
/// alternate screen and vanish with it. A panic on any other thread is
/// only reported: printing it would scribble over the running UI. With no
/// active session every panic goes straight to the previous hook.
///
/// Only the first call installs the hook; later calls do nothing.
pub fn install_panic_hook<F>(report: F)
where
  F: Fn(&PanicHookInfo<'_>, PanicOrigin) + Send + Sync + 'static,
{
  static INSTALL: Once = Once::new();
  INSTALL.call_once(move || {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
      if !session_active() {
        previous(info);
        return;
      }
      if owns_session(thread::current().id()) {
        report(info, PanicOrigin::SessionThread);
        emergency_restore();
        previous(info);
      } else {
        report(info, PanicOrigin::Background);
      }
    }));
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn emergency_leave_undoes_only_enabled_modes() {
    let mut all = Vec::new();
    write_emergency_leave(&mut all, true, true).unwrap();
    let all = String::from_utf8(all).unwrap();
    assert!(all.starts_with("\x1b[?1049l"), "{all:?}");
    assert!(all.contains("\x1b[?1000l"), "{all:?}");
    assert!(all.contains("\x1b[?2004l"), "{all:?}");
    assert!(all.ends_with("\x1b[?25h"), "{all:?}");

    let mut plain = Vec::new();
    write_emergency_leave(&mut plain, false, false).unwrap();
    assert_eq!(String::from_utf8(plain).unwrap(), "\x1b[?1049l\x1b[?25h");
  }
}
