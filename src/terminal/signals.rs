//! Termination signals (SIGTERM, SIGHUP) without an async runtime.

use std::io;

/// A signal asking the process to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationSignal {
  /// SIGTERM, e.g. from `kill`.
  Terminate,
  /// SIGHUP: the terminal went away.
  Hangup,
}

/// Call `handler` on a background thread each time the process receives
/// SIGTERM or SIGHUP, instead of being killed with the terminal still in
/// raw mode.
///
/// The handler takes over from the default action, so it has to end the
/// app: send a quit event to the event loop (which then restores the
/// terminal as usual), or call [`emergency_restore`](super::emergency_restore)
/// and exit. On platforms without these signals (Windows) this does
/// nothing.
pub fn watch_termination_signals<F>(handler: F) -> io::Result<()>
where
  F: FnMut(TerminationSignal) + Send + 'static,
{
  imp::watch(handler)
}

#[cfg(unix)]
mod imp {
  use std::{io, thread};

  use signal_hook::{
    consts::{SIGHUP, SIGTERM},
    iterator::Signals,
  };

  use super::TerminationSignal;

  pub(super) fn watch<F>(mut handler: F) -> io::Result<()>
  where
    F: FnMut(TerminationSignal) + Send + 'static,
  {
    let mut signals = Signals::new([SIGTERM, SIGHUP])?;
    thread::Builder::new()
      .name("termination-signals".to_string())
      .spawn(move || {
        for signal in signals.forever() {
          handler(if signal == SIGHUP {
            TerminationSignal::Hangup
          } else {
            TerminationSignal::Terminate
          });
        }
      })?;
    Ok(())
  }
}

#[cfg(not(unix))]
mod imp {
  use std::io;

  use super::TerminationSignal;

  pub(super) fn watch<F>(_handler: F) -> io::Result<()>
  where
    F: FnMut(TerminationSignal) + Send + 'static,
  {
    Ok(())
  }
}

#[cfg(all(test, unix))]
mod tests {
  use std::{sync::mpsc, time::Duration};

  use super::*;

  #[test]
  fn hangup_reaches_the_handler() {
    let (tx, rx) = mpsc::channel();
    watch_termination_signals(move |signal| {
      let _ = tx.send(signal);
    })
    .unwrap();
    signal_hook::low_level::raise(signal_hook::consts::SIGHUP).unwrap();
    assert_eq!(
      rx.recv_timeout(Duration::from_secs(5)),
      Ok(TerminationSignal::Hangup)
    );
  }
}
