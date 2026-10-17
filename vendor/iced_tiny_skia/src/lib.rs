#![allow(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub mod window;

mod engine;
mod layer;
mod primitive;
mod settings;
mod text;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "svg")]
mod vector;

#[cfg(feature = "geometry")]
pub mod geometry;

use iced_debug as debug;
pub use iced_graphics as graphics;
pub use iced_graphics::core;

pub use layer::Layer;
pub use primitive::Primitive;
pub use settings::Settings;

#[cfg(feature = "geometry")]
pub use geometry::Geometry;

use crate::core::renderer;
use crate::core::{
    Background, Color, Font, Pixels, Point, Rectangle, Size, Transformation,
    Vector,
};
use crate::engine::Engine;
use crate::graphics::Viewport;
use crate::graphics::compositor;
use crate::graphics::text::{Editor, Paragraph};

/// A [`tiny-skia`] graphics renderer for [`iced`].
///
/// [`tiny-skia`]: https://github.com/RazrFalcon/tiny-skia
/// [`iced`]: https://github.com/iced-rs/iced
#[derive(Debug)]
pub struct Renderer {
    default_font: Font,
    default_text_size: Pixels,
    layers: layer::Stack,
    engine: Engine, // TODO: Shared engine
    /// Pixels of the region being painted (see `draw`).
    scratch: Vec<u8>,
}

impl Renderer {
    pub fn new(default_font: Font, default_text_size: Pixels) -> Self {
        Self {
            default_font,
            default_text_size,
            layers: layer::Stack::new(),
            engine: Engine::new(),
            scratch: Vec::new(),
        }
    }

    pub fn layers(&mut self) -> &[Layer] {
        self.layers.flush();
        self.layers.as_slice()
    }

    pub fn draw(
        &mut self,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        _clip_mask: &mut tiny_skia::Mask,
        viewport: &Viewport,
        damage: &[Rectangle],
        background_color: Color,
    ) {
        let scale_factor = viewport.scale_factor();

        self.layers.flush();

        // Haze: each damaged region is painted into a scratch pixmap of its own size and then
        // copied in. tiny-skia has no clip rectangle, only a full-size mask, so painting straight
        // into the window made every big quad and image (the window background, the panels, the
        // blurred cover) cost its whole area once per damaged region, which made scrolling crawl.
        let (full_w, full_h) = (pixels.width(), pixels.height());

        for &damage_bounds in damage {
            let damage_bounds = damage_bounds * scale_factor;

            // Whole pixels, so text and images land on the same pixels as before.
            let x0 = damage_bounds.x.floor().max(0.0) as u32;
            let y0 = damage_bounds.y.floor().max(0.0) as u32;
            let x1 = ((damage_bounds.x + damage_bounds.width).ceil() as u32).min(full_w);
            let y1 = ((damage_bounds.y + damage_bounds.height).ceil() as u32).min(full_h);

            if x1 <= x0 || y1 <= y0 {
                continue;
            }

            let (width, height) = (x1 - x0, y1 - y0);
            let damage_bounds = Rectangle {
                x: x0 as f32,
                y: y0 as f32,
                width: width as f32,
                height: height as f32,
            };

            self.scratch.clear();
            self.scratch.resize(width as usize * height as usize * 4, 0);

            let Some(mut local) = tiny_skia::PixmapMut::from_bytes(
                &mut self.scratch,
                width,
                height,
            ) else {
                continue;
            };

            let Some(mut clip_mask) = tiny_skia::Mask::new(width, height)
            else {
                continue;
            };

            local.fill(engine::into_color(background_color));

            let offset = Vector::new(-(x0 as f32), -(y0 as f32));
            let to_local = Transformation::translate(offset.x, offset.y)
                * Transformation::scale(scale_factor);

            for layer in self.layers.iter() {
                let Some(layer_bounds) =
                    damage_bounds.intersection(&(layer.bounds * scale_factor))
                else {
                    continue;
                };

                let layer_bounds = layer_bounds + offset;

                engine::adjust_clip_mask(&mut clip_mask, layer_bounds);

                if !layer.quads.is_empty() {
                    let render_span = debug::render(debug::Primitive::Quad);
                    for (quad, background) in &layer.quads {
                        self.engine.draw_quad(
                            quad,
                            background,
                            to_local,
                            &mut local,
                            &mut clip_mask,
                            layer_bounds,
                        );
                    }
                    render_span.finish();
                }

                if !layer.primitives.is_empty() {
                    let render_span = debug::render(debug::Primitive::Triangle);

                    for group in &layer.primitives {
                        let Some(group_bounds) = (group.clip_bounds()
                            * scale_factor
                            + offset)
                            .intersection(&layer_bounds)
                        else {
                            continue;
                        };

                        engine::adjust_clip_mask(&mut clip_mask, group_bounds);

                        for primitive in group.as_slice() {
                            self.engine.draw_primitive(
                                primitive,
                                to_local * group.transformation(),
                                &mut local,
                                &mut clip_mask,
                                group_bounds,
                            );
                        }

                        engine::adjust_clip_mask(&mut clip_mask, layer_bounds);
                    }

                    render_span.finish();
                }

                if !layer.images.is_empty() {
                    let render_span = debug::render(debug::Primitive::Image);

                    for image in &layer.images {
                        self.engine.draw_image(
                            image,
                            to_local,
                            &mut local,
                            &mut clip_mask,
                            layer_bounds,
                        );
                    }

                    render_span.finish();
                }

                if !layer.text.is_empty() {
                    let render_span = debug::render(debug::Primitive::Image);

                    for group in &layer.text {
                        for text in group.as_slice() {
                            self.engine.draw_text(
                                text,
                                to_local * group.transformation(),
                                &mut local,
                                &mut clip_mask,
                                layer_bounds,
                            );
                        }
                    }

                    render_span.finish();
                }
            }

            pixels.draw_pixmap(
                x0 as i32,
                y0 as i32,
                local.as_ref(),
                &tiny_skia::PixmapPaint {
                    blend_mode: tiny_skia::BlendMode::Source,
                    quality: tiny_skia::FilterQuality::Nearest,
                    ..Default::default()
                },
                tiny_skia::Transform::identity(),
                None,
            );
        }

        self.engine.trim();
    }
}

impl core::Renderer for Renderer {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.layers.push_clip(bounds);
    }

    fn end_layer(&mut self) {
        self.layers.pop_clip();
    }

    fn start_transformation(&mut self, transformation: Transformation) {
        self.layers.push_transformation(transformation);
    }

    fn end_transformation(&mut self) {
        self.layers.pop_transformation();
    }

    fn fill_quad(
        &mut self,
        quad: renderer::Quad,
        background: impl Into<Background>,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into(), transformation);
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        self.layers.reset(new_bounds);
    }

    fn allocate_image(
        &mut self,
        _handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>)
        + Send
        + 'static,
    ) {
        #[cfg(feature = "image")]
        #[allow(unsafe_code)]
        // TODO: Concurrency
        callback(self.engine.raster_pipeline.load(_handle));

        #[cfg(not(feature = "image"))]
        callback(Err(core::image::Error::Unsupported));
    }
}

impl core::text::Renderer for Renderer {
    type Font = Font;
    type Paragraph = Paragraph;
    type Editor = Editor;

    const ICON_FONT: Font = Font::with_name("Iced-Icons");
    const CHECKMARK_ICON: char = '\u{f00c}';
    const ARROW_DOWN_ICON: char = '\u{e800}';
    const ICED_LOGO: char = '\u{e801}';
    const SCROLL_UP_ICON: char = '\u{e802}';
    const SCROLL_DOWN_ICON: char = '\u{e803}';
    const SCROLL_LEFT_ICON: char = '\u{e804}';
    const SCROLL_RIGHT_ICON: char = '\u{e805}';

    fn default_font(&self) -> Self::Font {
        self.default_font
    }

    fn default_size(&self) -> Pixels {
        self.default_text_size
    }

    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();

        layer.draw_paragraph(
            text,
            position,
            color,
            clip_bounds,
            transformation,
        );
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_editor(editor, position, color, clip_bounds, transformation);
    }

    fn fill_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text(text, position, color, clip_bounds, transformation);
    }
}

impl graphics::text::Renderer for Renderer {
    fn fill_raw(&mut self, raw: graphics::text::Raw) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text_raw(raw, transformation);
    }
}

#[cfg(feature = "geometry")]
impl graphics::geometry::Renderer for Renderer {
    type Geometry = Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        let (layer, transformation) = self.layers.current_mut();

        match geometry {
            Geometry::Live {
                primitives,
                images,
                text,
                clip_bounds,
            } => {
                layer.draw_primitive_group(
                    primitives,
                    clip_bounds,
                    transformation,
                );

                for image in images {
                    layer.draw_image(image, transformation);
                }

                layer.draw_text_group(text, clip_bounds, transformation);
            }
            Geometry::Cache(cache) => {
                layer.draw_primitive_cache(
                    cache.primitives,
                    cache.clip_bounds,
                    transformation,
                );

                for image in cache.images.iter() {
                    layer.draw_image(image.clone(), transformation);
                }

                layer.draw_text_cache(
                    cache.text,
                    cache.clip_bounds,
                    transformation,
                );
            }
        }
    }
}

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, _mesh: graphics::Mesh) {
        log::warn!("iced_tiny_skia does not support drawing meshes");
    }

    fn draw_mesh_cache(&mut self, _cache: iced_graphics::mesh::Cache) {
        log::warn!("iced_tiny_skia does not support drawing meshes");
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        self.engine.raster_pipeline.load(handle)
    }

    fn measure_image(
        &self,
        handle: &Self::Handle,
    ) -> Option<crate::core::Size<u32>> {
        self.engine.raster_pipeline.dimensions(handle)
    }

    fn draw_image(
        &mut self,
        image: core::Image,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(
        &self,
        handle: &core::svg::Handle,
    ) -> crate::core::Size<u32> {
        self.engine.vector_pipeline.viewport_dimensions(handle)
    }

    fn draw_svg(
        &mut self,
        svg: core::Svg,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

impl compositor::Default for Renderer {
    type Compositor = window::Compositor;
}

impl renderer::Headless for Renderer {
    async fn new(
        default_font: Font,
        default_text_size: Pixels,
        backend: Option<&str>,
    ) -> Option<Self> {
        if backend.is_some_and(|backend| {
            !["tiny-skia", "tiny_skia"].contains(&backend)
        }) {
            return None;
        }

        Some(Self::new(default_font, default_text_size))
    }

    fn name(&self) -> String {
        "tiny-skia".to_owned()
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        let viewport = Viewport::with_physical_size(size, scale_factor);

        window::compositor::screenshot(self, &viewport, background_color)
    }
}
