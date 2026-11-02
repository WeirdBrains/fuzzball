//! Multiplexer passthrough (README §3.2). Use `enc=b64` (the default) inside these.

/// Wrap for tmux (`allow-passthrough on`): `ESC P tmux ; <msg with ESC doubled> ESC \`.
pub fn tmux(msg: &[u8]) -> Vec<u8> {
    let mut o = b"\x1bPtmux;".to_vec();
    for &b in msg {
        if b == 0x1b {
            o.push(0x1b);
        }
        o.push(b);
    }
    o.extend_from_slice(b"\x1b\\");
    o
}

/// Wrap for GNU screen: the message split into pieces of at most 768 bytes, each sent
/// as `ESC P <piece> ESC \`.
pub fn screen(msg: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(msg.len() + msg.len() / 768 * 4 + 4);
    for piece in msg.chunks(768) {
        o.extend_from_slice(b"\x1bP");
        o.extend_from_slice(piece);
        o.extend_from_slice(b"\x1b\\");
    }
    o
}

/// Wrap for whatever the environment says: tmux when `TMUX` is set, screen when `STY`
/// is set, otherwise unchanged.
pub fn auto(msg: &[u8]) -> Vec<u8> {
    if std::env::var_os("TMUX").is_some() {
        tmux(msg)
    } else if std::env::var_os("STY").is_some() {
        screen(msg)
    } else {
        msg.to_vec()
    }
}
