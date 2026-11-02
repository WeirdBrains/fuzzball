String esc(String s) => s.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');

/// A code block. [lang] is shown as a small label.
String codeBlock(String code, {String lang = 'rust'}) =>
    keepPre('<div class="code"><span class="lang">${esc(lang)}</span><pre><code>${esc(code.trim())}</code></pre></div>');

/// Bytes on the wire, with ESC shown as a dimmed token.
String wire(String bytes, {String label = 'on the wire'}) {
  final h = esc(bytes).replaceAll('ESC', '<b class="esc">ESC</b>');
  return keepPre('<div class="wire"><span class="lang">${esc(label)}</span><pre><code>$h</code></pre></div>');
}

/// A real screenshot, if present, with caption.
String shot(String file, String alt, String caption) =>
    '<figure class="shot"><img src="images/shots/$file" alt="${esc(alt)}" loading="lazy"><figcaption>$caption</figcaption></figure>';

/// Jaspr indents every line of raw HTML when it pretty-prints the page, which
/// would shift lines inside <pre>. Encode those newlines as character references.
String keepPre(String html) => html.replaceAllMapped(
    RegExp(r'<pre[\s\S]*?</pre>'), (m) => m[0]!.replaceAll('\r\n', '\n').replaceAll('\n', '&#10;'));
