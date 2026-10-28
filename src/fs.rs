//! Cross-platform atomic file writes: unique `create_new` temp files plus a
//! destination replace that overwrites on every platform. Plain `fs::rename`
//! fails on Windows once the destination exists, so repeated saves need
//! `MoveFileExW` there.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic per-process serial so two calls within one run never reuse a
/// temp name; the pid distinguishes concurrent instances.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A unique temp path under `dir` for `hint` (same directory as the target,
/// so a later rename stays on one filesystem).
pub fn unique_temp_path(dir: &Path, hint: &str) -> PathBuf {
  let serial = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
  dir.join(format!(
    ".{hint}.{}.{serial}.{}.tmp",
    std::process::id(),
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos()
  ))
}

/// Atomically publish `body` at `path`: write a `create_new` temp file in
/// the same directory, sync it, then replace the destination. On failure the
/// temp file is removed and the destination is left untouched. The
/// destination is replaced (not just renamed), so repeated writes also work
/// on Windows where a plain rename fails when the target already exists.
pub fn atomic_write_bytes(path: &Path, body: &[u8]) -> io::Result<()> {
  let dir = path.parent().unwrap_or_else(|| Path::new("."));
  let hint = path
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or("file");
  std::fs::create_dir_all(dir)?;
  let temp = unique_temp_path(dir, hint);
  let result = (|| {
    let mut file = std::fs::OpenOptions::new()
      .create_new(true)
      .write(true)
      .open(&temp)?;
    file.write_all(body)?;
    file.sync_all()?;
    replace_path(&temp, path)
  })();
  if result.is_err() {
    let _ = std::fs::remove_file(&temp);
  }
  result
}

/// Replace `destination` with `source`, overwriting any existing file. Unix
/// `rename` does this atomically; Windows needs `MoveFileExW`.
pub fn replace_path(source: &Path, destination: &Path) -> io::Result<()> {
  #[cfg(not(windows))]
  {
    std::fs::rename(source, destination)
  }
  #[cfg(windows)]
  {
    move_file_ex(source, destination)
  }
}

#[cfg(windows)]
fn move_file_ex(source: &Path, destination: &Path) -> io::Result<()> {
  use std::os::windows::ffi::OsStrExt;
  use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
  };

  let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
  let destination: Vec<u16> = destination
    .as_os_str()
    .encode_wide()
    .chain(Some(0))
    .collect();
  let moved = unsafe {
    MoveFileExW(
      source.as_ptr(),
      destination.as_ptr(),
      MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
    )
  };
  if moved == 0 {
    Err(io::Error::last_os_error())
  } else {
    Ok(())
  }
}
