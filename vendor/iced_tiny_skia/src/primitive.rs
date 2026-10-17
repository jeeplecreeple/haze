use crate::core::Rectangle;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    /// A path filled with some paint.
    Fill {
        /// The path to fill.
        path: tiny_skia::Path,
        /// The paint to use.
        paint: tiny_skia::Paint<'static>,
        /// The fill rule to follow.
        rule: tiny_skia::FillRule,
    },
    /// A path stroked with some paint.
    Stroke {
        /// The path to stroke.
        path: tiny_skia::Path,
        /// The paint to use.
        paint: tiny_skia::Paint<'static>,
        /// The stroke settings.
        stroke: tiny_skia::Stroke,
    },
}

impl Primitive {
    /// Returns the visible bounds of the [`Primitive`].
    pub fn visible_bounds(&self) -> Rectangle {
        // Haze: a stroke reaches half its width past the path, and anti-aliasing a pixel past
        // that. Upstream used the bare path bounds, so thick strokes left stale pixels behind.
        let (bounds, reach) = match self {
            Primitive::Fill { path, .. } => (path.bounds(), 1.0),
            Primitive::Stroke { path, stroke, .. } => {
                (path.bounds(), stroke.width / 2.0 + 1.0)
            }
        };

        Rectangle {
            x: bounds.x() - reach,
            y: bounds.y() - reach,
            width: bounds.width() + reach * 2.0,
            height: bounds.height() + reach * 2.0,
        }
    }
}
