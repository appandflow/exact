//! A `Composite` canvas's children (LLP 1014 D2, D3): laid out by the kernel
//! in the canvas's box, painted apart at the painter's scale into one
//! picture, which the host uploads for the surface to sample
//! (`gpu_texture`, `Surface::children`), and not painted over the canvas.
//! They keep their hit boxes, at their layout frames.
use super::*;
use std::collections::BTreeSet;

/// The canvases whose surfaces composite their children, and the pictures
/// of their children this paint made.
#[derive(Default)]
pub(crate) struct Composites {
    pub(crate) canvases: BTreeSet<ViewId>,
    pub(crate) pictures: Vec<(ViewId, Arc<Pixmap>)>,
}

impl Painter {
    /// Paint `canvas`'s children apart when its surface composites them;
    /// whether it does. `offset` and `clip` are what its children would
    /// otherwise be painted with.
    pub(super) fn composite(
        &mut self,
        walk: &mut Walk<'_, '_>,
        canvas: &NodeRef<'_>,
        ts: Transform,
        offset: (f32, f32),
        clip: Option<Rect4>,
    ) -> bool {
        use crate::placement;
        if !self.composites.canvases.contains(&canvas.id) {
            return false;
        }
        self.row_refuse();
        let f = canvas.frame;
        if f.width <= 0. || f.height <= 0. {
            return true;
        }
        let mut painter = Painter::new(
            self.text.clone(),
            self.scale,
            Box::new(crate::raster::Raster::transparent()),
        );
        painter.dark = self.dark;
        painter.symbols = self.symbols;
        painter.placements = self.placements.clone();
        painter.viewport = (f.width, f.height);
        painter.backend.begin(f.width, f.height, self.scale);
        let mut children = Walk {
            scene: walk.scene,
            boxes: Vec::new(),
            text: BTreeMap::new(),
            fields: HashMap::new(),
            inline: HashMap::new(),
            skip: None,
            replay: None,
            ranks: Rc::clone(&walk.ranks),
            reveal: walk.reveal,
            dialog: walk.dialog,
        };
        // Kernel frames are the page's: the canvas's own origin is the picture's.
        for child in canvas.children() {
            painter.node(
                &mut children,
                child,
                Transform::identity(),
                (f.x, f.y),
                None,
            );
        }
        walk.text.extend(children.text);
        walk.fields.extend(children.fields);
        walk.inline.extend(children.inline);
        if let Ok(pixels) = painter.backend.finish() {
            self.composites.pictures.push((canvas.id, Arc::new(pixels)));
        }
        // Hit boxes map from the picture to the page as the canvas does.
        let identity = [1., 0., 0., 0., 1., 0., 0., 0., 1.];
        let h = placement::compose(identity, ts, f.x - offset.0, f.y - offset.1);
        let Some(inverse) = placement::inverse(h) else {
            return true;
        };
        let planes = [[0., 0., 1.]; 2];
        for mut b in children.boxes {
            b.projective = Some((inverse, b.rect, b.clip, planes));
            b.rect = placement::clipped_bounds(&h, planes, b.rect);
            b.clip = clip;
            walk.boxes.push(b);
        }
        true
    }
}
