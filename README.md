# framework-tui

Shared interaction building blocks for [ratatui](https://ratatui.rs) apps: a
command prompt with history and completion, configurable multi-key bindings
with which-key hints and a help dialog, `$EDITOR` integration, and the widgets
that draw them.

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
- **External editor.** Hand the prompt, or any text, to `$EDITOR` and get the
  result back.
- **Works with any script.** Chinese, Japanese and other wide text, emoji and
  accents line up; long input scrolls so the cursor stays visible; AltGr
  characters type normally on Windows.
- **Themeable.** Every widget takes a style, and overlays default to the
  terminal's own background.

## Usage

```toml
[dependencies]
framework-tui = { git = "https://github.com/WindustH/framework-tui" }
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

For `edit_in_editor`, leave raw mode and the alternate screen, call
`edit_text_in_editor(&input, cache_dir)`, restore the terminal, and put the
result back with `PromptBuffer::set_input`.

## License

MIT
