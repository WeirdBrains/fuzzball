//! Round-trip and streaming tests.

use fuzzball::graphics::{self, Format, UploadChunk, base64_decode, base64_encode};
use fuzzball::keyboard::{ImePhase, Key, KeyEvent, KeyKind, Locks, Modifiers};
use fuzzball::{Decoder, Item, Message, Params, Parse, parse, parse_with_limit};

fn events() -> Vec<KeyEvent> {
    let mut v = Vec::new();
    for kind in [KeyKind::Press, KeyKind::Repeat, KeyKind::Release] {
        for (code, key) in [
            ("KeyA", Key::Char('a')),
            ("KeyA", Key::Char('ф')),
            ("Enter", Key::Named("Enter".into())),
            ("F13", Key::Named("F13".into())),
            ("KeyZ", Key::Char('😀')),
        ] {
            for mods in [0u8, 1, 4, 58, 127, 255] {
                for locks in [0u8, 1, 7] {
                    for text in [None, Some("x"), Some("漢字🦀")] {
                        let mut e = KeyEvent::new(kind, code, key.clone())
                            .with_mods(Modifiers::from_bits(mods))
                            .with_locks(Locks::from_bits(locks));
                        if kind != KeyKind::Release {
                            e.text = text.map(Into::into);
                        }
                        v.push(e);
                    }
                }
            }
        }
    }
    for phase in [ImePhase::Start, ImePhase::Update, ImePhase::Commit, ImePhase::Cancel] {
        v.push(KeyEvent::ime(phase, None));
        v.push(KeyEvent::ime(phase, Some("か".into())));
    }
    v
}

#[test]
fn key_events_round_trip() {
    let evs = events();
    assert!(evs.len() > 700);
    for e in &evs {
        let wire = e.encode();
        assert_eq!(KeyEvent::parse(&wire).as_ref(), Some(e), "{}", String::from_utf8_lossy(&wire));
    }
}

#[test]
fn release_never_carries_text() {
    let e = KeyEvent::new(KeyKind::Release, "KeyA", Key::Char('a')).with_text("a");
    assert_eq!(KeyEvent::parse(&e.encode()).unwrap().text, None);
}

#[test]
fn every_split_point_decodes_the_same() {
    let mut stream = b"hi\x1b[A".to_vec();
    for e in events().iter().take(50) {
        stream.extend_from_slice(&e.encode());
        stream.extend_from_slice(b"z");
    }
    stream.extend_from_slice(b"\x1bP=Fzkr;id=1,ok=1,on=1\x1b\\\x1b");
    let collect = |chunks: &[&[u8]]| {
        let mut d = Decoder::new();
        let (mut msgs, mut text) = (Vec::new(), Vec::new());
        for c in chunks {
            d.push(c);
            while let Some(i) = d.next() {
                match i {
                    Item::Message(m) => msgs.push(m),
                    Item::Text(t) => text.extend(t),
                    Item::Invalid(b) => panic!("invalid {b:?}"),
                    _ => unreachable!(),
                }
            }
        }
        if let Some(Item::Text(t)) = d.flush() {
            text.extend(t);
        }
        (msgs, text)
    };
    let whole = collect(&[&stream]);
    assert_eq!(whole.0.len(), 51);
    assert_eq!(whole.1, [b"hi\x1b[A".as_slice(), &b"z".repeat(50), b"\x1b"].concat());
    for split in 0..stream.len() {
        assert_eq!(collect(&[&stream[..split], &stream[split..]]), whole, "split at {split}");
    }
    let bytes: Vec<&[u8]> = stream.chunks(1).collect();
    assert_eq!(collect(&bytes), whole);
}

#[test]
fn parse_results() {
    assert_eq!(parse(b""), Parse::Incomplete);
    assert_eq!(parse(b"\x1bP="), Parse::Incomplete);
    assert_eq!(parse(b"\x1b[A"), Parse::NotFuzzball);
    assert_eq!(parse(b"abc"), Parse::NotFuzzball);
    assert_eq!(parse(b"\x1bP=Fzkq;id=1"), Parse::Incomplete);
    assert_eq!(parse(b"\x1bP=Fz\x1b\\rest"), Parse::Invalid { len: 7 });
    let Parse::Complete { value, len } = parse(b"\x1bP=Fzkq;id=1\x1b\\tail") else { panic!() };
    assert_eq!((value.subsystem, value.verb, value.params.num("id"), len), (b'k', b'q', Some(1), 14));
    assert!(matches!(parse_with_limit(b"\x1bP=Fzke;t=p,code=KeyAAAAAA", 16), Parse::Invalid { .. }));
    // Raw payload: exactly len bytes, may contain ESC \.
    let raw = b"\x1bP=Fzgu;id=1,fmt=png,size=4,enc=raw,len=4:\x1b\\ab\x1b\\";
    let Parse::Complete { value, len } = parse(raw) else { panic!() };
    assert_eq!((value.payload.as_deref(), len), (Some(&b"\x1b\\ab"[..]), raw.len()));
    let c = UploadChunk::from_message(&value).unwrap();
    assert_eq!(c.data, b"\x1b\\ab");
    assert_eq!(parse(&raw[..raw.len() - 1]), Parse::Incomplete);
    assert_eq!(parse(b"\x1bP=Fzgu;enc=raw,len=2:abXY"), Parse::Invalid { len: 24 }, "resume after the payload");
}

#[test]
fn messages_round_trip() {
    let m = Message::with_payload(b'g', b'u', Params::new().with("id", 1).with("len", 4), b"AAAA".to_vec());
    assert_eq!(Message::from_bytes(&m.encode()), Some(m.clone()));
    assert_eq!(m.encode_string(), "\x1bP=Fzgu;id=1,len=4:AAAA\x1b\\");
    let p = Params::parse("a=1,junk,b=,a=2");
    assert_eq!((p.get("a"), p.get("b"), p.get("junk")), (Some("2"), Some(""), None));
    assert_eq!(Params::parse("n=12345678901").num("n"), None);
    assert_eq!(Params::parse("n=+1").num("n"), None);
}

#[test]
fn base64_and_uploads_round_trip() {
    for n in 0..200usize {
        let data: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
        let e = base64_encode(&data);
        assert_eq!(base64_decode(e.as_bytes()).unwrap(), data);
        assert_eq!(base64_decode(e.trim_end_matches('=').as_bytes()).unwrap(), data);
        for max in [4usize, 8, 64, 65536] {
            let mut out = Vec::new();
            for (i, w) in graphics::upload(9, Format::Rgba, 0, 0, &data, max).iter().enumerate() {
                let c = UploadChunk::from_message(&Message::from_bytes(w).unwrap()).unwrap();
                assert!(c.data.len() <= max.max(4) / 4 * 3);
                assert_eq!(c.format.is_some(), i == 0);
                out.extend(c.data);
            }
            assert_eq!(out, data);
        }
    }
    assert_eq!(base64_decode(b"A"), None);
    assert_eq!(base64_decode(b"A*=="), None);
}
