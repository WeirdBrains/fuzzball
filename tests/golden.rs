//! Golden-byte tests: the exact bytes Mouser (`mouser-core` 1.1.x, `fuzzball.rs` and its
//! shell integrations) and the WeirdBrains/crossterm `fuzzball` feature emit and accept
//! today. These lock the 1.x wire format; never change an expected value.

use fuzzball::common::{Capabilities, capability_query, detect_query};
use fuzzball::graphics::{self, Delete, DeleteTarget, Format, GraphicsInfo, Layer, Place, UploadChunk};
use fuzzball::keyboard::{self, ImePhase, Key, KeyEvent, KeyKind, Locks, ModeReply, Modifiers};
use fuzzball::shell::{Advertise, ChangeDir, ShellReply, pct_decode, pct_encode};
use fuzzball::{ErrorCode, Message, Params, Reply, passthrough, plugins};

fn ev(kind: KeyKind, code: &str, key: Key, mods: u8, text: Option<&str>) -> Vec<u8> {
    let mut e = KeyEvent::new(kind, code, key).with_mods(Modifiers::from_bits(mods));
    e.text = text.map(Into::into);
    e.encode()
}

// mouser-core tests/fuzzball_tests.rs `key_event_examples` (keyboard.md §2).
#[test]
fn mouser_key_event_encoding() {
    use KeyKind::*;
    let c = Key::Char;
    let n = |s: &str| Key::Named(s.into());
    assert_eq!(ev(Press, "KeyA", c('a'), 0, Some("a")), b"\x1bP=Fzke;t=p,code=KeyA,key=U+0061,text=61\x1b\\");
    assert_eq!(ev(Press, "KeyA", c('a'), 1, Some("A")), b"\x1bP=Fzke;t=p,code=KeyA,key=U+0061,mods=1,text=41\x1b\\");
    assert_eq!(ev(Release, "KeyA", c('a'), 0, Some("a")), b"\x1bP=Fzke;t=u,code=KeyA,key=U+0061\x1b\\");
    assert_eq!(ev(Press, "KeyC", c('c'), 4, None), b"\x1bP=Fzke;t=p,code=KeyC,key=U+0063,mods=4\x1b\\");
    assert_eq!(ev(Press, "KeyA", c('ф'), 4, None), b"\x1bP=Fzke;t=p,code=KeyA,key=U+0444,mods=4\x1b\\");
    assert_eq!(ev(Press, "Enter", n("Enter"), 0, Some("\r")), b"\x1bP=Fzke;t=p,code=Enter,key=Enter,text=0d\x1b\\");
    assert_eq!(ev(Repeat, "ArrowUp", n("ArrowUp"), 0, None), b"\x1bP=Fzke;t=r,code=ArrowUp,key=ArrowUp\x1b\\");
    assert_eq!(ev(Press, "ShiftLeft", n("Shift"), 1, None), b"\x1bP=Fzke;t=p,code=ShiftLeft,key=Shift,mods=1\x1b\\");
    let k = KeyEvent::new(Press, "KeyA", c('a')).with_locks(Locks::CAPS_LOCK).with_text("A");
    assert_eq!(k.encode(), b"\x1bP=Fzke;t=p,code=KeyA,key=U+0061,locks=1,text=41\x1b\\");
    let crab = String::from_utf8(ev(Press, "KeyA", c('a'), 0, Some("🦀"))).unwrap();
    assert!(crab.contains("text=f09fa680"));
    // Space is a character key in Mouser; non-BMP keys use more than 4 digits.
    assert_eq!(ev(Press, "Space", c(' '), 0, Some(" ")), b"\x1bP=Fzke;t=p,code=Space,key=U+0020,text=20\x1b\\");
    assert_eq!(ev(Press, "KeyA", c('😀'), 0, None), b"\x1bP=Fzke;t=p,code=KeyA,key=U+1F600\x1b\\");
}

// crossterm fork src/event/fuzzball.rs tests (`spec_examples`, `full_fields`, `ime`, framing).
#[test]
fn fork_key_event_parsing() {
    let p = |m: &str| KeyEvent::parse(format!("\x1bP=Fz{m}\x1b\\").as_bytes());
    let e = p("ke;t=p,code=KeyA,key=U+0444,mods=4").unwrap();
    assert_eq!((e.kind, e.code.as_deref(), e.key, e.mods), (KeyKind::Press, Some("KeyA"), Some(Key::Char('ф')), Modifiers::CTRL));
    let e = p("ke;t=p,code=Numpad7,key=U+0037,mods=58,locks=3,text=37").unwrap();
    assert_eq!(e.mods, Modifiers::ALT | Modifiers::SUPER | Modifiers::HYPER | Modifiers::META);
    assert_eq!(e.locks, Locks::CAPS_LOCK | Locks::NUM_LOCK);
    let e = p("ke;t=p,code=KeyQ,key=U+0071,mods=64,text=40").unwrap();
    assert!(e.mods.contains(Modifiers::ALT_GR));
    assert_eq!(e.text.as_deref(), Some("@"));
    assert_eq!(p("ke;t=r,code=ArrowUp,key=ArrowUp").unwrap().key, Some(Key::Named("ArrowUp".into())));
    assert_eq!(p("ke;t=p,code=KeyA,key=U+0061,future=9").unwrap().key, Some(Key::Char('a')), "unknown params ignored");
    assert_eq!(p("ke;t=p,ime=start").unwrap().ime, Some(ImePhase::Start));
    assert_eq!(p("ke;t=p,ime=commit,text=e6bca2e5ad97").unwrap().text.as_deref(), Some("漢字"));
    assert_eq!(p("ke;t=x,key=Enter"), None);
    assert_eq!(p("ke;t=p,key=U+0061,text=6"), None);
    assert_eq!(p("ke;code=KeyA,key=U+0061"), None, "t is required");
    assert_eq!(p("ke;t=p,code=KeyA"), None, "non-IME needs key");
    assert_eq!(keyboard::parse_event(b"\x1bP=Fzke;t=p,code=KeyA", 4096), fuzzball::Parse::Incomplete);
    let mut long = b"\x1bP=Fzke;".to_vec();
    long.resize(4097, b'a');
    assert!(matches!(keyboard::parse_event(&long, 4096), fuzzball::Parse::Invalid { .. }));
}

#[test]
fn keyboard_commands_and_replies() {
    assert_eq!(keyboard::ENABLE, b"\x1bP=Fzks;on=1,v=0\x1b\\");
    assert_eq!(keyboard::DISABLE, b"\x1bP=Fzks;on=0\x1b\\");
    assert_eq!(keyboard::ENABLE_STR.as_bytes(), keyboard::ENABLE);
    assert_eq!(keyboard::DISABLE_STR.as_bytes(), keyboard::DISABLE);
    assert_eq!(keyboard::set_mode(true, None), keyboard::ENABLE);
    assert_eq!(keyboard::set_mode(false, None), keyboard::DISABLE);
    assert_eq!(keyboard::set_mode(true, Some(1)), b"\x1bP=Fzks;on=1,v=0,id=1\x1b\\");
    assert_eq!(keyboard::query(2), b"\x1bP=Fzkq;id=2\x1b\\");
    assert_eq!(ModeReply::new(1, true, true).encode(), b"\x1bP=Fzkr;id=1,ok=1,on=1\x1b\\");
    let m = Message::from_bytes(b"\x1bP=Fzkr;id=1,ok=1,on=1\x1b\\").unwrap();
    assert_eq!(ModeReply::from_message(&m), Some(ModeReply::new(1, true, true)));
}

const MOUSER_CAPS: &str = "v=0,sub=gkps,raw=0,max_chunk=65536,g_mem=335544320,g_max_dim=10000,g_fmt=rgba:rgb:png,g_max_place=4096,g_fit=w,g_reflow=1,k=1,k_ime=0,k_release=1";

#[test]
fn common_capabilities() {
    assert_eq!(capability_query(1), b"\x1bP=Fzcq;id=1\x1b\\");
    assert_eq!(detect_query(1), b"\x1bP=Fzcq;id=1\x1b\\\x1b[c");
    let wire = format!("\x1bP=Fzcr;id=1,{MOUSER_CAPS}\x1b\\");
    let c = Capabilities::from_message(&Message::from_bytes(wire.as_bytes()).unwrap()).unwrap();
    assert_eq!((c.id, c.version, c.subsystems.as_str(), c.raw, c.max_chunk), (1, 0, "gkps", false, Some(65536)));
    assert!(c.keyboard() && c.has_subsystem('s') && c.has_subsystem('g'));
    assert_eq!(c.encode(), wire.as_bytes());
    assert_eq!(Capabilities::new(1, Params::parse(MOUSER_CAPS)).encode(), wire.as_bytes());
    // Fork: no `k` in sub means no keyboard.
    let c = Capabilities::from_message(&Message::from_bytes(b"\x1bP=Fzcr;id=1,v=0,sub=g\x1b\\").unwrap()).unwrap();
    assert!(!c.keyboard());
}

#[test]
fn error_replies() {
    // Mouser: `<sub> r ; id=<n>,ok=0,err=<CODE>,msg=<text>`.
    let r = Reply::error(b'g', 3, ErrorCode::NotFound, "no such image");
    assert_eq!(r.encode(), b"\x1bP=Fzgr;id=3,ok=0,err=ENOENT,msg=no such image\x1b\\");
    let back = Reply::from_message(&Message::from_bytes(&r.encode()).unwrap()).unwrap();
    assert_eq!((back.id, back.is_ok(), back.error_code(), back.msg()), (3, false, Some(ErrorCode::NotFound), Some("no such image")));
    for c in ["ENOTSUP", "EINVAL", "EPROTO", "ENOENT", "ENOMEM", "EPERM", "EVERSION", "EFUTURE"] {
        assert_eq!(ErrorCode::from_code(c).as_str(), c);
    }
    assert_eq!(Reply::ok(b'g', 7, Params::new().with("w", 64).with("h", 64)).encode(), b"\x1bP=Fzgr;id=7,ok=1,w=64,h=64\x1b\\");
}

#[test]
fn graphics_messages() {
    let px = [255, 0, 0, 255, 0, 0, 255, 255];
    let one = graphics::upload(1, Format::Rgba, 2, 1, &px, 65536);
    assert_eq!(one, vec![b"\x1bP=Fzgu;id=1,fmt=rgba,w=2,h=1,size=8,enc=b64,len=12:/wAA/wAA//8=\x1b\\".to_vec()]);
    let c = UploadChunk::from_message(&Message::from_bytes(&one[0]).unwrap()).unwrap();
    assert_eq!(
        (c.id, c.format, c.width, c.height, c.size, c.more, c.data.as_slice()),
        (1, Some(Format::Rgba), Some(2), Some(1), Some(8), false, &px[..])
    );
    // Chunking (README §3.1): 12 bytes at max_chunk=8 → 6 raw bytes (8 b64 chars) each.
    let data: Vec<u8> = (0..12).collect();
    let many = graphics::upload(7, Format::Rgb, 2, 2, &data, 8);
    assert_eq!(many.len(), 2);
    assert_eq!(many[0], b"\x1bP=Fzgu;id=7,fmt=rgb,w=2,h=2,size=12,enc=b64,len=8,more=1:AAECAwQF\x1b\\");
    assert_eq!(many[1], b"\x1bP=Fzgu;id=7,enc=b64,len=8,more=0:BgcICQoL\x1b\\");
    let png = graphics::upload(2, Format::Png, 0, 0, b"\x89PNG", 65536);
    assert_eq!(png[0], b"\x1bP=Fzgu;id=2,fmt=png,size=4,enc=b64,len=8:iVBORw==\x1b\\");
    // Bad len is rejected.
    assert!(UploadChunk::from_message(&Message::from_bytes(b"\x1bP=Fzgu;id=1,fmt=png,size=1,enc=b64,len=3:AA==\x1b\\").unwrap()).is_none());

    assert_eq!(Place::new(1).pid(1).size(4, 2).encode(), b"\x1bP=Fzgp;id=1,pid=1,cols=4,rows=2\x1b\\");
    let full = Place::new(3).pid(2).size(10, 5).src(1, 2, 3, 4).fit_width().layer(Layer::BelowText).move_cursor(false);
    assert_eq!(full.encode(), b"\x1bP=Fzgp;id=3,pid=2,cols=10,rows=5,sx=1,sy=2,sw=3,sh=4,fit=w,z=-1,move=0\x1b\\");
    assert_eq!(Place::from_message(&Message::from_bytes(&full.encode()).unwrap()), Some(full));

    assert_eq!(Delete::new(DeleteTarget::Image { id: 1, pid: None }, true).encode(), b"\x1bP=Fzgd;what=image,id=1,free=1\x1b\\");
    assert_eq!(Delete::new(DeleteTarget::All, false).encode(), b"\x1bP=Fzgd;what=all\x1b\\");
    assert_eq!(Delete::new(DeleteTarget::Cell, false).encode(), b"\x1bP=Fzgd;what=cell\x1b\\");
    assert_eq!(Delete::new(DeleteTarget::Image { id: 4, pid: Some(9) }, false).encode(), b"\x1bP=Fzgd;what=image,id=4,pid=9\x1b\\");

    assert_eq!(graphics::query(1), b"\x1bP=Fzgq;id=1\x1b\\");
    let info = b"\x1bP=Fzgr;id=1,ok=1,mem_used=1048576,mem_max=335544320,images=3,cell_w=9,cell_h=18\x1b\\";
    let i = GraphicsInfo::from_message(&Message::from_bytes(info).unwrap()).unwrap();
    assert_eq!(i, GraphicsInfo::new(1, 1048576, 335544320, 3, 9, 18));
    assert_eq!(i.encode(), info);
}

// Mouser `shell_cd`, `pct_encode` and shell/mouser.{bash,zsh,fish}.
#[test]
fn shell_messages() {
    assert_eq!(ChangeDir::new(7, b"/home/me/a dir").encode(), b"\x1bP=Fzsc;id=7,path=/home/me/a%20dir\x1b\\");
    assert_eq!(pct_encode(b"a,b;c=d\x1b\xff~"), "a%2Cb%3Bc%3Dd%1B%FF~");
    assert_eq!(pct_decode("a%2cb%3B").unwrap(), b"a,b;");
    assert_eq!(pct_decode("%G0"), None);
    assert_eq!(pct_decode("%2"), None);
    assert_eq!(Advertise::new("bash", &["cwd"]).encode(), b"\x1bP=Fzsa;v=0,shell=bash,caps=cwd\x1b\\");
    assert_eq!(Advertise::withdraw().encode(), b"\x1bP=Fzsa;v=0,caps=\x1b\\");
    let a = Advertise::from_message(&Message::from_bytes(b"\x1bP=Fzsa;v=0,caps=cwd,shell=zsh\x1b\\").unwrap()).unwrap();
    assert!(a.has("cwd") && a.shell.as_deref() == Some("zsh"));
    assert!(Advertise::from_message(&Message::from_bytes(b"\x1bP=Fzsa;v=0\x1b\\").unwrap()).unwrap().caps.is_empty());
    assert_eq!(ShellReply::ok(7).encode(), b"\x1bP=Fzsr;id=7,ok=1\x1b\\");
    let e = ShellReply::error(8, ErrorCode::NotFound, b"cannot cd to /x y");
    assert_eq!(e.encode(), b"\x1bP=Fzsr;id=8,ok=0,err=ENOENT,msg=cannot%20cd%20to%20/x%20y\x1b\\");
    assert_eq!(ShellReply::from_message(&Message::from_bytes(&e.encode()).unwrap()), Some(e));
    let c = ChangeDir::from_message(&Message::from_bytes(b"\x1bP=Fzsc;id=7,path=/home/me/a%20dir\x1b\\").unwrap()).unwrap();
    assert_eq!(c.path, b"/home/me/a dir");
}

// Mouser `plugin_reply` and docs/protocol/plugins.md.
#[test]
fn plugin_messages() {
    let h = plugins::Hello::new(1, "hello-fuzzball", plugins::Kind::ToolProvider, &["notify", "screen.read"]);
    assert_eq!(h.encode(), b"\x1bP=Fzph;id=1,name=hello-fuzzball,kind=tool-provider,caps=notify+screen.read\x1b\\");
    assert_eq!(plugins::Hello::from_message(&Message::from_bytes(&h.encode()).unwrap()), Some(h));
    assert_eq!(plugins::granted(7, &["notify".into()]), b"\x1bP=Fzpr;id=7,ok=1,granted=notify\x1b\\");
    assert_eq!(plugins::denied(7), b"\x1bP=Fzpr;id=7,ok=0,err=EPERM,msg=denied by the user\x1b\\");
    assert_eq!(plugins::query(2, "hello-fuzzball"), b"\x1bP=Fzpq;id=2,name=hello-fuzzball\x1b\\");
    assert_eq!(plugins::bye(3, "hello-fuzzball"), b"\x1bP=Fzpb;id=3,name=hello-fuzzball\x1b\\");
    let r = Reply::from_message(&Message::from_bytes(b"\x1bP=Fzpr;id=8,ok=1,granted=\x1b\\").unwrap()).unwrap();
    assert_eq!(plugins::granted_caps(&r), Some(vec![]));
}

#[test]
fn multiplexer_wrapping() {
    let m = b"\x1bP=Fzcq;id=1\x1b\\";
    assert_eq!(passthrough::tmux(m), b"\x1bPtmux;\x1b\x1bP=Fzcq;id=1\x1b\x1b\\\x1b\\");
    assert_eq!(passthrough::screen(m), b"\x1bP\x1bP=Fzcq;id=1\x1b\\\x1b\\");
    let big = vec![b'a'; 1000];
    let s = passthrough::screen(&big);
    assert_eq!(s.len(), 1000 + 8);
    assert!(s.starts_with(b"\x1bP") && s[2 + 768..].starts_with(b"\x1b\\\x1bP"));
}
