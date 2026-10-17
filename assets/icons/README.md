# Built-in app icons

The alternative icons you can pick under Settings → Appearance → App icon. Each one is a rounded
square with a gradient and a glyph from [Lucide](https://lucide.dev) (ISC licence, see
`../fonts/Lucide-LICENSE.txt`), drawn at 1024 px with Pillow and scaled to 256 px. `../icon.png`
is the default icon.

| File | Colours | Glyph |
| --- | --- | --- |
| sunset.png | #ffb03a → #ff2e6e | headphones |
| ocean.png | #35d8ff → #2747ff | audio-waveform |
| mint.png | #55f0b0 → #0e8f86 | music-2 |
| midnight.png | #283152 → #07080f, pink glyph | disc-3 |
| mono.png | #ffffff → #e4e4ea, black glyph | music |
| aurora.png | #8b3dff → #19d3e6 | sparkles |
| ember.png | #3a0d0d → #ff5a1f | radio |

To add one, also list it in `BUILTIN_ICONS` in `src/ui/custom.rs`.
