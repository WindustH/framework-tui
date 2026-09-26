//! Configurable key bindings.
//!
//! Apps describe bindings as [`KeyBindingConfig`] entries in four sections
//! (browser, detail, input, global) and build [`KeyBindings`] from them.
//! Key events become string tokens with [`key_event_to_token`];
//! [`KeyDispatcher`] feeds tokens through the bindings, tracking multi-key
//! sequences and their which-key hints, and [`KeyBindings::help_entries`]
//! lists the bindings for a help dialog.
//!
//! Keys in a config can be written as plain names (`j`, `G`, `enter`,
//! `pgdn`, `ctrl-x`, `alt-x`) or in vim-style notation (`<CR>`, `<C-x>`,
//! `<A-x>`, `<S-Tab>`). Named keys match in any case; single characters
//! are case-sensitive.

mod bindings;
mod dispatch;
mod help;
mod token;

pub use bindings::{KeyBindingConfig, KeyBindings, KeyContext, KeyHint, MatchResult};
pub use dispatch::KeyDispatcher;
pub use help::{KeyHelpEntry, merge_help_entries};
pub use token::key_event_to_token;

pub(crate) use token::typed_char;
