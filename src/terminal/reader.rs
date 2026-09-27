//! Reading terminal input on a background thread that steps aside while
//! another program owns the terminal.

use std::{
  io,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use crossterm::event::{self, Event};

/// How long one poll may block before the reader looks at its gate again;
/// bounds how long [`InputReader::pause`] waits.
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Sleep between gate checks while paused.
const PARKED_SLEEP: Duration = Duration::from_millis(10);
/// Longest [`InputReader::pause`] waits for the reader to park.
const PAUSE_TIMEOUT: Duration = Duration::from_millis(300);
const MIN_ERROR_BACKOFF: Duration = Duration::from_millis(10);
const MAX_ERROR_BACKOFF: Duration = Duration::from_secs(1);
/// Upper bound for [`discard_pending_events`], so a steady stream of
/// events cannot keep it busy.
const MAX_DISCARDED_EVENTS: usize = 1024;

/// A terminal event and the reader generation current when it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputEvent {
  pub event: Event,
  pub generation: u64,
}

/// A thread that reads terminal events and passes each one to a sink.
///
/// The sink runs on the reader thread and returns whether to keep reading;
/// it typically forwards into a channel:
///
/// ```no_run
/// use framework_tui::{InputEvent, InputReader};
///
/// let (tx, rx) = std::sync::mpsc::channel::<InputEvent>();
/// let input = InputReader::spawn(move |event| tx.send(event).is_ok())?;
/// for event in rx {
///   if input.is_current(event.generation) {
///     // handle event.event
///   }
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// A tokio app forwards the same way into an unbounded `mpsc` sender, and
/// can drop events it never wants (such as bare mouse motion) in the sink
/// before they wake its event loop.
///
/// Before another program takes the terminal, [`pause`](Self::pause) stops
/// the reader and waits until it no longer polls, so the program receives
/// every keystroke; [`resume`](Self::resume) starts it again.
/// [`run_outside_tui`](super::run_outside_tui) does both. Each pause and
/// resume starts a new *generation*: events carry the generation they were
/// read in, and ones read before a hand-off fail
/// [`is_current`](Self::is_current).
///
/// The thread ends when the sink returns `false` or the reader is dropped.
pub struct InputReader {
  shared: Arc<Shared>,
}

struct Shared {
  enabled: AtomicBool,
  /// Set by the reader once it has seen `enabled == false` and will not
  /// touch the terminal until enabled again (or for good, once it ends).
  parked: AtomicBool,
  stopped: AtomicBool,
  generation: AtomicU64,
}

/// Where the reader gets events: crossterm, or a fake in tests.
trait EventSource: Send + 'static {
  fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
  fn read(&mut self) -> io::Result<Event>;
}

struct Crossterm;

impl EventSource for Crossterm {
  fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
    event::poll(timeout)
  }

  fn read(&mut self) -> io::Result<Event> {
    event::read()
  }
}

impl InputReader {
  /// Start reading on a thread named `terminal-input`.
  pub fn spawn<F>(sink: F) -> io::Result<Self>
  where
    F: FnMut(InputEvent) -> bool + Send + 'static,
  {
    Self::spawn_named("terminal-input", sink)
  }

  /// Start reading on a thread with the given name.
  pub fn spawn_named<F>(name: impl Into<String>, sink: F) -> io::Result<Self>
  where
    F: FnMut(InputEvent) -> bool + Send + 'static,
  {
    Self::spawn_with(name.into(), Crossterm, sink)
  }

  fn spawn_with<S, F>(name: String, source: S, sink: F) -> io::Result<Self>
  where
    S: EventSource,
    F: FnMut(InputEvent) -> bool + Send + 'static,
  {
    let shared = Arc::new(Shared {
      enabled: AtomicBool::new(true),
      parked: AtomicBool::new(false),
      stopped: AtomicBool::new(false),
      generation: AtomicU64::new(0),
    });
    let reader = shared.clone();
    thread::Builder::new()
      .name(name)
      .spawn(move || read_loop(&reader, source, sink))?;
    Ok(Self { shared })
  }

  /// The current generation.
  pub fn generation(&self) -> u64 {
    self.shared.generation.load(Ordering::SeqCst)
  }

  /// Whether an event of `generation` is still current, i.e. was not read
  /// before the latest pause or resume.
  pub fn is_current(&self, generation: u64) -> bool {
    generation == self.generation()
  }

  pub fn is_paused(&self) -> bool {
    !self.shared.enabled.load(Ordering::SeqCst)
  }

  /// Stop reading and wait until the reader has parked, so it no longer
  /// polls the terminal. Events already delivered become stale. Returns
  /// `false` if the reader did not park within 300 ms (for example because
  /// the sink is blocked); it stays paused either way.
  pub fn pause(&self) -> bool {
    self.shared.enabled.store(false, Ordering::SeqCst);
    self.shared.generation.fetch_add(1, Ordering::SeqCst);
    let deadline = Instant::now() + PAUSE_TIMEOUT;
    loop {
      if self.shared.parked.load(Ordering::SeqCst) {
        return true;
      }
      if Instant::now() >= deadline {
        return false;
      }
      thread::sleep(Duration::from_millis(1));
    }
  }

  /// Start reading again after [`pause`](Self::pause), in a new
  /// generation. Input that arrived meanwhile is still queued; see
  /// [`discard_pending_events`].
  pub fn resume(&self) {
    self.shared.generation.fetch_add(1, Ordering::SeqCst);
    self.shared.enabled.store(true, Ordering::SeqCst);
  }
}

impl Drop for InputReader {
  /// Ends the thread within one poll interval; does not wait for it.
  fn drop(&mut self) {
    self.shared.stopped.store(true, Ordering::SeqCst);
  }
}

fn read_loop(
  shared: &Shared,
  mut source: impl EventSource,
  mut sink: impl FnMut(InputEvent) -> bool,
) {
  let mut backoff = MIN_ERROR_BACKOFF;
  while !shared.stopped.load(Ordering::SeqCst) {
    // Clear `parked` before checking the gate: `pause` takes a set flag as
    // proof that this check, which comes next, sees the pause.
    shared.parked.store(false, Ordering::SeqCst);
    if !shared.enabled.load(Ordering::SeqCst) {
      shared.parked.store(true, Ordering::SeqCst);
      thread::sleep(PARKED_SLEEP);
      continue;
    }
    // The generation from before the wait: an event read while a pause
    // begins counts as stale.
    let generation = shared.generation.load(Ordering::SeqCst);
    // Poll with a timeout instead of blocking in `read`, so a pause takes
    // effect without waiting for the next key.
    let ready = match source.poll(POLL_INTERVAL) {
      Ok(ready) => ready,
      Err(_) => {
        back_off(&mut backoff);
        continue;
      }
    };
    // A pause began while polling: leave the event queued for the
    // hand-off to discard.
    if !ready || !shared.enabled.load(Ordering::SeqCst) {
      continue;
    }
    match source.read() {
      Ok(event) => {
        backoff = MIN_ERROR_BACKOFF;
        if !sink(InputEvent { event, generation }) {
          break;
        }
      }
      Err(_) => back_off(&mut backoff),
    }
  }
  shared.parked.store(true, Ordering::SeqCst);
}

/// Wait after a failed poll or read. A vanished terminal fails every call;
/// waiting longer each time keeps the reader from spinning.
fn back_off(backoff: &mut Duration) {
  thread::sleep(*backoff);
  *backoff = (*backoff * 2).min(MAX_ERROR_BACKOFF);
}

/// Drop terminal events that are already queued, such as keys typed into
/// another program after it stopped reading. Returns how many were dropped.
///
/// Call it only while no [`InputReader`] is reading (paused, or none).
pub fn discard_pending_events() -> usize {
  let mut discarded = 0;
  while discarded < MAX_DISCARDED_EVENTS && matches!(event::poll(Duration::ZERO), Ok(true)) {
    if event::read().is_err() {
      break;
    }
    discarded += 1;
  }
  discarded
}

#[cfg(test)]
mod tests {
  use std::{
    collections::VecDeque,
    sync::{Mutex, atomic::AtomicUsize, mpsc},
  };

  use crossterm::event::{KeyCode, KeyEvent};

  use super::*;

  /// Events pushed by the test. While `foreign` is set another program
  /// "owns the terminal", and any poll or read counts as a stolen key.
  #[derive(Clone, Default)]
  struct Fake {
    queue: Arc<Mutex<VecDeque<Event>>>,
    polls: Arc<AtomicUsize>,
    foreign: Arc<AtomicBool>,
    stolen: Arc<AtomicUsize>,
  }

  impl Fake {
    fn push(&self, ch: char) {
      let event = Event::Key(KeyEvent::from(KeyCode::Char(ch)));
      self.queue.lock().unwrap().push_back(event);
    }

    fn polls(&self) -> usize {
      self.polls.load(Ordering::SeqCst)
    }
  }

  impl EventSource for Fake {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
      self.polls.fetch_add(1, Ordering::SeqCst);
      if self.foreign.load(Ordering::SeqCst) {
        self.stolen.fetch_add(1, Ordering::SeqCst);
      }
      if !self.queue.lock().unwrap().is_empty() {
        return Ok(true);
      }
      thread::sleep(timeout.min(Duration::from_millis(2)));
      Ok(false)
    }

    fn read(&mut self) -> io::Result<Event> {
      if self.foreign.load(Ordering::SeqCst) {
        self.stolen.fetch_add(1, Ordering::SeqCst);
      }
      self
        .queue
        .lock()
        .unwrap()
        .pop_front()
        .ok_or_else(|| io::Error::other("no event"))
    }
  }

  fn spawn(fake: &Fake) -> (InputReader, mpsc::Receiver<InputEvent>) {
    let (tx, rx) = mpsc::channel();
    let reader = InputReader::spawn_with("test-input".to_string(), fake.clone(), move |event| {
      tx.send(event).is_ok()
    })
    .unwrap();
    (reader, rx)
  }

  fn key(event: &InputEvent) -> char {
    match event.event {
      Event::Key(KeyEvent {
        code: KeyCode::Char(ch),
        ..
      }) => ch,
      _ => panic!("not a character key: {event:?}"),
    }
  }

  const WAIT: Duration = Duration::from_secs(5);

  #[test]
  fn delivers_events_in_order_with_their_generation() {
    let fake = Fake::default();
    let (reader, rx) = spawn(&fake);
    fake.push('a');
    fake.push('b');
    let first = rx.recv_timeout(WAIT).unwrap();
    let second = rx.recv_timeout(WAIT).unwrap();
    assert_eq!((key(&first), key(&second)), ('a', 'b'));
    assert_eq!(first.generation, 0);
    assert!(reader.is_current(first.generation));
  }

  #[test]
  fn paused_reader_leaves_the_terminal_alone() {
    let fake = Fake::default();
    let (reader, rx) = spawn(&fake);
    fake.push('a');
    let before = rx.recv_timeout(WAIT).unwrap();

    assert!(reader.pause());
    assert!(reader.is_paused());
    assert!(!reader.is_current(before.generation));
    // Another program runs now; the reader must not poll or read.
    fake.foreign.store(true, Ordering::SeqCst);
    let polls = fake.polls();
    fake.push('x');
    thread::sleep(POLL_INTERVAL * 3);
    assert_eq!(fake.stolen.load(Ordering::SeqCst), 0);
    assert_eq!(fake.polls(), polls);
    assert!(rx.try_recv().is_err());
    fake.foreign.store(false, Ordering::SeqCst);

    reader.resume();
    assert!(!reader.is_paused());
    let after = rx.recv_timeout(WAIT).unwrap();
    assert_eq!(key(&after), 'x');
    assert_eq!(after.generation, 2);
    assert!(reader.is_current(after.generation));
  }

  #[test]
  fn repeated_hand_offs_never_poll_while_paused() {
    let fake = Fake::default();
    let (reader, rx) = spawn(&fake);
    for round in 0..20 {
      assert!(reader.pause(), "round {round}");
      fake.foreign.store(true, Ordering::SeqCst);
      thread::sleep(Duration::from_millis(3));
      fake.foreign.store(false, Ordering::SeqCst);
      reader.resume();
      // Resume and pause again right away, possibly before the reader
      // has noticed the resume.
    }
    assert_eq!(fake.stolen.load(Ordering::SeqCst), 0);
    fake.push('z');
    let event = rx.recv_timeout(WAIT).unwrap();
    assert_eq!(key(&event), 'z');
    assert!(reader.is_current(event.generation));
  }

  #[test]
  fn sink_returning_false_ends_the_reader() {
    let fake = Fake::default();
    let (tx, rx) = mpsc::channel();
    let reader = InputReader::spawn_with("test-input".to_string(), fake.clone(), move |event| {
      let _ = tx.send(event);
      false
    })
    .unwrap();
    fake.push('a');
    fake.push('b');
    assert_eq!(key(&rx.recv_timeout(WAIT).unwrap()), 'a');
    // The ended reader counts as parked at once.
    let started = Instant::now();
    assert!(reader.pause());
    assert!(started.elapsed() < PAUSE_TIMEOUT);
    thread::sleep(POLL_INTERVAL);
    assert!(rx.try_recv().is_err());
  }

  #[test]
  fn dropping_the_reader_ends_the_thread() {
    let fake = Fake::default();
    let (reader, _rx) = spawn(&fake);
    thread::sleep(Duration::from_millis(10));
    drop(reader);
    thread::sleep(POLL_INTERVAL * 2);
    let polls = fake.polls();
    thread::sleep(POLL_INTERVAL * 2);
    assert_eq!(fake.polls(), polls);
  }
}
