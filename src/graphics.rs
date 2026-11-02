//! Graphics subsystem (`g`): upload, place, delete, query.
//!
//! ```
//! use fuzzball::graphics::{Format, Place, upload};
//!
//! // A 2x1 RGBA image (red, blue), one chunk.
//! let msgs = upload(1, Format::Rgba, 2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], 65536);
//! assert_eq!(msgs[0], b"\x1bP=Fzgu;id=1,fmt=rgba,w=2,h=1,size=8,enc=b64,len=12:/wAA/wAA//8=\x1b\\");
//! let place = Place::new(1).pid(1).size(4, 2).encode();
//! assert_eq!(place, b"\x1bP=Fzgp;id=1,pid=1,cols=4,rows=2\x1b\\");
//! ```

use crate::message::{Message, Params};

/// Pixel format (`fmt=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Format {
    /// 8-bit RGBA, straight alpha.
    Rgba,
    /// 8-bit RGB.
    Rgb,
    /// PNG file data.
    Png,
}

impl Format {
    /// Wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Rgba => "rgba",
            Format::Rgb => "rgb",
            Format::Png => "png",
        }
    }

    /// From a wire name.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "rgba" => Format::Rgba,
            "rgb" => Format::Rgb,
            "png" => Format::Png,
            _ => return None,
        })
    }
}

/// Payload encoding (`enc=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Encoding {
    /// Standard base64 (default).
    #[default]
    Base64,
    /// Raw bytes (only after the terminal advertised `raw=1`; Mouser never does).
    Raw,
}

impl Encoding {
    /// Wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Encoding::Base64 => "b64",
            Encoding::Raw => "raw",
        }
    }
}

/// Upload an image as one or more `g u` messages, each with at most `max_chunk` payload
/// bytes on the wire (base64). Single-chunk uploads carry no `more`; multi-chunk uploads
/// send `more=1` on every chunk but the last, which sends `more=0`. Continuation chunks
/// repeat only `id`, `enc`, `len`, `more`. `w`/`h` are omitted when 0 (use 0 for PNG).
pub fn upload(id: u32, format: Format, width: u32, height: u32, data: &[u8], max_chunk: usize) -> Vec<Vec<u8>> {
    let per = (max_chunk / 4 * 3).max(3);
    let pieces: Vec<&[u8]> = if data.is_empty() { vec![&[][..]] } else { data.chunks(per).collect() };
    let n = pieces.len();
    pieces
        .into_iter()
        .enumerate()
        .map(|(i, piece)| {
            let b64 = base64_encode(piece);
            let mut p = Params::new().with("id", id);
            if i == 0 {
                p.push("fmt", format.as_str());
                if width != 0 {
                    p.push("w", width);
                }
                if height != 0 {
                    p.push("h", height);
                }
                p.push("size", data.len());
            }
            p.push("enc", "b64");
            p.push("len", b64.len());
            if n > 1 {
                p.push("more", u8::from(i + 1 < n));
            }
            Message::with_payload(b'g', b'u', p, b64.into_bytes()).encode()
        })
        .collect()
}

/// One decoded `g u` chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UploadChunk {
    /// Image id.
    pub id: u32,
    /// `fmt` (first chunk only).
    pub format: Option<Format>,
    /// `w`, `h` (first chunk, raw formats).
    pub width: Option<u32>,
    /// See `width`.
    pub height: Option<u32>,
    /// Total decoded size (first chunk).
    pub size: Option<u32>,
    /// `enc`.
    pub encoding: Encoding,
    /// `more=1`.
    pub more: bool,
    /// Decoded payload bytes of this chunk.
    pub data: Vec<u8>,
}

impl UploadChunk {
    /// Read from a `g u` message: checks `len` against the payload and decodes base64.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b'g', b'u') {
            return None;
        }
        let p = &m.params;
        let payload = m.payload.as_deref()?;
        if p.num("len")? as usize != payload.len() {
            return None;
        }
        let encoding = match p.get("enc").unwrap_or("b64") {
            "b64" => Encoding::Base64,
            "raw" => Encoding::Raw,
            _ => return None,
        };
        let data = match encoding {
            Encoding::Base64 => base64_decode(payload)?,
            Encoding::Raw => payload.to_vec(),
        };
        Some(UploadChunk {
            id: p.num("id").filter(|&i| i > 0)?,
            format: match p.get("fmt") {
                Some(f) => Some(Format::from_name(f)?),
                None => None,
            },
            width: p.num("w"),
            height: p.num("h"),
            size: p.num("size"),
            encoding,
            more: p.num("more") == Some(1),
            data,
        })
    }
}

/// Stacking (`z=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Layer {
    /// `-1`: behind text, above cell backgrounds.
    BelowText,
    /// `0` (default): above text.
    AboveText,
    /// `1`: above text and cursor.
    AboveCursor,
}

impl Layer {
    fn value(self) -> &'static str {
        match self {
            Layer::BelowText => "-1",
            Layer::AboveText => "0",
            Layer::AboveCursor => "1",
        }
    }
}

/// Place an image at the cursor cell (`g p`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Place {
    /// Image id.
    pub id: u32,
    /// Placement id.
    pub pid: Option<u32>,
    /// Width in cells.
    pub cols: Option<u32>,
    /// Height in cells.
    pub rows: Option<u32>,
    /// Source rectangle `(sx, sy, sw, sh)` in pixels.
    pub src: Option<(u32, u32, u32, u32)>,
    /// `fit=w`: fill the width.
    pub fit_width: bool,
    /// `z`.
    pub layer: Option<Layer>,
    /// `move` (`Some(false)` leaves the cursor).
    pub move_cursor: Option<bool>,
}

impl Place {
    /// Place image `id`.
    pub fn new(id: u32) -> Self {
        Place { id, ..Default::default() }
    }
    /// Set `pid`.
    pub fn pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }
    /// Set `cols` and `rows`.
    pub fn size(mut self, cols: u32, rows: u32) -> Self {
        self.cols = Some(cols);
        self.rows = Some(rows);
        self
    }
    /// Set the source rectangle.
    pub fn src(mut self, sx: u32, sy: u32, sw: u32, sh: u32) -> Self {
        self.src = Some((sx, sy, sw, sh));
        self
    }
    /// Set `fit=w`.
    pub fn fit_width(mut self) -> Self {
        self.fit_width = true;
        self
    }
    /// Set `z`.
    pub fn layer(mut self, l: Layer) -> Self {
        self.layer = Some(l);
        self
    }
    /// Set `move`.
    pub fn move_cursor(mut self, m: bool) -> Self {
        self.move_cursor = Some(m);
        self
    }

    /// The message: `id,pid,cols,rows,sx,sy,sw,sh,fit,z,move`.
    pub fn to_message(&self) -> Message {
        let mut p = Params::new().with("id", self.id);
        let opt = |p: &mut Params, k: &str, v: Option<u32>| {
            if let Some(v) = v {
                p.push(k, v)
            }
        };
        opt(&mut p, "pid", self.pid);
        opt(&mut p, "cols", self.cols);
        opt(&mut p, "rows", self.rows);
        if let Some((sx, sy, sw, sh)) = self.src {
            p.push("sx", sx);
            p.push("sy", sy);
            p.push("sw", sw);
            p.push("sh", sh);
        }
        if self.fit_width {
            p.push("fit", "w");
        }
        if let Some(l) = self.layer {
            p.push("z", l.value());
        }
        if let Some(m) = self.move_cursor {
            p.push("move", u8::from(m));
        }
        Message::new(b'g', b'p', p)
    }

    /// Wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        self.to_message().encode()
    }

    /// Read from a `g p` message.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b'g', b'p') {
            return None;
        }
        let p = &m.params;
        let src = match (p.num("sx"), p.num("sy"), p.num("sw"), p.num("sh")) {
            (None, None, None, None) => None,
            (a, b, c, d) => Some((a.unwrap_or(0), b.unwrap_or(0), c.unwrap_or(u32::MAX), d.unwrap_or(u32::MAX))),
        };
        Some(Place {
            id: p.num("id").filter(|&i| i > 0)?,
            pid: p.num("pid"),
            cols: p.num("cols"),
            rows: p.num("rows"),
            src,
            fit_width: match p.get("fit") {
                None => false,
                Some("w") => true,
                Some(_) => return None,
            },
            layer: match p.get("z") {
                None => None,
                Some("-1") => Some(Layer::BelowText),
                Some("0") => Some(Layer::AboveText),
                Some("1") => Some(Layer::AboveCursor),
                Some(_) => return None,
            },
            move_cursor: p.num("move").map(|m| m == 1),
        })
    }
}

/// What a delete removes (`what=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeleteTarget {
    /// All placements on the current screen.
    All,
    /// Placements of image `id` (all, or just `pid`).
    Image {
        /// Image id.
        id: u32,
        /// Placement id.
        pid: Option<u32>,
    },
    /// Placements covering the cursor cell.
    Cell,
}

/// Delete placements (`g d`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Delete {
    /// What to delete.
    pub target: DeleteTarget,
    /// `free=1`: also free image data left without placements.
    pub free: bool,
}

impl Delete {
    /// A delete.
    pub fn new(target: DeleteTarget, free: bool) -> Self {
        Delete { target, free }
    }

    /// The message: `what,id,pid,free`.
    pub fn to_message(&self) -> Message {
        let mut p = Params::new();
        match self.target {
            DeleteTarget::All => p.push("what", "all"),
            DeleteTarget::Cell => p.push("what", "cell"),
            DeleteTarget::Image { id, pid } => {
                p.push("what", "image");
                p.push("id", id);
                if let Some(pid) = pid {
                    p.push("pid", pid);
                }
            }
        }
        if self.free {
            p.push("free", 1);
        }
        Message::new(b'g', b'd', p)
    }

    /// Wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        self.to_message().encode()
    }

    /// Read from a `g d` message.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b'g', b'd') {
            return None;
        }
        let p = &m.params;
        let target = match p.get("what")? {
            "all" => DeleteTarget::All,
            "cell" => DeleteTarget::Cell,
            "image" => DeleteTarget::Image { id: p.num("id")?, pid: p.num("pid") },
            _ => return None,
        };
        Some(Delete { target, free: p.num("free") == Some(1) })
    }
}

/// Graphics query: `ESC P = F z g q ; id=<id> ESC \`.
pub fn query(id: u32) -> Vec<u8> {
    Message::new(b'g', b'q', Params::new().with("id", id)).encode()
}

/// Reply to the graphics query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct GraphicsInfo {
    /// Reply id.
    pub id: u32,
    /// Bytes of image data held.
    pub mem_used: u64,
    /// Limit.
    pub mem_max: u64,
    /// Number of images.
    pub images: u32,
    /// Cell width in pixels.
    pub cell_w: u32,
    /// Cell height in pixels.
    pub cell_h: u32,
}

impl GraphicsInfo {
    /// Info.
    pub fn new(id: u32, mem_used: u64, mem_max: u64, images: u32, cell_w: u32, cell_h: u32) -> Self {
        GraphicsInfo { id, mem_used, mem_max, images, cell_w, cell_h }
    }

    /// Read from a successful `g r` reply to a query.
    pub fn from_message(m: &Message) -> Option<Self> {
        if !m.is(b'g', b'r') || m.params.get("ok") != Some("1") {
            return None;
        }
        let p = &m.params;
        let big = |k: &str| p.get(k).and_then(|v| v.parse::<u64>().ok());
        Some(GraphicsInfo {
            id: p.num("id").unwrap_or(0),
            mem_used: big("mem_used")?,
            mem_max: big("mem_max")?,
            images: p.num("images")?,
            cell_w: p.num("cell_w")?,
            cell_h: p.num("cell_h")?,
        })
    }

    /// Wire bytes in Mouser's order.
    pub fn encode(&self) -> Vec<u8> {
        let p = Params::new()
            .with("id", self.id)
            .with("ok", 1)
            .with("mem_used", self.mem_used)
            .with("mem_max", self.mem_max)
            .with("images", self.images)
            .with("cell_w", self.cell_w)
            .with("cell_h", self.cell_h);
        Message::new(b'g', b'r', p).encode()
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (RFC 4648 §4).
pub fn base64_encode(data: &[u8]) -> String {
    let mut o = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        o.push(B64[(n >> 18) as usize & 63] as char);
        o.push(B64[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    o
}

/// Decode standard base64; padding optional. `None` on any other character.
pub fn base64_decode(s: &[u8]) -> Option<Vec<u8>> {
    let s = s.strip_suffix(b"==").or_else(|| s.strip_suffix(b"=")).unwrap_or(s);
    if s.len() % 4 == 1 {
        return None;
    }
    let mut o = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in s {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            o.push((acc >> bits) as u8);
        }
    }
    Some(o)
}
