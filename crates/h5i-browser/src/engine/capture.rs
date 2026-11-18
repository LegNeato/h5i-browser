use anyrender::{ImageRenderer, PaintScene, Scene};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_paint::paint_scene;
use h5i_error::H5iError;
use vello_cpu::kurbo::Affine;

use super::{Page, encode_png};

/// An encoded capture and its actual pixel dimensions.
pub struct PngCapture {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

struct Geometry {
    width: u32,
    height: u32,
    scale: f64,
}

impl Geometry {
    fn new(width: u32, height: u32, scale: f64, max_edge: u32) -> Result<Self, H5iError> {
        if !(1..=2048).contains(&max_edge)
            || width == 0
            || height == 0
            || !scale.is_finite()
            || scale <= 0.0
        {
            return Err(H5iError::Metadata(
                "invalid scaled capture dimensions".to_owned(),
            ));
        }
        let ratio = (f64::from(max_edge) / f64::from(width.max(height))).min(1.0);
        Ok(Self {
            width: (f64::from(width) * ratio)
                .ceil()
                .clamp(1.0, f64::from(max_edge)) as u32,
            height: (f64::from(height) * ratio)
                .ceil()
                .clamp(1.0, f64::from(max_edge)) as u32,
            scale: scale * ratio,
        })
    }
}

impl Page {
    /// Capture the viewport at a bounded resolution without changing its layout.
    ///
    /// Paint directly at the fitted resolution. No full-resolution intermediate
    /// image is allocated. `max_edge` must be between 1 and 2048 pixels.
    pub fn screenshot_png_scaled(&mut self, max_edge: u32) -> Result<PngCapture, H5iError> {
        let geometry = Geometry::new(
            self.options.width,
            self.options.height,
            f64::from(self.options.scale),
            max_edge,
        )?;
        // Parley metrics already include the document's viewport scale. Keep
        // that scale during painting, then transform every recorded command.
        let mut recorded = Scene::new();
        let mut doc = self.doc.borrow_mut();
        paint_scene(
            &mut recorded,
            &mut doc,
            f64::from(self.options.scale),
            self.options.width,
            self.options.height,
            0,
            0,
        );
        drop(doc);
        let ratio = geometry.scale / f64::from(self.options.scale);
        let mut renderer = VelloCpuImageRenderer::new(geometry.width, geometry.height);
        let mut rgba = Vec::new();
        renderer.render_to_vec(
            |scene| scene.append_scene(recorded, Affine::scale(ratio)),
            &mut rgba,
        );
        Ok(PngCapture {
            width: geometry.width,
            height: geometry.height,
            png: encode_png(&rgba, geometry.width, geometry.height)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Geometry;

    #[test]
    fn fits_portrait_landscape_and_square_without_upscaling() {
        for (width, height, expected) in [
            (400, 800, (256, 512)),
            (800, 400, (512, 256)),
            (1024, 1024, (512, 512)),
            (40, 80, (40, 80)),
            (1, u32::MAX, (1, 512)),
        ] {
            let size = Geometry::new(width, height, 1.0, 512).unwrap();
            assert_eq!((size.width, size.height), expected);
        }
        assert_eq!(Geometry::new(1024, 1024, 2.0, 512).unwrap().scale, 1.0);
    }

    #[test]
    fn rejects_invalid_extents_and_limits_before_allocating() {
        assert!(Geometry::new(0, 100, 1.0, 512).is_err());
        assert!(Geometry::new(100, 0, 1.0, 512).is_err());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(Geometry::new(100, 100, bad, 512).is_err());
        }
        for edge in [0, 2049, u32::MAX] {
            assert!(Geometry::new(100, 100, 1.0, edge).is_err());
        }
    }
}
