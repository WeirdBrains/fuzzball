import 'package:jaspr/dom.dart';
import 'package:jaspr/jaspr.dart';

import '../content/html.dart';
import '../content/shots.dart';

// Every snippet here compiles against fuzzball 1.0.0, and every "on the wire"
// block is the exact output of that code (ESC = 0x1b).

String _ex(String id, String title, String intro, List<String> blocks) =>
    '<section class="ex" id="$id"><h2><a href="examples#$id">#</a> $title</h2><p>$intro</p>${blocks.join()}${shotFor(id)}</section>';

final _html = [
  _ex('keyboard', 'Keyboard', 'Turn the mode on, then feed input to a <code>Decoder</code> as it arrives. Partial messages are held until complete; everything else comes back as <code>Item::Text</code> for your normal input parser.', [
    codeBlock(r'''
use fuzzball::keyboard::{self, KeyEvent};
use fuzzball::{Decoder, Item};

// Turn the mode on (write this to the terminal).
let enable: &[u8] = keyboard::ENABLE;

let mut d = Decoder::new();
d.push(b"\x1bP=Fzke;t=p,code=KeyA,key=U+0061,text=61\x1b\\");
while let Some(item) = d.next() {
    match item {
        Item::Message(m) => {
            if let Some(ev) = KeyEvent::from_message(&m) {
                println!("{:?} {:?} {:?}", ev.kind, ev.key, ev.text);
            }
        }
        Item::Text(other) => { /* hand to your normal input parser */ let _ = other; }
        _ => {}
    }
}'''),
    wire(r'ESC P=Fzks;on=1,v=0ESC \', label: 'keyboard::ENABLE'),
    wire(r'ESC P=Fzke;t=p,code=KeyA,key=U+0061,text=61ESC \', label: 'a key press from the terminal'),
    '<p>Parsers that manage their own buffer (such as crossterm\'s) can use <code>keyboard::parse_event(buf, max)</code>, which parses one event at the start of the buffer and reports <code>Incomplete</code>, <code>Invalid { len }</code> or <code>NotFuzzball</code>. <code>keyboard::ENABLE_STR</code> and <code>DISABLE_STR</code> are the same commands as <code>&amp;str</code>.</p>',
    codeBlock(r'''
use fuzzball::keyboard;
use fuzzball::Parse;

let buf = b"\x1bP=Fzke;t=p,code=KeyA,key=U+0061,text=61\x1b\\";
match keyboard::parse_event(buf, 4096) {
    Parse::Complete { value, len } => println!("{value:?} ({len} bytes)"),
    Parse::Incomplete => { /* wait for more input */ }
    Parse::Invalid { len } => { /* skip len bytes */ let _ = len; }
    Parse::NotFuzzball => { /* not ours */ }
    _ => {} // Parse is #[non_exhaustive]
}'''),
    wire('KeyEvent { kind: Press, code: Some("KeyA"), key: Some(Char(\'a\')), mods: Modifiers(0), locks: Locks(0), text: Some("a"), ime: None } (42 bytes)', label: 'output'),
  ]),
  _ex('graphics', 'Graphics', 'Upload an image once, then place it at the cursor cell as often as you like. <code>upload</code> splits the base64 payload into chunks of at most <code>max_chunk</code> bytes.', [
    codeBlock(r'''
use fuzzball::graphics::{self, Format, Layer, Place};

// A 2×1 RGBA image: one red pixel, one green pixel.
let px = [255u8, 0, 0, 255, 0, 255, 0, 255];
for chunk in graphics::upload(1, Format::Rgba, 2, 1, &px, fuzzball::MAX_CHUNK) {
    out.write_all(&chunk)?;
}

// Show it 20 columns by 10 rows, behind the text.
out.write_all(&Place::new(1).size(20, 10).layer(Layer::BelowText).encode())?;'''),
    wire(r'ESC P=Fzgu;id=1,fmt=rgba,w=2,h=1,size=8,enc=b64,len=12:/wAA/wD/AP8=ESC \', label: 'upload'),
    wire(r'ESC P=Fzgp;id=1,cols=20,rows=10,z=-1ESC \', label: 'place'),
    '<p><code>graphics::query(id)</code> asks for memory use, image count and cell size (<code>GraphicsInfo</code>); <code>Delete</code> removes placements or images.</p>',
  ]),
  _ex('shell', 'Shell integration', 'The terminal asks the shell to change directory without typing anything at the prompt. Paths are bytes and are percent-encoded on the wire.', [
    codeBlock(r'''
use fuzzball::shell::{Advertise, ChangeDir, ShellReply, CAP_CWD};

// Terminal → shell: silent cd.
let req = ChangeDir::new(7, b"/home/me/src").encode();

// Shell side: read the request, answer when done.
if let Some(cd) = ChangeDir::from_message(&msg) {
    std::env::set_current_dir(std::ffi::OsStr::from_bytes(&cd.path))?;
    out.write_all(&ShellReply::ok(cd.id).encode())?;
}

// Shell → terminal at startup: what this shell can do.
let hello = Advertise::new("zsh", &[CAP_CWD]).encode();'''),
    wire(r'ESC P=Fzsc;id=7,path=/home/me/srcESC \', label: 'change directory'),
  ]),
  _ex('plugins', 'Plugins', 'A program asks to be accepted as a plugin. The terminal asks the user and answers later; read the answer with <code>granted_caps</code>.', [
    codeBlock(r'''
use fuzzball::plugins::{self, Hello, Kind};

let hello = Hello::new(1, "hello-fuzzball", Kind::ToolProvider, &["notify", "screen.read"]).encode();

// Later: which capabilities did this tab grant?
let q = plugins::query(2, "hello-fuzzball");'''),
    wire(r'ESC P=Fzph;id=1,name=hello-fuzzball,kind=tool-provider,caps=notify+screen.readESC \', label: 'hello'),
  ]),
  _ex('passthrough', 'Passthrough', 'Inside tmux (with <code>allow-passthrough on</code>) or GNU screen, wrap messages so they reach the outer terminal.', [
    codeBlock(r'''
use fuzzball::{keyboard, passthrough};

let wrapped = passthrough::tmux(keyboard::ENABLE);
// or: passthrough::screen(..), or passthrough::auto(..) to decide from TMUX / STY.'''),
    wire(r'ESC Ptmux;ESC ESC P=Fzks;on=1,v=0ESC ESC \ESC \', label: 'tmux'),
  ]),
  _ex('detection', 'Detection &amp; capabilities', 'Send a capability query followed by DA1. A Fuzzball terminal answers <code>c r</code> before the DA1 reply. Every terminal answers DA1, so if DA1 arrives without a Fuzzball reply, Fuzzball is unavailable.', [
    codeBlock(r'''
use fuzzball::common::{self, Capabilities};

out.write_all(&common::detect_query(1))?;

// When a message arrives before the DA1 reply:
if let Some(caps) = Capabilities::from_message(&msg) {
    if caps.keyboard() { /* enable keyboard mode */ }
    if caps.has_subsystem('g') { /* use graphics */ }
}'''),
    wire(r'ESC P=Fzcq;id=1ESC \ESC [c', label: 'detect_query(1)'),
  ]),
];

class Examples extends StatelessComponent {
  const Examples({super.key});

  @override
  Component build(BuildContext context) {
    return div(classes: 'examples', [
      RawText('<header class="pagehead"><h1>Examples</h1><p>Real code against the fuzzball 1.0.0 API, with the exact bytes each call produces. ESC is byte 0x1b.</p>'
          '<nav class="toc"><a href="examples#keyboard">Keyboard</a><a href="examples#graphics">Graphics</a><a href="examples#shell">Shell</a><a href="examples#plugins">Plugins</a><a href="examples#passthrough">Passthrough</a><a href="examples#detection">Detection</a></nav></header>'),
      RawText(_html.join()),
    ]);
  }
}
