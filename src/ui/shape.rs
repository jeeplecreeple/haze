//! The outlines used by the Now Playing page: cookie, flower, burst, hexagon and friends. The same
//! function cuts the cover art (as a transparent mask) and draws the play button and visualizer.

use iced::widget::canvas::Path;
use iced::{Point, Vector};

use crate::config::Shape;

/// A shape's wavy-edge settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outline {
    pub shape: Shape,
    /// Bumps around the edge (cookie and flower).
    pub waves: u32,
    /// How deep the bumps go, 0-1.
    pub depth: f32,
}

impl Outline {
    /// The edge's distance from the centre at `angle`, where 1 reaches the side of the square box.
    pub fn radius(&self, angle: f32) -> f32 {
        let n = self.waves.max(3) as f32;
        let (c, s) = (angle.cos().abs(), angle.sin().abs());
        let superellipse = |p: f32| 1.0 / (c.powf(p) + s.powf(p)).powf(1.0 / p);
        match self.shape {
            Shape::Circle => 1.0,
            Shape::Cookie => 1.0 - self.depth * 0.18 * (0.5 - 0.5 * (n * angle).cos()),
            Shape::Flower => 1.0 - self.depth * 0.4 * (1.0 - (n * angle * 0.5).cos().abs()).powf(1.6),
            // Straight-sided points, like a starburst sticker.
            Shape::Burst => {
                let t = (n * angle / std::f32::consts::TAU).rem_euclid(1.0);
                1.0 - self.depth * 0.38 * (2.0 * t - 1.0).abs()
            }
            // Points at the left and right, flat top and bottom.
            Shape::Hexagon => {
                let sixth = std::f32::consts::FRAC_PI_3;
                (sixth / 2.0).cos() / (angle.rem_euclid(sixth) - sixth / 2.0).cos()
            }
            Shape::Squircle => superellipse(4.0),
            Shape::Rounded => superellipse(9.0),
            Shape::Square => 1.0 / c.max(s),
        }
    }

    /// The outline as a closed path around `center`, `half` pixels to the side, turned by `turn` radians.
    pub fn path(&self, center: Point, half: f32, turn: f32) -> Path {
        let steps = 240;
        Path::new(|p| {
            for i in 0..=steps {
                let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                let r = self.radius(a) * half;
                let point = center + Vector::new((a + turn).cos() * r, (a + turn).sin() * r);
                if i == 0 { p.move_to(point) } else { p.line_to(point) }
            }
            p.close();
        })
    }

    /// The cover art cut to this outline: a square RGBA image with see-through outside, and a
    /// one-pixel soft edge so it isn't jagged.
    pub fn mask(&self, img: &::image::RgbaImage) -> ::image::RgbaImage {
        let side = img.width().min(img.height());
        let (x0, y0) = ((img.width() - side) / 2, (img.height() - side) / 2);
        let half = side as f32 / 2.0;
        let mut out = ::image::RgbaImage::new(side, side);
        for (x, y, px) in out.enumerate_pixels_mut() {
            let (dx, dy) = ((x as f32 + 0.5 - half) / half, (y as f32 + 0.5 - half) / half);
            let edge = self.radius(dy.atan2(dx));
            let alpha = ((edge - dx.hypot(dy)) * half + 0.5).clamp(0.0, 1.0);
            if alpha > 0.0 {
                let mut p = *img.get_pixel(x0 + x, y0 + y);
                p.0[3] = (p.0[3] as f32 * alpha) as u8;
                *px = p;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_stay_in_their_box() {
        for &shape in Shape::ALL {
            let o = Outline { shape, waves: 12, depth: 1.0 };
            for i in 0..720 {
                let a = i as f32 / 720.0 * std::f32::consts::TAU;
                let r = o.radius(a);
                assert!(r > 0.3, "{shape:?} too thin at {a}");
                assert!((r * a.cos()).abs() <= 1.0001 && (r * a.sin()).abs() <= 1.0001, "{shape:?} leaves its box at {a}");
            }
        }
    }

    #[test]
    fn circle_mask_clears_the_corners() {
        let img = ::image::RgbaImage::from_pixel(64, 64, ::image::Rgba([200, 10, 10, 255]));
        let m = Outline { shape: Shape::Circle, waves: 12, depth: 0.3 }.mask(&img);
        assert_eq!(m.get_pixel(0, 0).0[3], 0);
        assert_eq!(m.get_pixel(32, 32).0[3], 255);
    }
}
