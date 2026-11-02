import 'html.dart';

// Real screenshots of Mouser 1.1.1 (the reference terminal) on Linux/Hyprland,
// captured with grim from a test window. Test media made locally (ImageMagick, ffmpeg).

final _graphics = shot(
  'mouser-graphics.png',
  'Mouser terminal window after running "mouser show --width 30 logo-512.png test.png": the pink Fuzzball logo and a pink-to-yellow test image appear inline, followed by the shell prompt.',
  'Fuzzball graphics in Mouser: <code>mouser show</code> uploads each image (<code>g u</code>) and places it at the cursor cell (<code>g p</code>).',
);

final _video = shot(
  'mouser-video-frame.png',
  'Mouser terminal window showing a still frame of an ffmpeg testsrc2 clip (colour bars with a timestamp) drawn inline below the three commands that made and showed it.',
  'A frame from a test video, extracted with ffmpeg and shown inline. <code>mouser show</code> displays images (PNG, JPEG, GIF first frame, WebP, BMP); it does not play video.',
);

final List<String> homeShots = [_graphics, _video];

String shotFor(String key) => switch (key) {
      'graphics' => _graphics + _video,
      _ => '',
    };
