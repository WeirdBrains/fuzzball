# Wire format as implemented (fuzzball 1.0.0)

The documents in this directory are the v0.1 drafts moved from Mouser
(`docs/protocol/`). The wire version is still `v=0`. This file records what Mouser 1.1.x
and the WeirdBrains/crossterm `fuzzball` feature actually emit and accept, where that is
more specific than, or differs from, the drafts. The `fuzzball` crate follows this file,
and `tests/golden.rs` locks it.

## Framing

1. **`;` is always written**, even when a message has no params.
2. **Payload separator.** A `:` starts a payload only when the params before it contain
   `len`. Every payload-taking verb requires `len`. Values may contain `:`: the
   capability reply's `g_fmt=rgba:rgb:png` does. (Mouser's terminal-side parser splits
   app→terminal messages at the first `:`. That never matters for valid messages,
   because no app→terminal value contains `:`.)
3. **Raw payload framing error.** When `ST` does not follow an `enc=raw` payload of `len`
   bytes, the bytes up to the end of the payload are skipped and parsing resumes there.
4. **Duplicate params:** the last one wins. **Items without `=`** are ignored.
5. **Numbers:** unsigned decimal, 1–10 digits, no sign. Anything else counts as absent.
6. **Message limits:** Mouser keeps up to `max_chunk + 1024` = 66 560 bytes. The
   crossterm fork drops input messages over 4096 bytes.

## Common

7. **The capability reply has no `ok=`**: `c r ; id=<n>,v=0,sub=gkps,raw=0,...`. Mouser
   always reports `raw=0` (the README §4 example shows `raw=1`) and `sub=gkps`.
8. **Every error reply carries `msg=`**: `id=<n>,ok=0,err=<CODE>,msg=<text>`. In `p r`
   the text is plain ASCII with spaces (`msg=denied by the user`). In `s r` it is
   percent-encoded.

## Keyboard (differences from the v0 `keyboard.md` draft)

9. **`key=U+XXXX`** uses uppercase hex and at least 4 digits, with more for non-BMP
   characters (`U+1F600`). Decoders accept either case.
10. **`text=`** is lowercase hex UTF-8. Decoders accept either case. Odd length or
    invalid UTF-8 makes the event invalid, so it is dropped.
11. **Text is omitted** on releases, and on presses or repeats while Ctrl, Alt or Super
    is held (shortcuts). The draft only implies this through its Ctrl+c example.
12. **Named keys are winit `NamedKey` names.** These match W3C `key` names for common
    keys, with these exceptions:
    - The Windows/Command key is `key=Super` (W3C says `Meta`).
    - **Space is a character**: `key=U+0020,text=20`, not a named key.
    - The fork also accepts `OS` for Super, and `Clear`.
13. **Modifier bits:** Mouser sets bit 8 (SUPER) for both the Super and the Meta key, and
    never sends bit 32 (META). Bits 16 (HYPER) and 64 (ALTGR) are set only on the Hyper
    and AltGraph keys' own events.
14. **`locks` is never sent** by Mouser (always 0), and **no IME messages** are sent
    (`k_ime=0`). Both are decoded when present. The crossterm fork consumes
    `start`/`update`/`cancel` and delivers multi-character commits as paste.
15. **`k s` replies too.** With an `id`, `k s` is answered `k r ; id=<n>,ok=1,on=<0|1>`,
    the same shape as `k q`. A bad `on` value gets `EINVAL`, and `v` ≠ 0 gets `EVERSION`.
16. **Field order** written: `t, code, key, mods, locks, ime, text`. IME messages carry no
    `code` or `key`. Parsers must not depend on the order.
17. **Validity rules** (the fork's, kept by the crate):
    - `t` is required.
    - A non-IME event needs a non-empty `key`.
    - A bad `t`, `mods`, `locks`, `text` or `ime` makes the whole event invalid.
    - Unknown params are ignored.
18. The enable command is exactly `ESC P = F z k s ; on=1,v=0 ESC \` (no id), and disable
    is `ESC P = F z k s ; on=0 ESC \`.

## Graphics

19. **Chunk `more`:**
    - A single-chunk upload sends no `more`.
    - A multi-chunk upload sends `more=1` on every chunk but the last, which sends `more=0`.
    - The first chunk writes params in the order `id,fmt,w,h,size,enc,len[,more]`.
    - Continuation chunks write `id,enc,len,more`.
20. **Place** params are written in the order
    `id,pid,cols,rows,sx,sy,sw,sh,fit,z,move`. **Delete** params are written as
    `what,id,pid,free`.

## Shell

21. **Advertise** params are written as `v=0,shell=<name>,caps=<a+b>`, and withdraw is
    `v=0,caps=`. Mouser accepts any order.
