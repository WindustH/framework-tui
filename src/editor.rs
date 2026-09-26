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

/// Editor used when neither `EDITOR` nor `VISUAL` is set.
#[cfg(not(windows))]
const FALLBACK_EDITOR: &str = "vi";
#[cfg(windows)]
const FALLBACK_EDITOR: &str = "notepad";

#[derive(Debug, Clone)]
pub struct EditorOptions {
  /// Editor command; `None` (or a blank string) uses `$EDITOR`, then
  /// `$VISUAL`, then `vi` (`notepad` on Windows).
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
  ["EDITOR", "VISUAL"]
    .into_iter()
    .filter_map(|name| env::var(name).ok())
    .find(|editor| !editor.trim().is_empty())
    .unwrap_or_else(|| FALLBACK_EDITOR.to_string())
}

#[cfg(all(test, unix))]
mod tests {
  use super::*;

  /// A fresh scratch directory under the system temp dir, removed on drop.
  struct ScratchDir(PathBuf);

  impl ScratchDir {
    fn new(name: &str) -> Self {
      let dir = env::temp_dir().join(format!(
        "framework-tui-editor-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
          .duration_since(SystemTime::UNIX_EPOCH)
          .unwrap()
          .as_nanos()
      ));
      fs::create_dir_all(&dir).unwrap();
      Self(dir)
    }

    fn editor_files(&self) -> usize {
      fs::read_dir(self.0.join("editor"))
        .map(|entries| entries.count())
        .unwrap_or(0)
    }
  }

  impl Drop for ScratchDir {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  fn options(editor: &str) -> EditorOptions {
    EditorOptions {
      editor: Some(editor.to_string()),
      ..EditorOptions::default()
    }
  }

  #[test]
  fn returns_the_edited_text_and_removes_the_file() {
    // Quotes and spaces in the cache path reach the editor unchanged.
    let scratch = ScratchDir::new("it's a \"dir\"");
    let sources = ScratchDir::new("source");
    let source = sources.0.join("source.txt");
    fs::write(&source, "edited\n\n").unwrap();
    let editor = format!("cp '{}'", source.display());
    let result = edit_text_in_editor_with_options("initial", &scratch.0, &options(&editor));
    assert_eq!(result.as_deref(), Ok("edited"));
    assert_eq!(scratch.editor_files(), 0);

    let untrimmed = EditorOptions {
      trim_trailing_newline: false,
      ..options(&editor)
    };
    let result = edit_text_in_editor_with_options("initial", &scratch.0, &untrimmed);
    assert_eq!(result.as_deref(), Ok("edited\n\n"));
  }

  #[test]
  fn failing_editor_reports_and_cleans_up() {
    let scratch = ScratchDir::new("failing");
    let result = edit_text_in_editor_with_options("initial", &scratch.0, &options("false"));
    assert!(result.unwrap_err().starts_with("editor exited with"));
    assert_eq!(scratch.editor_files(), 0);
  }

  #[test]
  fn editor_can_carry_arguments() {
    let scratch = ScratchDir::new("args");
    let result = edit_text_in_editor_with_options("keep", &scratch.0, &options("true --wait"));
    assert_eq!(result.as_deref(), Ok("keep"));
  }
}
