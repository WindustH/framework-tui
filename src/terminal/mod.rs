//! Owning the terminal, without an async runtime.
//!
//! - [`TerminalSession`] enters raw mode and the alternate screen on stdout
//!   or stderr ([`TerminalOptions`]), suspends and resumes around other
//!   programs, and gives the terminal back on [`restore`], on drop and when
//!   setup fails.
//! - [`install_panic_hook`] restores the terminal before a panic on the
//!   session's thread is printed and reports panics on other threads to a
//!   callback instead of printing over the UI. [`emergency_restore`] does
//!   the restore from anywhere.
//! - [`InputReader`] reads terminal events on a background thread and
//!   passes them to a sink, such as a channel sender. It can be paused so a
//!   child process receives every keystroke.
//! - [`run_outside_tui`] and [`edit_text_outside_tui`] hand the terminal to
//!   another program and take it back: pause the reader, suspend, run,
//!   resume, drop the keys typed meanwhile, restart the reader.
//! - [`watch_termination_signals`] calls back on SIGTERM and SIGHUP.
//!
//! The pieces work on their own: a synchronous app can read events itself
//! and use only the session and the panic hook.
//!
//! ```no_run
//! use framework_tui::{
//!   PanicOrigin, TerminalOptions, TerminalOutput, TerminalSession, install_panic_hook,
//! };
//!
//! install_panic_hook(|info, origin| {
//!   if origin == PanicOrigin::Background {
//!     // log `info`; it is not printed while the UI is up
//!   }
//! });
//! let mut session = TerminalSession::enter(TerminalOptions {
//!   output: TerminalOutput::stdout_if_terminal(),
//!   ..TerminalOptions::default()
//! })?;
//! session.draw(|frame| { /* ... */ })?;
//! session.restore()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! [`restore`]: TerminalSession::restore

mod handoff;
mod reader;
mod session;
mod signals;
mod state;

pub use handoff::{HandOff, edit_text_outside_tui, run_outside_tui};
pub use reader::{InputEvent, InputReader, discard_pending_events};
pub use session::{
  SessionBackend, SuspendTerminal, TerminalOptions, TerminalOutput, TerminalSession,
};
pub use signals::{TerminationSignal, watch_termination_signals};
pub use state::{PanicOrigin, emergency_restore, install_panic_hook};
