# framework-tui

Shared interaction building blocks for [ratatui](https://ratatui.rs) apps: a
command prompt with history and completion, configurable multi-key bindings
with which-key hints and a help dialog, keymap files, `$EDITOR` integration,
terminal setup that always gives the terminal back, and the widgets that draw
them.

It gives [pdf-tui](https://github.com/WindustH/pdf-tui),
[gallery-tui](https://github.com/WindustH/gallery-tui),
[music-tui](https://github.com/WindustH/music-tui) and
[calibre-tui](https://github.com/WindustH/calibre-tui) the same prompts, keys
and dialogs, while each app keeps its own colors.

## Features

- **Command line and text prompts.** A `:` command line and labelled prompts
  (rename, search, ...) with the familiar editing keys, paste, and history:
  up and down walk through earlier commands and bring back what you were
  typing.
- **Completion.** The rest of the selected match appears after the cursor, a
  candidate list shows the alternatives, Tab cycles through them, and Enter
  accepts a completion before it runs the command.
- **Your keys, in your config.** Bindings are grouped into browser, detail,
  input and global sections. Keys are written as `j`, `G`, `enter`, `pgdn`,
  `ctrl-x`, `alt-x`, or in vim notation such as `<CR>`, `<C-x>`, `<A-x>` and
  `<S-Tab>`.
- **Key sequences with hints.** Bind sequences like `g g`; while one is
  half-typed, a hint bar lists the keys that can follow. A key that does not
  fit simply starts over.
- **Help dialog.** A scrollable popup lists the bindings that apply right
  now. Apps with several panes can merge their bindings into one list and
  send keys to the focused pane first.
- **Keymap files.** A ready-made `keymap.toml` format (`on`, `run`, `desc` per
  binding, grouped in sections) with the usual prompt keys as defaults. Files
  are written one binding per line, typos such as a misspelt `keymap` are
  caught instead of silently dropping bindings, and actions added in a new
  release get their default key while the user's own keys stay untouched.
- **External editor.** Hand the prompt, or any text, to `$EDITOR` and get the
  result back. The editor receives every keystroke, and the screen is
  repainted when you return.
- **The terminal always comes back.** Raw mode and the alternate screen are
  undone on exit, on errors, on panics and on `SIGTERM`/`SIGHUP`. A crash is
  printed on the normal screen where you can read it, and a failing
  background thread is reported to the app instead of scribbling over the UI.
- **Pipe-friendly.** Draw the UI on stderr, or on stdout only when it is a
  terminal, so `app | xargs ...` and `$(app)` receive just the app's output.
- **Works with any script.** Chinese, Japanese and other wide text, emoji and
  accents line up; long input scrolls so the cursor stays visible; AltGr
  characters type normally on Windows.
- **Themeable.** Every widget takes a style, and overlays default to the
  terminal's own background.

## Usage

```toml
[dependencies]
# `serde` adds reading and writing keymap files.
framework-tui = { git = "https://github.com/WindustH/framework-tui", features = ["serde"] }
```

Build the bindings once from your config, then feed key events through them:

```rust
use crossterm::event::KeyEvent;
use framework_tui::{
  CommandCompletion, CommandState, KeyBindingConfig, KeyBindings, KeyContext, KeyDispatcher,
  MatchResult, Prompt, PromptInputResult, current_word_start, filter_completion_candidates,
  handle_prompt_key, key_event_to_token,
};

fn bind(on: &[&str], action: &str, desc: &str) -> KeyBindingConfig {
  KeyBindingConfig {
    on: on.iter().map(|key| key.to_string()).collect(),
    action: action.to_string(),
    desc: desc.to_string(),
  }
}

struct App {
  bindings: KeyBindings,
  dispatcher: KeyDispatcher,
  prompt: Option<Prompt>,
  commands: CommandState,
}

impl App {
  fn new() -> Self {
    let bindings = KeyBindings::from_sections(
      // browser
      [
        bind(&["j"], "down", "Move down"),
        bind(&["g", "g"], "top", "Go to top"),
        bind(&[":"], "command", "Open the command line"),
      ],
      // detail
      [],
      // input: actions the prompt understands
      [
        bind(&["enter"], "submit", "Run"),
        bind(&["esc"], "cancel", "Cancel"),
        bind(&["backspace"], "backspace", "Delete backwards"),
        bind(&["tab"], "completion_next", "Next completion"),
        bind(&["up"], "history_previous", "Previous command"),
        bind(&["down"], "history_next", "Next command"),
      ],
      // global
      [bind(&["q"], "quit", "Quit")],
    );
    Self {
      bindings,
      dispatcher: KeyDispatcher::default(),
      prompt: None,
      commands: CommandState::default(),
    }
  }

  fn on_key(&mut self, key: KeyEvent) {
    if let Some(prompt) = &mut self.prompt {
      match handle_prompt_key(prompt, &mut self.commands, &self.bindings, key) {
        PromptInputResult::Changed => self.update_completion(),
        PromptInputResult::Submit => {
          let line = prompt.buffer().input.clone();
          self.commands.push_history(line.clone());
          self.prompt = None;
          self.run(&line);
        }
        PromptInputResult::Cancel => self.prompt = None,
        _ => {}
      }
      return;
    }
    let Some(token) = key_event_to_token(key) else {
      return;
    };
    if let MatchResult::Action(action) =
      self.dispatcher.dispatch(&self.bindings, KeyContext::Browser, token)
    {
      self.run(&action);
    }
  }

  fn update_completion(&mut self) {
    let Some(prompt) = &self.prompt else {
      return;
    };
    let buffer = prompt.buffer();
    let start = current_word_start(&buffer.input, buffer.cursor);
    let word = &buffer.input[start..buffer.cursor];
    let candidates = filter_completion_candidates(["open", "quit", "sort"], word);
    let completion = CommandCompletion::new(start, buffer.cursor, word, candidates, true, 0);
    self.commands.set_completion_preserving_selection(Some(completion));
  }

  fn run(&mut self, action: &str) {
    if action == "command" {
      self.commands.reset_prompt_state();
      self.prompt = Some(Prompt::command(""));
      self.update_completion();
    }
    // ...
  }
}
```

Draw the pieces wherever they belong in your layout:

```rust
use framework_tui::{KeyHintsStyle, PromptLineStyle, draw_key_hints, draw_prompt_line};
use ratatui::{Frame, layout::Rect, style::Style};

fn draw_footer(frame: &mut Frame, app: &App, hints: Rect, prompt_line: Rect) {
  let hint_style = KeyHintsStyle {
    base: Style::default(),
    key: Style::default().bold(),
    separator: Style::default().dim(),
    description: Style::default(),
    separator_text: " → ".to_string(),
    columns: 3,
  };
  draw_key_hints(frame, app.dispatcher.hints(), hints, &hint_style);

  if let Some(prompt) = &app.prompt {
    let prompt_style = PromptLineStyle {
      base: Style::default(),
      prefix: Style::default().bold(),
      suggestion: Style::default().dim(),
    };
    draw_prompt_line(frame, prompt, app.commands.completion(), prompt_line, &prompt_style);
  }
}
```

`draw_completion_list` shows the candidates, and `KeyBindings::help_entries`
with `draw_key_help_dialog_scrolled` and `handle_help_dialog_key` make the help
dialog.

### Prompt actions

Bind these in the input section:

| Action | Effect |
| --- | --- |
| `submit` | Accept the selected completion, or submit the input |
| `cancel` | Close the prompt |
| `backspace`, `delete` | Delete before / under the cursor |
| `move_left`, `move_right`, `move_start`, `move_end` | Move the cursor |
| `kill_before_cursor`, `kill_after_cursor` | Delete to the start / end |
| `completion_next`, `completion_previous` | Cycle through completions |
| `history_previous`, `history_next` | Walk the command history |
| `edit_in_editor` | Ask the app to open the input in `$EDITOR` |

Any other action comes back to the app as
`PromptInputResult::UnknownAction`. The help dialog scrolls with the browser
actions `scroll_up`, `scroll_down`, `page_up` and `page_down`, and any other key
closes it.

For `edit_in_editor`, call `edit_text_outside_tui` (see below) and put the
result back with `PromptBuffer::set_input`.

### Keymap files

Keep the sections your app needs in its own config struct; the entries,
defaults and file layout come from the library:

```rust
use framework_tui::keymap::{
  InputKeymapOptions, KeyBindings, KeymapSection, default_input_keymap, format_keymap_sections,
  key,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct KeymapConfig {
  browser: KeymapSection,
  input: KeymapSection,
  global: KeymapSection,
}

impl Default for KeymapConfig {
  fn default() -> Self {
    Self {
      browser: KeymapSection::new(vec![
        key("j", "down", "Move down"),
        key(["g", "g"], "top", "Go to top"),
      ]),
      input: default_input_keymap(&InputKeymapOptions::default()),
      global: KeymapSection::new(vec![key("q", "quit", "Quit")]),
    }
  }
}

impl KeymapConfig {
  fn bindings(&self) -> KeyBindings {
    KeyBindings::from_sections(
      self.browser.binding_configs(),
      [],
      self.input.binding_configs(),
      self.global.binding_configs(),
    )
  }

  /// Give actions added since the file was written their default keys.
  fn fill_in_defaults(&mut self) {
    let defaults = Self::default();
    self.browser.append_missing_actions(&defaults.browser);
    self.input.append_missing_actions(&defaults.input);
    self.global.append_missing_actions(&defaults.global);
  }

  fn to_toml(&self) -> String {
    format_keymap_sections([
      ("browser", &self.browser),
      ("input", &self.input),
      ("global", &self.global),
    ])
  }
}
```

### Terminal

Set up the terminal once, read input on a background thread, and hand the
terminal to the editor when asked. Nothing here needs an async runtime; an
async app forwards input into its own channel from the reader's callback.

```rust
use framework_tui::{
  EditorOptions, InputReader, PanicOrigin, TerminalOptions, TerminalOutput, TerminalSession,
  edit_text_outside_tui, install_panic_hook,
};

fn main() -> std::io::Result<()> {
  install_panic_hook(|info, origin| {
    if origin == PanicOrigin::Background {
      // Log it: it is not printed while the UI is on screen.
    }
  });
  let mut session = TerminalSession::enter(TerminalOptions {
    output: TerminalOutput::stdout_if_terminal(),
    ..TerminalOptions::default()
  })?;
  let (tx, rx) = std::sync::mpsc::channel();
  let input = InputReader::spawn(move |event| tx.send(event).is_ok())?;

  for event in rx {
    if !input.is_current(event.generation) {
      continue; // typed before the editor ran
    }
    // ... handle event.event, draw with session.draw(...) ...
    let handoff = edit_text_outside_tui(
      &mut session,
      Some(&input),
      "text to edit",
      std::path::Path::new("/tmp/my-app"),
      &EditorOptions::default(),
    );
    let _edited = handoff.output;
    handoff.terminal?;
    break;
  }
  session.restore()
}
```

Apps that draw extra things around ratatui (such as terminal images) wrap the
session and implement `SuspendTerminal` to tear them down and rebuild them.
`watch_termination_signals` calls back on `SIGTERM` and `SIGHUP`, so the app
can quit cleanly.

## License

MIT
