//! CSS `background-blend-mode` (CSS Compositing 1 §4). The background colour
//! and gradient layers are drawn into a CPU island, each layer blended with
//! what lies beneath it there. The island is then placed over the box
//! unblended, so no layer reads what is behind the box. Every backend places
//! an island, Android's reader included.

use super::{gradient, Backend, BoxGeometry, BoxPaint, Shape};
use crate::raster::Raster;
use std::sync::Arc;
use tiny_skia::Transform;

impl BoxPaint {
    /// Whether a gradient layer blends.
    pub(super) fn blends(&self) -> bool {
        self.blend != exact_kernel::MixBlendMode::Normal && !self.gradients.is_empty()
    }

    /// The background colour and gradients within `clip`, as one group;
    /// false, having drawn nothing, for a box past the largest island. Such a
    /// box (a scrolled column over 16,384 device pixels) draws unblended
    /// rather than an island cut to what shows, which would move its layers.
    pub(super) fn blended(
        &self,
        backend: &mut dyn Backend,
        geometry: &BoxGeometry,
        clip: &Shape,
        ts: Transform,
        scale: f32,
    ) -> bool {
        let rect = geometry.outer.rect;
        let side = (rect.2.max(rect.3) * scale).ceil();
        if !(side <= crate::raster::MAX_FRAME_SIDE as f32) {
            return false;
        }
        let shift = Transform::from_translate(-rect.0, -rect.1);
        if let Some(group) = island(rect, scale, |group| {
            self.layers(group, &geometry.outer, clip, shift, scale);
        }) {
            backend.island_image(Arc::new(group), rect, ts, 0);
        }
        true
    }

    /// The colour within `clip`, then each gradient blended over what is
    /// beneath it in `backend`, the last layer first. `outer` is the border
    /// box the gradients are placed in.
    pub(super) fn layers(
        &self,
        backend: &mut dyn Backend,
        outer: &Shape,
        clip: &Shape,
        ts: Transform,
        scale: f32,
    ) {
        if self.background[3] > 0 {
            backend.fill(clip, self.background, ts);
        }
        let rect = outer.rect;
        let shift = Transform::from_translate(-rect.0, -rect.1);
        for g in self.gradients.iter().rev() {
            if !self.blends() {
                gradient::paint(g, outer, clip, self.widths, backend, ts);
                continue;
            }
            let Some(layer) = island(rect, scale, |layer| {
                gradient::paint(g, outer, clip, self.widths, layer, shift);
            }) else {
                continue;
            };
            backend.island_image(Arc::new(layer), rect, ts, self.blend as u8);
        }
    }
}

/// Transparent pixels over `rect` at `scale`, painted by `paint` in the
/// rectangle's own space, its corner at 0.
fn island(
    rect: super::Rect4,
    scale: f32,
    paint: impl FnOnce(&mut Raster),
) -> Option<tiny_skia::Pixmap> {
    if !(rect.2 > 0.0 && rect.3 > 0.0) {
        return None;
    }
    let mut raster = Raster::transparent();
    raster.begin(rect.2, rect.3, scale);
    paint(&mut raster);
    raster.finish().ok()
}
