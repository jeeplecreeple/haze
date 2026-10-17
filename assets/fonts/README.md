# Bundled fonts

- **Inter** and **Inter Display** by Rasmus Andersson, v4.1, from
  <https://github.com/rsms/inter/releases>. SIL Open Font License 1.1, see `Inter-OFL.txt`.
  Inter is the app font; Inter Display (ExtraBold) is the lyrics font.
- **Lucide** icons, from the `lucide-static` npm package (1.59.0). ISC licence, see `Lucide-LICENSE.txt`.
  `lucide-subset.ttf` holds only the icons listed in `src/ui/icon.rs`. To add one, look its
  codepoint up in `lucide-static/font/codepoints.json` and re-run:

  ```sh
  pyftsubset lucide.ttf --unicodes=<comma-separated U+XXXX list> --no-hinting \
    --output-file=assets/fonts/lucide-subset.ttf
  ```
