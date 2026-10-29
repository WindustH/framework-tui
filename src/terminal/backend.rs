//! Crossterm output with optional, bounded batches of cell updates.

use std::io::{self, BufWriter, Write};

use ratatui::{
  backend::{Backend, ClearType, CrosstermBackend, WindowSize},
  buffer::Cell,
  layout::{Position, Size},
};

use super::session::TerminalOutput;

/// The terminal backend used by [`super::TerminalSession`].
/// Protocol integrations can delimit small batches of text updates without
/// inserting escape sequences inside UTF-8 characters or terminal commands.
pub struct SessionBackend {
  inner: CrosstermBackend<BufWriter<TerminalOutput>>,
  draw_batching: Option<(usize, &'static [u8])>,
}

impl SessionBackend {
  pub(super) fn new(writer: BufWriter<TerminalOutput>) -> Self {
    Self {
      inner: CrosstermBackend::new(writer),
      draw_batching: None,
    }
  }

  /// Prefix every batch of at most `cells` updates with a caller-supplied
  /// control sequence. `None` restores a single draw call per frame.
  /// The prefix must not change the cursor, text attributes, or screen cells.
  pub fn set_draw_batching(&mut self, batching: Option<(usize, &'static [u8])>) {
    self.draw_batching = batching.map(|(cells, prefix)| (cells.max(1), prefix));
  }
}

impl Write for SessionBackend {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.inner.write(buf)
  }

  fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
    self.inner.write_all(buf)
  }

  fn flush(&mut self) -> io::Result<()> {
    Write::flush(&mut self.inner)
  }
}

impl Backend for SessionBackend {
  type Error = io::Error;

  fn draw<'a, I>(&mut self, mut content: I) -> io::Result<()>
  where
    I: Iterator<Item = (u16, u16, &'a Cell)>,
  {
    let Some((cells, prefix)) = self.draw_batching else {
      return self.inner.draw(content);
    };
    let mut batch = Vec::with_capacity(cells);
    loop {
      batch.extend(content.by_ref().take(cells));
      if batch.is_empty() {
        return Ok(());
      }
      self.inner.write_all(prefix)?;
      self.inner.draw(batch.drain(..))?;
    }
  }

  fn append_lines(&mut self, n: u16) -> io::Result<()> {
    self.inner.append_lines(n)
  }

  fn hide_cursor(&mut self) -> io::Result<()> {
    self.inner.hide_cursor()
  }

  fn show_cursor(&mut self) -> io::Result<()> {
    self.inner.show_cursor()
  }

  fn get_cursor_position(&mut self) -> io::Result<Position> {
    self.inner.get_cursor_position()
  }

  fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
    self.inner.set_cursor_position(position)
  }

  fn clear(&mut self) -> io::Result<()> {
    self.inner.clear()
  }

  fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
    self.inner.clear_region(clear_type)
  }

  fn size(&self) -> io::Result<Size> {
    self.inner.size()
  }

  fn window_size(&mut self) -> io::Result<WindowSize> {
    self.inner.window_size()
  }

  fn flush(&mut self) -> io::Result<()> {
    Backend::flush(&mut self.inner)
  }
}
