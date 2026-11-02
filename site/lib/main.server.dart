/// Server (pre-render) entrypoint for the Fuzzball site.
library;

import 'package:jaspr/dom.dart';
import 'package:jaspr/server.dart';

import 'app.dart';
import 'main.server.options.dart';

const siteUrl = 'https://weirdbrains.github.io/fuzzball/';

void main() {
  Jaspr.initializeApp(options: defaultServerOptions);
  runApp(Document(
    title: 'Fuzzball — terminal protocol crate',
    lang: 'en',
    base: '/fuzzball/',
    meta: {
      'description': 'Encoder and decoder for the Fuzzball terminal protocol: DCS Fz keyboard, graphics, shell and plugin messages. Rust, no dependencies.',
      'theme-color': '#17111c',
      'og:title': 'Fuzzball',
      'og:description': 'The Fuzzball terminal protocol, in Rust.',
      'og:image': '${siteUrl}images/og.png',
      'og:url': siteUrl,
      'og:type': 'website',
      'twitter:card': 'summary_large_image',
    },
    head: [
      link(rel: 'icon', type: 'image/svg+xml', href: 'images/logo.svg'),
      link(rel: 'icon', type: 'image/png', href: 'images/favicon-32.png'),
      link(rel: 'apple-touch-icon', href: 'images/logo-512.png'),
      link(rel: 'stylesheet', href: 'styles.css'),
    ],
    body: App(),
  ));
}
