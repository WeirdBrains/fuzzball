import 'package:jaspr/dom.dart';
import 'package:jaspr/jaspr.dart';

import '../content/html.dart';
import '../content/shots.dart';

const _features = [
  ('k', 'Keyboard', 'One opt-in mode of structured key events: physical key, logical key, modifiers, locks, text and IME phases. Press, repeat and release are all reported, so applications see exactly what was typed.', 'examples#keyboard', 'spec/keyboard'),
  ('g', 'Graphics', 'Image upload as chunked base64 (RGBA, RGB or PNG), cell-anchored placements with size, source rectangle and stacking layer, delete, and a memory/cell-size query.', 'examples#graphics', 'spec/graphics'),
  ('s', 'Shell integration', 'Shells advertise what they can do, and the terminal can ask for a silent <code>cd</code>. Paths are percent-encoded bytes, so they need not be UTF-8.', 'examples#shell', 'spec/shell'),
  ('p', 'Plugins', 'A program running in the terminal (locally, over ssh, inside tmux) asks to be accepted as a plugin with a name, kind and capabilities. The terminal asks the user and answers later; grants are per tab and end with the tab.', 'examples#plugins', 'spec/plugins'),
  ('⇄', 'Passthrough', 'Wrap any message for tmux or GNU screen, or let <code>passthrough::auto</code> pick: tmux when <code>TMUX</code> is set, screen when <code>STY</code> is set.', 'examples#passthrough', 'spec'),
  ('c', 'Detection &amp; capabilities', 'A capability query followed by DA1. A Fuzzball terminal answers <code>c r</code> before the DA1 reply, listing its subsystems, version and chunk limit.', 'examples#detection', 'spec'),
];

class Home extends StatelessComponent {
  const Home({super.key});

  @override
  Component build(BuildContext context) {
    final feats = StringBuffer();
    for (final (glyph, name, body, ex, spec) in _features) {
      feats.write('<article class="feat"><div class="glyph">$glyph</div><h3>$name</h3><p>$body</p>'
          '<p class="more"><a href="$ex">Example →</a> <a href="$spec">Spec →</a></p></article>');
    }
    return div([
      section(classes: 'hero', [
        RawText('''
<div class="hero-text">
  <p class="kicker">Rust crate · v1.0.0 · no dependencies</p>
  <h1>fuzzball</h1>
  <p class="lede">Encoder and decoder for the <strong>Fuzzball</strong> terminal protocol, the DCS <code>Fz</code> extension family first implemented in the <a href="https://mouser.weirdbrains.com">Mouser</a> terminal. No I/O, no terminal library: bring your own reader and writer.</p>
  ${wire('ESC P = F z <subsystem> <verb> ; <params> [: <payload>] ESC \\', label: 'every message')}
  <div class="cta">
    <a class="btn primary" href="examples">See examples</a>
    <a class="btn" href="spec">Read the spec</a>
    <a class="btn" href="https://crates.io/crates/fuzzball">crates.io</a>
  </div>
</div>
<div class="hero-art"><img src="images/logo.svg" alt="Fuzzball logo: a matte, fuzzy pink pom-pom cat-toy ball" width="300" height="300"></div>
'''),
      ]),
      section(classes: 'install', [
        RawText('<h2>Install</h2><div class="two">'
            '${codeBlock('cargo add fuzzball', lang: 'shell')}'
            '${codeBlock('[dependencies]\nfuzzball = "1"', lang: 'Cargo.toml')}'
            '</div><p class="note">1.x keeps the wire format and API compatible. Golden-byte tests lock every message the crate writes. MSRV: Rust 1.85.</p>'),
      ]),
      section(id: 'features', classes: 'features', [
        RawText('<h2>Features</h2><p class="sub">Five subsystems behind one prefix, plus helpers for multiplexers.</p>'
            '<div class="grid">$feats</div>'),
      ]),
      if (homeShots.isNotEmpty)
        section(classes: 'gallery', [
          RawText('<h2>In action in Mouser</h2><p class="sub">Real screenshots of Mouser, the reference terminal.</p>'
              '<div class="shots">${homeShots.join()}</div>'),
        ]),
    ]);
  }
}
