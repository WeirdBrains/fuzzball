//! Encoder and decoder for the **Fuzzball** terminal protocol.
//!
//! Fuzzball is a small family of terminal extensions, first implemented in the
//! [Mouser](https://github.com/WeirdBrains/mouser) terminal. Every message is a DCS
//! string with the private marker `=` and the family tag `Fz`:
//!
//! ```text
//! ESC P = F z <subsystem> <verb> ; <params> [: <payload>] ESC \
//! ```
//!
//! This crate has no dependencies and does no I/O. It turns typed values into the exact
//! bytes Mouser emits and parses those bytes back, including partial input:
//!
//! - [`message`]: framing, [`Params`], [`parse`] for one message and a streaming
//!   [`Decoder`] that splits terminal input into Fuzzball messages and other bytes.
//! - [`common`]: capability query and reply, error codes, generic replies.
//! - [`keyboard`]: key events, enable / disable / query commands.
//! - [`graphics`]: upload (with chunking), place, delete, query.
//! - [`shell`]: shell-integration silent actions (advertise, change directory, reply).
//! - [`plugins`]: plugin hello, query, bye and replies.
//! - [`passthrough`]: tmux and GNU screen wrapping.
//!
//! The wire format is specified in the `docs/` directory of the repository.
//!
//! # Parsing key events
//!
//! ```
//! use fuzzball::keyboard::{Key, KeyEvent, KeyKind};
//! use fuzzball::{Decoder, Item};
//!
//! let mut d = Decoder::new();
//! d.push(b"x\x1bP=Fzke;t=p,code=KeyA,key=U+0061,te");
//! assert_eq!(d.next(), Some(Item::Text(b"x".to_vec())));
//! assert_eq!(d.next(), None); // the rest of the message has not arrived yet
//! d.push(b"xt=61\x1b\\");
//! let Some(Item::Message(m)) = d.next() else { panic!() };
//! let ev = KeyEvent::from_message(&m).unwrap();
//! assert_eq!(ev.kind, KeyKind::Press);
//! assert_eq!(ev.key, Some(Key::Char('a')));
//! assert_eq!(ev.text.as_deref(), Some("a"));
//! ```
//!
//! # Stability
//!
//! From 1.0.0 on, the wire format and this API follow semantic versioning: public enums
//! and structs that may grow are `#[non_exhaustive]`.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod common;
pub mod graphics;
pub mod keyboard;
pub mod message;
pub mod passthrough;
pub mod plugins;
pub mod shell;

pub use common::{ErrorCode, Reply};
pub use message::{Decoder, Item, Message, Params, Parse, parse, parse_with_limit};

/// Bytes every Fuzzball message starts with: `ESC P = F z`.
pub const PREFIX: &[u8] = b"\x1bP=Fz";
/// String terminator: `ESC \`.
pub const ST: &[u8] = b"\x1b\\";
/// Wire protocol version (`v=`) this crate speaks.
pub const VERSION: u32 = 0;
/// Largest payload per message Mouser accepts, in wire bytes (`max_chunk`).
pub const MAX_CHUNK: usize = 65536;
/// Default largest message [`parse`] and [`Decoder`] buffer before giving up.
pub const DEFAULT_MAX_MESSAGE: usize = MAX_CHUNK + 1024;
