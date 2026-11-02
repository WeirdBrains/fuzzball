//! Message framing: [`Message`], [`Params`], one-shot [`parse`] and the streaming
//! [`Decoder`].

use crate::{DEFAULT_MAX_MESSAGE, PREFIX, ST};

/// Ordered `key=value` parameters of a message.
///
/// Order is kept so encoding is byte-exact. Lookups return the **last** value for a key,
/// as Mouser does. Items without `=` are dropped on parse.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Params(Vec<(String, String)>);

impl Params {
    /// No parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse `k=v,k=v`. Items without `=` are ignored.
    pub fn parse(s: &str) -> Self {
        Params(s.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    /// Append a parameter (builder style).
    pub fn with(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.push(key, value);
        self
    }

    /// Append a parameter.
    pub fn push(&mut self, key: impl Into<String>, value: impl ToString) {
        self.0.push((key.into(), value.to_string()));
    }

    /// The last value for `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// `key` as an unsigned decimal of at most 10 digits (no sign), as the spec requires.
    pub fn num(&self, key: &str) -> Option<u32> {
        let v = self.get(key)?;
        if v.is_empty() || v.len() > 10 || !v.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        v.parse().ok()
    }

    /// Whether there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over `(key, value)` in wire order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Encode as `k=v,k=v`.
    pub fn encode(&self) -> String {
        let mut s = String::new();
        for (i, (k, v)) in self.0.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(k);
            s.push('=');
            s.push_str(v);
        }
        s
    }
}

impl<K: Into<String>, V: ToString> FromIterator<(K, V)> for Params {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Params(iter.into_iter().map(|(k, v)| (k.into(), v.to_string())).collect())
    }
}

/// One Fuzzball message: subsystem letter, verb letter, params and optional payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Message {
    /// Subsystem letter (`c`, `g`, `k`, `p`, `s`).
    pub subsystem: u8,
    /// Verb letter.
    pub verb: u8,
    /// Parameters.
    pub params: Params,
    /// Payload bytes after `:` exactly as on the wire (still base64 for `enc=b64`).
    pub payload: Option<Vec<u8>>,
}

impl Message {
    /// A message without payload.
    pub fn new(subsystem: u8, verb: u8, params: Params) -> Self {
        Message { subsystem, verb, params, payload: None }
    }

    /// A message with a payload (wire bytes).
    pub fn with_payload(subsystem: u8, verb: u8, params: Params, payload: Vec<u8>) -> Self {
        Message { subsystem, verb, params, payload: Some(payload) }
    }

    /// Whether this is `<subsystem><verb>`, e.g. `msg.is(b'k', b'e')`.
    pub fn is(&self, subsystem: u8, verb: u8) -> bool {
        self.subsystem == subsystem && self.verb == verb
    }

    /// Full wire bytes: `ESC P = F z <sub><verb>;<params>[:<payload>] ESC \`.
    ///
    /// The `;` is always written, as Mouser does.
    pub fn encode(&self) -> Vec<u8> {
        let mut o = PREFIX.to_vec();
        o.push(self.subsystem);
        o.push(self.verb);
        o.push(b';');
        o.extend_from_slice(self.params.encode().as_bytes());
        if let Some(p) = &self.payload {
            o.push(b':');
            o.extend_from_slice(p);
        }
        o.extend_from_slice(ST);
        o
    }

    /// [`encode`](Self::encode) as a `String` (lossy only for non-UTF-8 raw payloads).
    pub fn encode_string(&self) -> String {
        String::from_utf8_lossy(&self.encode()).into_owned()
    }

    /// Parse a message body: the bytes between `ESC P = F z` and `ESC \`.
    pub fn from_body(body: &[u8]) -> Option<Self> {
        if body.len() < 2 || !body[0].is_ascii_alphabetic() || !body[1].is_ascii_alphabetic() {
            return None;
        }
        let rest = &body[2..];
        let rest = rest.strip_prefix(b";").unwrap_or(rest);
        // A `:` starts a payload only when the params before it carry `len` (every
        // payload-taking verb requires it); otherwise `:` is part of a value, as in the
        // capability reply's `g_fmt=rgba:rgb:png`.
        if let Some(i) = rest.iter().position(|&b| b == b':') {
            let head = Params::parse(std::str::from_utf8(&rest[..i]).ok()?);
            if head.get("len").is_some() {
                return Some(Message { subsystem: body[0], verb: body[1], params: head, payload: Some(rest[i + 1..].to_vec()) });
            }
        }
        let params = Params::parse(std::str::from_utf8(rest).ok()?);
        let payload = None;
        Some(Message { subsystem: body[0], verb: body[1], params, payload })
    }

    /// Parse one complete framed message (prefix and `ST` required).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match parse(bytes) {
            Parse::Complete { value, len } if len == bytes.len() => Some(value),
            _ => None,
        }
    }
}

/// Result of parsing the start of a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Parse<T> {
    /// A complete value taking the first `len` bytes.
    Complete {
        /// The parsed value.
        value: T,
        /// Bytes consumed, including the framing.
        len: usize,
    },
    /// The buffer is a Fuzzball message (or a prefix of one) that has not ended yet.
    Incomplete,
    /// A framed Fuzzball message of `len` bytes that is malformed or too long; skip it.
    Invalid {
        /// Bytes to skip.
        len: usize,
    },
    /// The buffer does not start with `ESC P = F z`.
    NotFuzzball,
}

impl<T> Parse<T> {
    /// Map the value of a complete parse.
    pub fn map<U>(self, f: impl FnOnce(T) -> Option<U>) -> Parse<U> {
        match self {
            Parse::Complete { value, len } => match f(value) {
                Some(value) => Parse::Complete { value, len },
                None => Parse::Invalid { len },
            },
            Parse::Incomplete => Parse::Incomplete,
            Parse::Invalid { len } => Parse::Invalid { len },
            Parse::NotFuzzball => Parse::NotFuzzball,
        }
    }
}

/// Parse a message at the start of `buf` with [`DEFAULT_MAX_MESSAGE`].
pub fn parse(buf: &[u8]) -> Parse<Message> {
    parse_with_limit(buf, DEFAULT_MAX_MESSAGE)
}

/// Parse a message at the start of `buf`; messages longer than `max` bytes are
/// [`Parse::Invalid`].
///
/// With `enc=raw,len=<n>` the payload is read as exactly `n` bytes (it may contain
/// `ESC \`), then `ST` must follow.
pub fn parse_with_limit(buf: &[u8], max: usize) -> Parse<Message> {
    if buf.len() < PREFIX.len() {
        return if PREFIX.starts_with(buf) { Parse::Incomplete } else { Parse::NotFuzzball };
    }
    if !buf.starts_with(PREFIX) {
        return Parse::NotFuzzball;
    }
    let body_start = PREFIX.len();
    // Header: up to the first ':' or ST.
    let st = find(buf, body_start, ST);
    let colon = buf[body_start..].iter().position(|&b| b == b':').map(|i| i + body_start);
    if let Some(c) = colon.filter(|&c| st.is_none_or(|s| c < s)) {
        let head_start = (body_start + 2).min(c);
        let head = &buf[head_start..c];
        let head = Params::parse(&String::from_utf8_lossy(head.strip_prefix(b";").unwrap_or(head)));
        if head.get("enc") == Some("raw") {
            let Some(n) = head.num("len").map(|n| n as usize) else {
                return invalid_until_st(buf, st, max);
            };
            let end = c + 1 + n;
            if end + 2 > max {
                return Parse::Invalid { len: buf.len().min(end + 2) };
            }
            if buf.len() < end + 2 {
                return Parse::Incomplete;
            }
            if &buf[end..end + 2] != ST {
                return Parse::Invalid { len: end };
            }
            return match Message::from_body(&buf[body_start..end]) {
                Some(value) => Parse::Complete { value, len: end + 2 },
                None => Parse::Invalid { len: end + 2 },
            };
        }
    }
    match st {
        Some(s) if s + 2 > max => Parse::Invalid { len: s + 2 },
        Some(s) => match Message::from_body(&buf[body_start..s]) {
            Some(value) => Parse::Complete { value, len: s + 2 },
            None => Parse::Invalid { len: s + 2 },
        },
        None if buf.len() > max => Parse::Invalid { len: buf.len() },
        None => Parse::Incomplete,
    }
}

fn invalid_until_st(buf: &[u8], st: Option<usize>, max: usize) -> Parse<Message> {
    match st {
        Some(s) => Parse::Invalid { len: s + 2 },
        None if buf.len() > max => Parse::Invalid { len: buf.len() },
        None => Parse::Incomplete,
    }
}

fn find(buf: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    buf.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|i| i + from)
}

/// One piece of input from a [`Decoder`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Item {
    /// A Fuzzball message.
    Message(Message),
    /// Bytes that are not part of a Fuzzball message (pass to your other parser).
    Text(Vec<u8>),
    /// A malformed or over-long Fuzzball message that was skipped.
    Invalid(Vec<u8>),
}

/// Streaming decoder: feed bytes as they arrive, take [`Item`]s out.
///
/// A message split across reads is held back until it is complete. A trailing `ESC`
/// (or any prefix of `ESC P = F z`) is held back too, so call [`flush`](Self::flush)
/// when input goes idle to get it as text.
#[derive(Debug, Clone)]
pub struct Decoder {
    buf: Vec<u8>,
    max: usize,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    /// A decoder with [`DEFAULT_MAX_MESSAGE`].
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_MAX_MESSAGE)
    }

    /// A decoder that skips messages longer than `max` bytes.
    pub fn with_limit(max: usize) -> Self {
        Decoder { buf: Vec::new(), max }
    }

    /// Append input.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Bytes buffered but not yet returned.
    pub fn pending(&self) -> &[u8] {
        &self.buf
    }

    /// Return everything still buffered as text and clear the buffer.
    pub fn flush(&mut self) -> Option<Item> {
        (!self.buf.is_empty()).then(|| Item::Text(std::mem::take(&mut self.buf)))
    }

    /// Take the next complete item, if any.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Item> {
        if self.buf.is_empty() {
            return None;
        }
        match parse_with_limit(&self.buf, self.max) {
            Parse::Complete { value, len } => {
                self.buf.drain(..len);
                Some(Item::Message(value))
            }
            Parse::Invalid { len } => Some(Item::Invalid(self.buf.drain(..len).collect())),
            Parse::Incomplete => None,
            Parse::NotFuzzball => {
                // Text up to the next ESC that could start a message.
                let mut end = self.buf.len();
                for i in 1..self.buf.len() {
                    if self.buf[i] == 0x1b {
                        let rest = &self.buf[i..];
                        if rest.starts_with(PREFIX) || PREFIX.starts_with(rest) {
                            end = i;
                            break;
                        }
                    }
                }
                Some(Item::Text(self.buf.drain(..end).collect()))
            }
        }
    }
}
