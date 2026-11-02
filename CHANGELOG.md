# Changelog

All notable changes to this crate are documented here. The crate follows
[Semantic Versioning](https://semver.org/); the wire format is part of the contract.

## 1.0.0 (2026-11-02)

First stable release, extracted from Mouser (`mouser-core/src/fuzzball.rs`) and the
WeirdBrains/crossterm `fuzzball` feature.

- Framing: `Message`, ordered `Params`, `parse` / `parse_with_limit` with partial-input
  results, streaming `Decoder`, raw (`enc=raw,len=n`) payloads.
- Common: capability query, DA1 detection sequence, `Capabilities`, `Reply`, `ErrorCode`.
- Keyboard: `KeyEvent` encode/decode (keys, modifiers, locks, text, IME), `parse_event`,
  `ENABLE` / `DISABLE`, `set_mode`, `query`, `ModeReply`.
- Graphics: chunked `upload`, `UploadChunk`, `Place`, `Delete`, `query`, `GraphicsInfo`,
  base64.
- Shell: `Advertise`, `ChangeDir`, `ShellReply`, percent-encoding.
- Plugins: `Hello`, `query`, `bye`, `granted`, `denied`.
- Passthrough: tmux and GNU screen wrapping.
- Spec moved from Mouser `docs/protocol/` to `docs/`; `docs/wire-format.md` added.
