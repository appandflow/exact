//! A modal route with detents shown as a bottom sheet (`presenter/sheet.rs`):
//! the scrim over the page, then the route, laid out in its block, placed
//! with its top edge at the sheet's.
use super::*;

/// The sheet the presenter shows this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SheetPaint {
    /// The route; `None` while its exit plays (`exit.rs` draws it).
    pub(crate) route: Option<ViewId>,
    /// The route's parent, under whose children the scrim of an exit goes.
    pub(crate) parent: exact_kernel::NodeKey,
    /// The sheet's top edge, viewport points.
    pub(crate) top: f32,
    /// The scrim's opacity, 0 to 1 of Material's 32 % black.
    pub(crate) scrim: f32,
}

/// Material 3's scrim: black at 32 %.
const SCRIM: f32 = 0.32;

impl Painter {
    fn scrim(&mut self, s: SheetPaint) {
        let alpha = (s.scrim.clamp(0.0, 1.0) * SCRIM * 255.0).round() as u8;
        if alpha > 0 {
            let (w, h) = self.viewport;
            self.backend.fill(
                &Shape::rect((0., 0., w, h)),
                [0, 0, 0, alpha],
                Transform::identity(),
            );
        }
    }

    /// Paint `id` when it is the sheet's route; whether it was.
    pub(super) fn sheet_node(
        &mut self,
        walk: &mut Walk<'_, '_>,
        id: ViewId,
        ts: Transform,
        offset: (f32, f32),
        clip: Option<Rect4>,
    ) -> bool {
        let Some(s) = self.sheet.filter(|s| s.route == Some(id)) else {
            return false;
        };
        if (walk.scene.hidden)(id) {
            return true;
        }
        self.row_refuse();
        self.damage.unsupported = true;
        self.scrim(s);
        let (w, h) = self.viewport;
        // The scrim is the sheet's: a press on it reaches no page under it.
        walk.boxes.push(PaintedBox {
            id,
            pointer_hit: true,
            projective: None,
            affine: Some((Transform::identity(), (0., 0., w, h))),
            press: 1.,
            rect: (0., 0., w, h),
            clip: None,
            scroll: None,
        });
        // Rounded at its top alone, by its own `border-radius`, and its
        // content clipped to that, as Material's sheet shape clips.
        let Some(node) = walk.scene.kernel.node(id) else {
            return true;
        };
        let r = match node
            .style
            .border_radius_top_left
            .resolve(&walk.scene.kernel.env())
        {
            exact_kernel::Dimension::Points(r) => r,
            _ => 0.0,
        };
        let color = (node.style.mask.has(StyleId::BackgroundColor))
            .then_some(node.style.background_color)
            .flatten()
            .map_or([0; 4], |c| rgba(c.resolve(self.dark)));
        let rect = (
            -offset.0,
            s.top - offset.1,
            w,
            node.frame.height.max(h - s.top),
        );
        let shape = Shape::new(rect, [r, r, 0.0, 0.0]);
        self.backend.push_clip(&shape, ts);
        if color[3] > 0 {
            self.backend.fill(&shape, color, ts);
        }
        self.node_at(walk, id, ts, (offset.0, offset.1 - s.top), clip);
        // Material's drag handle: 32 by 4, 22 below the top edge.
        let handle = Shape::new((rect.0 + (w - 32.) / 2., rect.1 + 22., 32., 4.), [2.; 4]);
        self.backend.fill(&handle, [0x49, 0x45, 0x4f, 0x66], ts);
        self.backend.pop_clip();
        true
    }

    /// The scrim under a sheet whose exit plays, after `parent`'s children.
    pub(super) fn exit_scrim(&mut self, parent: exact_kernel::NodeKey) {
        if let Some(s) = self
            .sheet
            .filter(|s| s.route.is_none() && s.parent == parent)
        {
            self.damage.unsupported = true;
            self.scrim(s);
        }
    }
}
