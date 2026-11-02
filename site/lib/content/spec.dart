import 'dart:io';

import 'package:markdown/markdown.dart' as md;

import 'html.dart';

class SpecDoc {
  const SpecDoc(this.file, this.slug, this.title, this.blurb);
  final String file;
  final String slug;
  final String title;
  final String blurb;
  String get route => slug.isEmpty ? '/spec' : '/spec/$slug';
  String get href => slug.isEmpty ? 'spec' : 'spec/$slug';
}

const specDocs = [
  SpecDoc('README.md', '', 'Overview', 'Framing, negotiation, errors'),
  SpecDoc('keyboard.md', 'keyboard', 'Keyboard', 'Subsystem k'),
  SpecDoc('graphics.md', 'graphics', 'Graphics', 'Subsystem g'),
  SpecDoc('shell.md', 'shell', 'Shell actions', 'Subsystem s'),
  SpecDoc('plugins.md', 'plugins', 'Plugins', 'Subsystem p'),
  SpecDoc('wire-format.md', 'wire-format', 'Wire format', 'What implementations emit today'),
];

const _repo = 'https://github.com/WeirdBrains/fuzzball/blob/v1.0.0/';

/// Renders a docs/ file to HTML at pre-render time, rewriting links
/// between spec files to site routes and other repo links to GitHub.
String renderSpec(SpecDoc d) {
  final src = File('../docs/${d.file}').readAsStringSync();
  var html = md.markdownToHtml(src, extensionSet: md.ExtensionSet.gitHubWeb);
  html = html.replaceAllMapped(RegExp(r'href="([^"#:]+)(#[^"]*)?"'), (m) {
    final path = m[1]!;
    final frag = m[2] ?? '';
    final name = path.replaceFirst(RegExp(r'^\./'), '');
    for (final o in specDocs) {
      if (name == o.file) return 'href="${o.href}$frag"';
    }
    final repoPath = name.startsWith('../') ? name.substring(3) : 'docs/$name';
    return 'href="$_repo$repoPath$frag"';
  });
  return keepPre(html);
}
