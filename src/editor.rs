use std::{env, fs, path::Path, process::Command, time::SystemTime};

#[derive(Debug, Clone)]
pub struct EditorOptions {
  pub editor: Option<String>,
  pub file_prefix: String,
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

pub fn edit_text_in_editor(initial: &str, cache_dir: &Path) -> Result<String, String> {
  edit_text_in_editor_with_options(initial, cache_dir, &EditorOptions::default())
}

pub fn edit_text_in_editor_with_options(
  initial: &str,
  cache_dir: &Path,
  options: &EditorOptions,
) -> Result<String, String> {
  let editor = options.editor.clone().unwrap_or_else(default_editor);
  let editor_dir = cache_dir.join("editor");
  fs::create_dir_all(&editor_dir).map_err(|err| err.to_string())?;
  let nanos = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .map_err(|err| err.to_string())?
    .as_nanos();
  let path = editor_dir.join(format!(
    "{}-{}-{nanos}.txt",
    options.file_prefix,
    std::process::id()
  ));
  fs::write(&path, initial).map_err(|err| err.to_string())?;

  let status = launch_editor(&editor, &path).map_err(|err| err.to_string())?;
  if !status.success() {
    let _ = fs::remove_file(&path);
    return Err(format!("editor exited with {status}"));
  }
  let edited = fs::read_to_string(&path)
    .map_err(|err| err.to_string())
    .inspect_err(|_| {
      let _ = fs::remove_file(&path);
    })?;
  let _ = fs::remove_file(&path);
  if options.trim_trailing_newline {
    Ok(edited.trim_end_matches(['\r', '\n']).to_string())
  } else {
    Ok(edited)
  }
}

#[cfg(not(windows))]
fn launch_editor(editor: &str, path: &Path) -> std::io::Result<std::process::ExitStatus> {
  Command::new("sh")
    .arg("-c")
    .arg(format!("{} {}", editor, shell_quote(&path.display().to_string())))
    .status()
}

#[cfg(windows)]
fn launch_editor(editor: &str, path: &Path) -> std::io::Result<std::process::ExitStatus> {
  Command::new(editor).arg(path).status()
}

fn default_editor() -> String {
  env::var("EDITOR")
    .or_else(|_| env::var("VISUAL"))
    .unwrap_or_else(|_| "vi".to_string())
}

#[cfg(not(windows))]
fn shell_quote(value: &str) -> String {
  let mut quoted = String::from("'");
  for ch in value.chars() {
    if ch == '\'' {
      quoted.push_str("'\\''");
    } else {
      quoted.push(ch);
    }
  }
  quoted.push('\'');
  quoted
}
