//! Plugin subsystem (`p`): a program in the terminal asks to act as a plugin.

use crate::common::{ErrorCode, Reply};
use crate::message::{Message, Params};

/// Plugin kind (`kind=`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Kind {
    /// `tool-provider` (default).
    #[default]
    ToolProvider,
    /// `mode`.
    Mode,
    /// `model-provider`.
    ModelProvider,
    /// `command`.
    Command,
    /// Unknown, kept verbatim.
    Other(String),
}

impl Kind {
    /// Wire name.
    pub fn as_str(&self) -> &str {
        match self {
            Kind::ToolProvider => "tool-provider",
            Kind::Mode => "mode",
            Kind::ModelProvider => "model-provider",
            Kind::Command => "command",
            Kind::Other(s) => s,
        }
    }

    /// From a wire name.
    pub fn from_name(s: &str) -> Self {
        match s {
            "tool-provider" => Kind::ToolProvider,
            "mode" => Kind::Mode,
            "model-provider" => Kind::ModelProvider,
            "command" => Kind::Command,
            o => Kind::Other(o.into()),
        }
    }
}

/// `p h`: ask to be accepted as a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Hello {
    /// Request id (required).
    pub id: u32,
    /// Name: `a-z 0-9 - _`, at most 64.
    pub name: String,
    /// Kind.
    pub kind: Kind,
    /// Capabilities.
    pub caps: Vec<String>,
    /// Optional `v`.
    pub version: Option<u32>,
}

impl Hello {
    /// A hello.
    pub fn new(id: u32, name: &str, kind: Kind, caps: &[&str]) -> Self {
        Hello { id, name: name.into(), kind, caps: caps.iter().map(|c| c.to_string()).collect(), version: None }
    }

    /// Wire bytes: `id,name,kind,caps[,v]`.
    pub fn encode(&self) -> Vec<u8> {
        let mut p =
            Params::new().with("id", self.id).with("name", &self.name).with("kind", self.kind.as_str()).with("caps", self.caps.join("+"));
        if let Some(v) = self.version {
            p.push("v", v);
        }
        Message::new(b'p', b'h', p).encode()
    }

    /// Read from a `p h` message.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b'p', b'h') {
            return None;
        }
        let p = &m.params;
        Some(Hello {
            id: p.num("id").filter(|&i| i > 0)?,
            name: p.get("name").unwrap_or("").into(),
            kind: p.get("kind").map(Kind::from_name).unwrap_or_default(),
            caps: p.get("caps").unwrap_or("").split('+').filter(|c| !c.is_empty()).map(String::from).collect(),
            version: p.num("v"),
        })
    }
}

/// `p q`: which capabilities this tab granted to `name`.
pub fn query(id: u32, name: &str) -> Vec<u8> {
    Message::new(b'p', b'q', Params::new().with("id", id).with("name", name)).encode()
}

/// `p b`: drop the tab's grant for `name`.
pub fn bye(id: u32, name: &str) -> Vec<u8> {
    Message::new(b'p', b'b', Params::new().with("id", id).with("name", name)).encode()
}

/// The answer to a hello or query: `ok=1,granted=a+b`.
pub fn granted(id: u32, caps: &[String]) -> Vec<u8> {
    Reply::ok(b'p', id, Params::new().with("granted", caps.join("+"))).encode()
}

/// The answer when the user denied: `ok=0,err=EPERM,msg=denied by the user`.
pub fn denied(id: u32) -> Vec<u8> {
    Reply::error(b'p', id, ErrorCode::Permission, "denied by the user").encode()
}

/// Granted capabilities in a `p r` reply (`None` for an error reply).
pub fn granted_caps(r: &Reply) -> Option<Vec<String>> {
    if r.subsystem != b'p' || !r.is_ok() {
        return None;
    }
    Some(r.params.get("granted").unwrap_or("").split('+').filter(|c| !c.is_empty()).map(String::from).collect())
}
