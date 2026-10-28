//! Editing text in the user's external editor.
//!
//! The text is written to a temporary file under `<cache_dir>/editor/`, the
//! editor is run on it and waited for, and the edited text is read back.
//! The temporary file is removed afterwards in every case.
//!
//! The editor takes over the terminal: leave raw mode and the alternate
//! screen before calling, and restore them (and redraw) afterwards.

use std::{
  env, fs, io,
  path::{Path, PathBuf},
  process::{Command, ExitStatus},
  time::SystemTime,
};

/// Variables naming the user's editor, in order: `VISUAL` (the full-screen
/// editor) wins over `EDITOR`, as in git and most Unix tools.
const EDITOR_VARS: [&str; 2] = ["VISUAL", "EDITOR"];

/// Editor used when neither `VISUAL` nor `EDITOR` is set.
#[cfg(not(windows))]
const FALLBACK_EDITOR: &str = "vi";
#[cfg(windows)]
const FALLBACK_EDITOR: &str = "notepad";

#[derive(Debug, Clone)]
pub struct EditorOptions {
  /// Editor command; `None` (or a blank string) uses `$VISUAL`, then
  /// `$EDITOR`, then `vi` (`notepad` on Windows).
  pub editor: Option<String>,
  /// Start of the temporary file name.
  pub file_prefix: String,
  /// Strip trailing line breaks from the result (editors usually add one).
  pub trim_trailing_newline: bool,
}

impl Default for EditorOptions {
  fn default() -> Self {
    Self {
      editor: None,
      file_prefix: "input".to_string(),
      trim_trailing_newline: true,
    }
  }
}

/// Edit `initial` in the user's editor with default options. Returns the
/// edited text without trailing line breaks.
pub fn edit_text_in_editor(initial: &str, cache_dir: &Path) -> Result<String, String> {
  edit_text_in_editor_with_options(initial, cache_dir, &EditorOptions::default())
}

/// Edit `initial` in an editor. Fails if the editor cannot be started or
/// exits unsuccessfully.
///
/// On Unix the editor string is run by `sh`, so it may carry arguments
/// (`code --wait`). On Windows it must name the program alone.
pub fn edit_text_in_editor_with_options(
  initial: &str,
  cache_dir: &Path,
  options: &EditorOptions,
) -> Result<String, String> {
  let editor = options
    .editor
    .clone()
    .filter(|editor| !editor.trim().is_empty())
    .unwrap_or_else(default_editor);
  let file = TempFile::create(&cache_dir.join("editor"), &options.file_prefix, initial)?;

  let status = launch_editor(&editor, &file.path).map_err(|err| err.to_string())?;
  if !status.success() {
    return Err(format!("editor exited with {status}"));
  }
  let edited = fs::read_to_string(&file.path).map_err(|err| err.to_string())?;
  if options.trim_trailing_newline {
    Ok(edited.trim_end_matches(['\r', '\n']).to_string())
  } else {
    Ok(edited)
  }
}

/// A file that is deleted when dropped.
struct TempFile {
  path: PathBuf,
}

impl TempFile {
  fn create(dir: &Path, prefix: &str, contents: &str) -> Result<Self, String> {
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let nanos = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map_err(|err| err.to_string())?
      .as_nanos();
    let file = Self {
      path: dir.join(format!("{prefix}-{}-{nanos}.txt", std::process::id())),
    };
    fs::write(&file.path, contents).map_err(|err| err.to_string())?;
    Ok(file)
  }
}

impl Drop for TempFile {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.path);
  }
}

/// Run the editor as a shell command line, as git does: the path goes in
/// as a positional parameter, so it needs no quoting and non-UTF-8 paths
/// arrive intact.
#[cfg(not(windows))]
fn launch_editor(editor: &str, path: &Path) -> io::Result<ExitStatus> {
  Command::new("sh")
    .arg("-c")
    .arg(format!("{editor} \"$@\""))
    .arg(editor)
    .arg(path)
    .status()
}

#[cfg(windows)]
fn launch_editor(editor: &str, path: &Path) -> io::Result<ExitStatus> {
  Command::new(editor).arg(path).status()
}

fn default_editor() -> String {
  editor_from_vars(|name| env::var(name).ok())
}

/// The first non-blank editor variable, or the fallback editor.
fn editor_from_vars(lookup: impl Fn(&str) -> Option<String>) -> String {
  EDITOR_VARS
    .into_iter()
    .filter_map(lookup)
    .find(|editor| !editor.trim().is_empty())
    .unwrap_or_else(|| FALLBACK_EDITOR.to_string())
}
