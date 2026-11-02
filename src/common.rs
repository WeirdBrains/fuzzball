//! Common subsystem (`c`): capability query and reply, plus error codes and the generic
//! reply shape every subsystem uses (`<sub> r`).

use crate::message::{Message, Params};

/// Error codes carried in `err=` (README §5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// `ENOTSUP`: unknown subsystem, verb or value.
    NotSupported,
    /// `EINVAL`: bad or missing parameter.
    Invalid,
    /// `EPROTO`: framing error.
    Protocol,
    /// `ENOENT`: referenced object does not exist.
    NotFound,
    /// `ENOMEM`: over a published limit.
    NoMemory,
    /// `EPERM`: refused by policy.
    Permission,
    /// `EVERSION`: requested version not available.
    Version,
    /// A code this crate does not know, kept verbatim.
    Other(String),
}

impl ErrorCode {
    /// The wire code, e.g. `"EINVAL"`.
    pub fn as_str(&self) -> &str {
        match self {
            ErrorCode::NotSupported => "ENOTSUP",
            ErrorCode::Invalid => "EINVAL",
            ErrorCode::Protocol => "EPROTO",
            ErrorCode::NotFound => "ENOENT",
            ErrorCode::NoMemory => "ENOMEM",
            ErrorCode::Permission => "EPERM",
            ErrorCode::Version => "EVERSION",
            ErrorCode::Other(s) => s,
        }
    }

    /// Parse a wire code.
    pub fn from_code(s: &str) -> Self {
        match s {
            "ENOTSUP" => ErrorCode::NotSupported,
            "EINVAL" => ErrorCode::Invalid,
            "EPROTO" => ErrorCode::Protocol,
            "ENOENT" => ErrorCode::NotFound,
            "ENOMEM" => ErrorCode::NoMemory,
            "EPERM" => ErrorCode::Permission,
            "EVERSION" => ErrorCode::Version,
            o => ErrorCode::Other(o.to_string()),
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A reply (`ESC P = F z <sub> r ; id=<n>,… ESC \`) from any subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Reply {
    /// Subsystem letter the reply belongs to.
    pub subsystem: u8,
    /// Request id it answers (0 if absent).
    pub id: u32,
    /// All params after `id`, in wire order (`ok`, `err`, `msg`, …).
    pub params: Params,
}

impl Reply {
    /// A success reply: `id=<id>,ok=1` followed by `extra`.
    pub fn ok(subsystem: u8, id: u32, extra: Params) -> Self {
        let mut params = Params::new().with("ok", 1);
        for (k, v) in extra.iter() {
            params.push(k, v);
        }
        Reply { subsystem, id, params }
    }

    /// A reply whose params follow `id` unchanged (the capability reply has no `ok`).
    pub fn raw(subsystem: u8, id: u32, params: Params) -> Self {
        Reply { subsystem, id, params }
    }

    /// An error reply: `id=<id>,ok=0,err=<code>,msg=<msg>` (Mouser always sends `msg`).
    pub fn error(subsystem: u8, id: u32, code: ErrorCode, msg: &str) -> Self {
        Reply { subsystem, id, params: Params::new().with("ok", 0).with("err", code.as_str()).with("msg", msg) }
    }

    /// `ok=1` present. Replies without `ok` (capabilities) count as success.
    pub fn is_ok(&self) -> bool {
        self.params.get("ok") != Some("0")
    }

    /// The error code, for `ok=0`.
    pub fn error_code(&self) -> Option<ErrorCode> {
        (!self.is_ok()).then(|| ErrorCode::from_code(self.params.get("err").unwrap_or("")))
    }

    /// The `msg=` text, verbatim.
    pub fn msg(&self) -> Option<&str> {
        self.params.get("msg")
    }

    /// Read a reply from a message with verb `r`.
    pub fn from_message(m: &Message) -> Option<Self> {
        if m.verb != b'r' {
            return None;
        }
        let id = m.params.num("id").unwrap_or(0);
        let params = m.params.iter().filter(|(k, _)| *k != "id").collect();
        Some(Reply { subsystem: m.subsystem, id, params })
    }

    /// The message.
    pub fn to_message(&self) -> Message {
        let mut p = Params::new().with("id", self.id);
        for (k, v) in self.params.iter() {
            p.push(k, v);
        }
        Message::new(self.subsystem, b'r', p)
    }

    /// Wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        self.to_message().encode()
    }
}

/// Capability query: `ESC P = F z c q ; id=<id> ESC \`.
pub fn capability_query(id: u32) -> Vec<u8> {
    Message::new(b'c', b'q', Params::new().with("id", id)).encode()
}

/// The detection sequence (README §4): the capability query followed by DA1 (`ESC [ c`).
/// If the DA1 answer arrives without a capability reply, Fuzzball is unavailable.
pub fn detect_query(id: u32) -> Vec<u8> {
    let mut v = capability_query(id);
    v.extend_from_slice(b"\x1b[c");
    v
}

/// A capability reply (`c r`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Capabilities {
    /// Reply id.
    pub id: u32,
    /// Protocol version (`v`).
    pub version: u32,
    /// Subsystem letters (`sub`), e.g. `"gkps"`.
    pub subsystems: String,
    /// `raw=1`: `enc=raw` payloads accepted.
    pub raw: bool,
    /// `max_chunk`, if given.
    pub max_chunk: Option<u32>,
    /// Every param, including the ones above and subsystem keys (`g_*`, `k_*`).
    pub params: Params,
}

impl Capabilities {
    /// Read from a `c r` message.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b'c', b'r') {
            return None;
        }
        let p = &m.params;
        Some(Self::new(p.num("id").unwrap_or(0), p.iter().filter(|(k, _)| *k != "id").collect()))
    }

    /// Capabilities with reply id `id` and the given params (in wire order); the typed
    /// fields are derived from them.
    pub fn new(id: u32, params: Params) -> Self {
        let p = &params;
        Capabilities {
            id,
            version: p.num("v").unwrap_or(0),
            subsystems: p.get("sub").unwrap_or("").to_string(),
            raw: p.get("raw") == Some("1"),
            max_chunk: p.num("max_chunk"),
            params: params.clone(),
        }
    }

    /// Whether subsystem letter `s` is listed in `sub`.
    pub fn has_subsystem(&self, s: char) -> bool {
        self.subsystems.contains(s)
    }

    /// Keyboard mode available: `sub` lists `k` and `k=1` (what the crossterm fork checks).
    pub fn keyboard(&self) -> bool {
        self.has_subsystem('k') && self.params.get("k") == Some("1")
    }

    /// Wire bytes: `id=<id>` followed by [`params`](Self::params) in order.
    pub fn encode(&self) -> Vec<u8> {
        Reply::raw(b'c', self.id, self.params.clone()).encode()
    }
}
