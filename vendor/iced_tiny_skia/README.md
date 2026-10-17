# iced_tiny_skia 0.14.1, patched for Haze

This is iced's CPU renderer ([iced](https://github.com/iced-rs/iced), MIT licence, see `LICENSE`)
from crates.io, with two changes, both marked "Haze":

1. `Renderer::draw` (`src/lib.rs`): each damaged region of the window is painted into a scratch pixmap the size of that region, then
copied into the window. Upstream paints straight into the window and limits drawing with a
window-sized mask, which tiny-skia still walks pixel by pixel, so every large quad or image (the
window background, the panels, the blurred cover) cost its full area once per damaged region.
On the Settings page that was dozens of full-window repaints per scroll step.

2. `Primitive::visible_bounds` (`src/primitive.rs`) counts a stroke's width and a pixel of
   anti-aliasing, so thick lines (the visualizer's bars and glow) don't leave stale pixels.

To update: copy the new release over this folder and re-apply both changes.
