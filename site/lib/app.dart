import 'package:jaspr/dom.dart';
import 'package:jaspr/jaspr.dart';
import 'package:jaspr_router/jaspr_router.dart';

import 'content/spec.dart';
import 'pages/examples.dart';
import 'pages/home.dart';
import 'pages/spec_page.dart';

class App extends StatelessComponent {
  const App({super.key});

  @override
  Component build(BuildContext context) {
    return Router(routes: [
      Route(path: '/', title: 'Fuzzball — terminal protocol crate', builder: (c, s) => const Layout(active: 'features', child: Home())),
      Route(path: '/examples', title: 'Examples — Fuzzball', builder: (c, s) => const Layout(active: 'examples', child: Examples())),
      for (final d in specDocs)
        Route(
          path: d.route,
          title: '${d.title} — Fuzzball spec',
          builder: (c, s) => Layout(active: 'spec', child: SpecPage(doc: d)),
        ),
    ]);
  }
}

class Layout extends StatelessComponent {
  const Layout({required this.active, required this.child, super.key});
  final String active;
  final Component child;

  Component _nav(String id, String href, String label) =>
      a(href: href, classes: active == id ? 'active' : null, [.text(label)]);

  @override
  Component build(BuildContext context) {
    return div(classes: 'page', [
      header(classes: 'top', [
        a(href: './', classes: 'brand', [
          img(src: 'images/logo.svg', alt: '', width: 34, height: 34),
          span([.text('fuzzball')]),
        ]),
        nav([
          _nav('features', './#features', 'Features'),
          _nav('examples', 'examples', 'Examples'),
          _nav('spec', 'spec', 'Spec'),
          a(href: 'https://docs.rs/fuzzball', classes: 'ext', [.text('docs.rs')]),
          a(href: 'https://github.com/WeirdBrains/fuzzball', classes: 'ext', [.text('GitHub')]),
        ]),
      ]),
      main_([child]),
      footer(classes: 'foot', [
        RawText('<p><a href="https://crates.io/crates/fuzzball">crates.io</a> · '
            '<a href="https://docs.rs/fuzzball">docs.rs</a> · '
            '<a href="https://github.com/WeirdBrains/fuzzball">GitHub</a> · '
            'Reference terminal: <a href="https://mouser.weirdbrains.com">Mouser</a></p>'
            '<p>BSD-3-Clause · Copyright © 2026 Weird Brains</p>'),
      ]),
    ]);
  }
}
