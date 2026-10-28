//! Platform directories shared by the application family.
use std::{env, path::PathBuf};
pub fn env_path(name: &str) -> Option<PathBuf> {
  env::var_os(name)
    .filter(|v| !v.is_empty())
    .map(PathBuf::from)
}
fn platform_dir(xdg: &str, home_child: &str, windows: &str) -> PathBuf {
  if cfg!(windows) {
    env_path(windows).unwrap_or_else(|| PathBuf::from("."))
  } else {
    env_path(xdg)
      .or_else(|| env_path("HOME").map(|p| p.join(home_child)))
      .unwrap_or_else(|| PathBuf::from(home_child))
  }
}
pub fn config_dir() -> PathBuf {
  platform_dir("XDG_CONFIG_HOME", ".config", "APPDATA")
}
pub fn cache_dir() -> PathBuf {
  platform_dir("XDG_CACHE_HOME", ".cache", "LOCALAPPDATA")
}
pub fn state_dir() -> PathBuf {
  platform_dir("XDG_STATE_HOME", ".local/state", "APPDATA")
}
pub fn expand_home(path: &str) -> PathBuf {
  if (path == "~" || path.starts_with("~/"))
    && let Some(home) = env_path(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
  {
    return if path == "~" {
      home
    } else {
      home.join(&path[2..])
    };
  }
  PathBuf::from(path)
}
