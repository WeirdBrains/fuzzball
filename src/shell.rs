//! Shell-integration subsystem (`s`): silent actions such as changing directory without
//! typing `cd`.
//!
//! ```
//! use fuzzball::shell::{Advertise, ChangeDir};
//!
//! assert_eq!(Advertise::new("zsh", &["cwd"]).encode(), b"\x1bP=Fzsa;v=0,shell=zsh,caps=cwd\x1b\\");
//! assert_eq!(ChangeDir::new(7, b"/home/me/a dir").encode(), b"\x1bP=Fzsc;id=7,path=/home/me/a%20dir\x1b\\");
//! ```

use crate::common::ErrorCode;
use crate::message::{Message, Params};

/// The only capability v0 defines: change directory.
pub const CAP_CWD: &str = "cwd";

/// Shell → terminal: advertise (or, with no caps, withdraw) silent actions (`s a`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Advertise {
    /// `v`.
    pub version: u32,
    /// `shell` (informational: `zsh`, `bash`, `fish`).
    pub shell: Option<String>,
    /// Capabilities, `+`-joined on the wire. Empty withdraws.
    pub caps: Vec<String>,
}

impl Advertise {
    /// Advertise `caps` for `shell`.
    pub fn new(shell: &str, caps: &[&str]) -> Self {
        Advertise { version: crate::VERSION, shell: Some(shell.into()), caps: caps.iter().map(|c| c.to_string()).collect() }
    }

    /// Withdraw: `v=0,caps=`.
    pub fn withdraw() -> Self {
        Advertise { version: crate::VERSION, shell: None, caps: Vec::new() }
    }

    /// Whether `cap` is advertised.
    pub fn has(&self, cap: &str) -> bool {
        self.caps.iter().any(|c| c == cap)
    }

    /// The message: `v,shell,caps`.
    pub fn to_message(&self) -> Message {
        let mut p = Params::new().with("v", self.version);
        if let Some(s) = &self.shell {
            p.push("shell", s);
        }
        p.push("caps", self.caps.join("+"));
        Message::new(b's', b'a', p)
    }

    /// Wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        self.to_message().encode()
    }

    /// Read from an `s a` message. A missing `caps` is a withdrawal.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b's', b'a') {
            return None;
        }
        let p = &m.params;
        Some(Advertise {
            version: p.num("v").unwrap_or(crate::VERSION),
            shell: p.get("shell").map(String::from),
            caps: p.get("caps").unwrap_or("").split('+').filter(|c| !c.is_empty()).map(String::from).collect(),
        })
    }
}

/// Terminal → shell input: change directory (`s c`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChangeDir {
    /// Request id.
    pub id: u32,
    /// Path bytes (not necessarily UTF-8).
    pub path: Vec<u8>,
}

impl ChangeDir {
    /// A request.
    pub fn new(id: u32, path: &[u8]) -> Self {
        ChangeDir { id, path: path.to_vec() }
    }

    /// Wire bytes: `id=<id>,path=<pct>`.
    pub fn encode(&self) -> Vec<u8> {
        Message::new(b's', b'c', Params::new().with("id", self.id).with("path", pct_encode(&self.path))).encode()
    }

    /// Read from an `s c` message.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b's', b'c') {
            return None;
        }
        Some(ChangeDir { id: m.params.num("id")?, path: pct_decode(m.params.get("path")?)? })
    }
}

/// Shell → terminal: result of a request (`s r`). Never answered.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShellReply {
    /// Request id.
    pub id: u32,
    /// `Ok`, or the error code and decoded `msg` bytes.
    pub result: Result<(), (ErrorCode, Vec<u8>)>,
}

impl ShellReply {
    /// Success.
    pub fn ok(id: u32) -> Self {
        ShellReply { id, result: Ok(()) }
    }

    /// Failure.
    pub fn error(id: u32, code: ErrorCode, msg: &[u8]) -> Self {
        ShellReply { id, result: Err((code, msg.to_vec())) }
    }

    /// Wire bytes: `id,ok=1` or `id,ok=0,err,msg=<pct>`.
    pub fn encode(&self) -> Vec<u8> {
        let mut p = Params::new().with("id", self.id);
        match &self.result {
            Ok(()) => p.push("ok", 1),
            Err((c, m)) => {
                p.push("ok", 0);
                p.push("err", c.as_str());
                p.push("msg", pct_encode(m));
            }
        }
        Message::new(b's', b'r', p).encode()
    }

    /// Read from an `s r` message. A `msg` that fails to decode is kept verbatim.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b's', b'r') {
            return None;
        }
        let p = &m.params;
        let id = p.num("id")?;
        let result = if p.num("ok") == Some(1) {
            Ok(())
        } else {
            let msg = p.get("msg").unwrap_or("");
            Err((ErrorCode::from_code(p.get("err").unwrap_or("")), pct_decode(msg).unwrap_or_else(|| msg.as_bytes().to_vec())))
        };
        Some(ShellReply { id, result })
    }
}

/// Percent-encode: every byte but `A-Z a-z 0-9 - . _ ~ /` becomes `%XX` (uppercase).
pub fn pct_encode(b: &[u8]) -> String {
    let mut o = String::with_capacity(b.len());
    for &c in b {
        if c.is_ascii_alphanumeric() || b"-._~/".contains(&c) {
            o.push(c as char);
        } else {
            o.push_str(&format!("%{c:02X}"));
        }
    }
    o
}

/// Percent-decode (either hex case). `None` on a truncated or bad escape.
pub fn pct_decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    let mut o = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            if !h.bytes().all(|x| x.is_ascii_hexdigit()) {
                return None;
            }
            o.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            o.push(b[i]);
            i += 1;
        }
    }
    Some(o)
}
