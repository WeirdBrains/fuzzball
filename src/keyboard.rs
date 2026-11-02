//! Keyboard subsystem (`k`): key events and the mode commands.
//!
//! ```
//! use fuzzball::keyboard::{self, Key, KeyEvent, KeyKind, Modifiers};
//!
//! assert_eq!(keyboard::ENABLE, b"\x1bP=Fzks;on=1,v=0\x1b\\");
//! let ev = KeyEvent::parse(b"\x1bP=Fzke;t=p,code=KeyC,key=U+0063,mods=4\x1b\\").unwrap();
//! assert_eq!(ev.key, Some(Key::Char('c')));
//! assert!(ev.mods.contains(Modifiers::CTRL));
//! assert_eq!(ev.encode(), b"\x1bP=Fzke;t=p,code=KeyC,key=U+0063,mods=4\x1b\\");
//! ```

use crate::message::{Message, Params, Parse, parse_with_limit};
use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// Enable structured key events for the current screen: `ESC P = F z k s ; on=1,v=0 ESC \`.
pub const ENABLE: &[u8] = b"\x1bP=Fzks;on=1,v=0\x1b\\";
/// Disable structured key events: `ESC P = F z k s ; on=0 ESC \`.
pub const DISABLE: &[u8] = b"\x1bP=Fzks;on=0\x1b\\";

/// [`ENABLE`] as a `&str` (for `fmt::Write` based command APIs such as crossterm's).
pub const ENABLE_STR: &str = "\x1bP=Fzks;on=1,v=0\x1b\\";
/// [`DISABLE`] as a `&str`.
pub const DISABLE_STR: &str = "\x1bP=Fzks;on=0\x1b\\";

/// `k s` with a request id, so the terminal replies `k r ; id=<id>,ok=1,on=<0|1>`.
/// `on=1` also sends `v=0`.
pub fn set_mode(on: bool, id: Option<u32>) -> Vec<u8> {
    let mut p = Params::new().with("on", u8::from(on));
    if on {
        p.push("v", crate::VERSION);
    }
    if let Some(id) = id {
        p.push("id", id);
    }
    Message::new(b'k', b's', p).encode()
}

/// Query the mode: `ESC P = F z k q ; id=<id> ESC \`.
pub fn query(id: u32) -> Vec<u8> {
    Message::new(b'k', b'q', Params::new().with("id", id)).encode()
}

/// A keyboard reply (`k r`) to [`set_mode`] or [`query`]: whether the mode is on.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ModeReply {
    /// Request id.
    pub id: u32,
    /// `ok=1`.
    pub ok: bool,
    /// `on=1`.
    pub on: bool,
}

impl ModeReply {
    /// Read from a `k r` message.
    pub fn from_message(m: &Message) -> Option<Self> {
        m.is(b'k', b'r').then(|| ModeReply {
            id: m.params.num("id").unwrap_or(0),
            ok: m.params.get("ok") == Some("1"),
            on: m.params.get("on") == Some("1"),
        })
    }

    /// Wire bytes: `id=<id>,ok=1,on=<0|1>` (Mouser's order).
    pub fn encode(&self) -> Vec<u8> {
        let p = Params::new().with("id", self.id).with("ok", u8::from(self.ok)).with("on", u8::from(self.on));
        Message::new(b'k', b'r', p).encode()
    }

    /// A reply.
    pub fn new(id: u32, ok: bool, on: bool) -> Self {
        ModeReply { id, ok, on }
    }
}

/// Press, repeat or release (`t=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KeyKind {
    /// `t=p`.
    #[default]
    Press,
    /// `t=r`.
    Repeat,
    /// `t=u`.
    Release,
}

impl KeyKind {
    fn letter(self) -> char {
        match self {
            KeyKind::Press => 'p',
            KeyKind::Repeat => 'r',
            KeyKind::Release => 'u',
        }
    }
}

macro_rules! bits {
    ($(#[$m:meta])* $name:ident { $($(#[$cm:meta])* $c:ident = $v:expr;)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(u8);
        impl $name {
            /// None set.
            pub const NONE: Self = Self(0);
            $($(#[$cm])* pub const $c: Self = Self($v);)*
            /// From raw wire bits (unknown bits are kept).
            pub const fn from_bits(b: u8) -> Self { Self(b) }
            /// Raw wire bits.
            pub const fn bits(self) -> u8 { self.0 }
            /// Whether every bit of `o` is set.
            pub const fn contains(self, o: Self) -> bool { self.0 & o.0 == o.0 }
            /// Whether no bit is set.
            pub const fn is_empty(self) -> bool { self.0 == 0 }
            /// Set the bits of `o`.
            pub fn insert(&mut self, o: Self) { self.0 |= o.0 }
            /// Clear the bits of `o`.
            pub fn remove(&mut self, o: Self) { self.0 &= !o.0 }
        }
        impl BitOr for $name { type Output = Self; fn bitor(self, o: Self) -> Self { Self(self.0 | o.0) } }
        impl BitOrAssign for $name { fn bitor_assign(&mut self, o: Self) { self.0 |= o.0 } }
    };
}

bits!(
    /// Held modifiers (`mods=`).
    Modifiers {
        /// Shift (1).
        SHIFT = 1;
        /// Alt (2).
        ALT = 2;
        /// Control (4).
        CTRL = 4;
        /// Super (8). Mouser also reports the Meta/Windows/Command key here.
        SUPER = 8;
        /// Hyper (16).
        HYPER = 16;
        /// Meta (32).
        META = 32;
        /// AltGr (64).
        ALT_GR = 64;
    }
);

bits!(
    /// Active locks (`locks=`).
    Locks {
        /// Caps Lock (1).
        CAPS_LOCK = 1;
        /// Num Lock (2).
        NUM_LOCK = 2;
        /// Scroll Lock (4).
        SCROLL_LOCK = 4;
    }
);

/// IME composition phase (`ime=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImePhase {
    /// `start`.
    Start,
    /// `update`: pre-edit text changed, not inserted.
    Update,
    /// `commit`: `text` is inserted.
    Commit,
    /// `cancel`.
    Cancel,
}

impl ImePhase {
    fn as_str(self) -> &'static str {
        match self {
            ImePhase::Start => "start",
            ImePhase::Update => "update",
            ImePhase::Commit => "commit",
            ImePhase::Cancel => "cancel",
        }
    }
}

/// Logical key (`key=`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// The unmodified character, `U+XXXX` on the wire (uppercase hex, at least 4 digits).
    Char(char),
    /// A named key such as `Enter`, `ArrowUp`, `Shift`, `F13`.
    Named(String),
}

impl Key {
    /// Parse a `key=` value. `U+` followed by hex of a valid scalar is a [`Key::Char`].
    pub fn parse(v: &str) -> Key {
        if let Some(h) = v.strip_prefix("U+") {
            if !h.is_empty() && h.len() <= 6 && h.bytes().all(|b| b.is_ascii_hexdigit()) {
                if let Some(c) = u32::from_str_radix(h, 16).ok().and_then(char::from_u32) {
                    return Key::Char(c);
                }
            }
        }
        Key::Named(v.to_string())
    }

    /// The name, if named.
    pub fn name(&self) -> Option<&str> {
        match self {
            Key::Named(n) => Some(n),
            Key::Char(_) => None,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char(c) => write!(f, "U+{:04X}", *c as u32),
            Key::Named(n) => f.write_str(n),
        }
    }
}

/// One key event (`ESC P = F z k e ; … ESC \`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct KeyEvent {
    /// Press, repeat or release.
    pub kind: KeyKind,
    /// Physical key, W3C UI Events `code` (e.g. `KeyA`). `None` for IME messages.
    pub code: Option<String>,
    /// Logical key. `None` for IME messages.
    pub key: Option<Key>,
    /// Held modifiers (state after the event for modifier keys).
    pub mods: Modifiers,
    /// Active locks.
    pub locks: Locks,
    /// Inserted text (presses and repeats), or IME pre-edit / committed text.
    pub text: Option<String>,
    /// IME phase.
    pub ime: Option<ImePhase>,
}

impl KeyEvent {
    /// A key event with no modifiers, locks or text.
    pub fn new(kind: KeyKind, code: impl Into<String>, key: Key) -> Self {
        KeyEvent { kind, code: Some(code.into()), key: Some(key), ..Default::default() }
    }

    /// An IME message (`t=p,ime=<phase>[,text=…]`).
    pub fn ime(phase: ImePhase, text: Option<String>) -> Self {
        KeyEvent { ime: Some(phase), text, ..Default::default() }
    }

    /// Set the modifiers (builder style).
    pub fn with_mods(mut self, mods: Modifiers) -> Self {
        self.mods = mods;
        self
    }

    /// Set the locks (builder style).
    pub fn with_locks(mut self, locks: Locks) -> Self {
        self.locks = locks;
        self
    }

    /// Set the text (builder style).
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Read from a `k e` message. Unknown params are ignored. `None` when `t` is missing
    /// or bad, `mods`/`locks`/`text`/`ime` are malformed, or a non-IME event has no `key`
    /// (the rules the crossterm fork applies).
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b'k', b'e') {
            return None;
        }
        let mut ev = KeyEvent::default();
        let mut have_t = false;
        for (k, v) in m.params.iter() {
            match k {
                "t" => {
                    have_t = true;
                    ev.kind = match v {
                        "p" => KeyKind::Press,
                        "r" => KeyKind::Repeat,
                        "u" => KeyKind::Release,
                        _ => return None,
                    }
                }
                "code" => ev.code = Some(v.to_string()).filter(|s| !s.is_empty()),
                "key" => ev.key = (!v.is_empty()).then(|| Key::parse(v)),
                "mods" => ev.mods = Modifiers(v.parse().ok()?),
                "locks" => ev.locks = Locks(v.parse().ok()?),
                "text" => ev.text = Some(hex_decode_utf8(v)?),
                "ime" => {
                    ev.ime = Some(match v {
                        "start" => ImePhase::Start,
                        "update" => ImePhase::Update,
                        "commit" => ImePhase::Commit,
                        "cancel" => ImePhase::Cancel,
                        _ => return None,
                    })
                }
                _ => {}
            }
        }
        if !have_t || (ev.ime.is_none() && ev.key.is_none()) {
            return None;
        }
        Some(ev)
    }

    /// Parse one complete framed event (`ESC P = F z k e ; … ESC \`).
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        Self::from_message(&Message::from_bytes(bytes)?)
    }

    /// The message. Order: `t, code, key, mods, locks, ime, text`; zero `mods`/`locks`
    /// and empty text are omitted, and release events never carry text (as Mouser emits).
    pub fn to_message(&self) -> Message {
        let mut p = Params::new().with("t", self.kind.letter());
        if let Some(c) = &self.code {
            p.push("code", c);
        }
        if let Some(k) = &self.key {
            p.push("key", k);
        }
        if !self.mods.is_empty() {
            p.push("mods", self.mods.0);
        }
        if !self.locks.is_empty() {
            p.push("locks", self.locks.0);
        }
        if let Some(i) = self.ime {
            p.push("ime", i.as_str());
        }
        if self.kind != KeyKind::Release {
            if let Some(t) = self.text.as_deref().filter(|t| !t.is_empty()) {
                p.push("text", hex_encode(t.as_bytes()));
            }
        }
        Message::new(b'k', b'e', p)
    }

    /// Wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        self.to_message().encode()
    }
}

/// Parse a key event at the start of `buf`, handling partial input.
///
/// Returns [`Parse::Incomplete`] while the message is still arriving (keep the bytes and
/// call again with more), [`Parse::Invalid`] for a complete Fuzzball message that is not
/// a valid key event (skip `len` bytes; use [`crate::parse`] to handle replies), and
/// [`Parse::NotFuzzball`] when `buf` does not start with `ESC P = F z`. Messages longer
/// than `max` bytes are invalid; the crossterm fork uses 4096.
pub fn parse_event(buf: &[u8], max: usize) -> Parse<KeyEvent> {
    parse_with_limit(buf, max).map(|m| KeyEvent::from_message(&m))
}

fn hex_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

fn hex_decode_utf8(v: &str) -> Option<String> {
    if v.len() % 2 != 0 || !v.is_ascii() {
        return None;
    }
    let bytes = (0..v.len()).step_by(2).map(|i| u8::from_str_radix(&v[i..i + 2], 16).ok()).collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}
