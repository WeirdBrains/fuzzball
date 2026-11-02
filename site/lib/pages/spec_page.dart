import 'package:jaspr/dom.dart';
import 'package:jaspr/jaspr.dart';

import '../content/spec.dart';

class SpecPage extends StatelessComponent {
  const SpecPage({required this.doc, super.key});
  final SpecDoc doc;

  @override
  Component build(BuildContext context) {
    return div(classes: 'spec', [
      aside(classes: 'specnav', [
        p(classes: 'label', [.text('Protocol spec')]),
        ul([
          for (final d in specDocs)
            li([
              a(href: d.href, classes: d == doc ? 'active' : null, [
                span([.text(d.title)]),
                small([.text(d.blurb)]),
              ]),
            ]),
        ]),
        p(classes: 'src', [
          a(href: 'https://github.com/WeirdBrains/fuzzball/blob/v1.0.0/docs/${doc.file}', [.text('View docs/${doc.file} on GitHub')]),
        ]),
      ]),
      article(classes: 'prose', [RawText(renderSpec(doc))]),
    ]);
  }
}
