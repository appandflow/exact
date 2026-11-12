//! The routes a touch shell's presenter places itself (`presenter/stage.rs`):
//! a sheet over its scrim, and two routes crossing in a push or a pop. The
//! presenter hands the painter a short display list each frame; the walk
//! leaves the routes it names to it and draws it after the last route of
//! the stack, so what the app wrote after the routes (a toast) stays above.
//!
//! A live route is drawn by the ordinary walk with its border box at the
//! origin of the item's transform, so its boxes are hit where they show. A
//! route the presenter keeps a drawing of is also recorded into a tape
//! (`tape.rs`) in that same space: the commit that removes it leaves the
//! tape, which the presenter slides out.
use super::tape::{record, Recorder, Tape};
use super::*;
use exact_kernel::NodeKey;

/// One step of the stage's drawing, in viewport points.
pub(crate) enum Item {
    /// Clip to a shape until [`Item::PopClip`].
    Clip(Shape, Transform),
    PopClip,
    /// Draw at an opacity until [`Item::PopOpacity`].
    Opacity(f32),
    PopOpacity,
    Fill(Shape, [u8; 4], Transform),
    /// A live route, its border box at the origin of `ts`'s space; `tape`
    /// keeps its drawing.
    Route {
        id: ViewId,
        ts: Transform,
        tape: bool,
    },
    /// A route's kept drawing, through `ts`.
    Taped(NodeKey, Transform),
    /// A picture of the presenter's own (a bar), through `ts`.
    Picture(Rc<Tape>, Transform),
    /// A box that takes the touches in a rectangle for a node: a scrim is
    /// its sheet's.
    Hit(ViewId, Rect4),
}

/// What the stage draws this frame.
#[derive(Default)]
pub(crate) struct Stage {
    /// The route after which the stage draws.
    pub(crate) after: Option<ViewId>,
    /// The live routes the walk leaves to the stage.
    pub(crate) skip: Vec<ViewId>,
    /// The routes the walk draws whose drawing is kept as well: a route
    /// Back would pop.
    pub(crate) tape: Vec<ViewId>,
    /// The routes drawn on a sheet's own ground: a background their author
    /// did not write is not painted.
    pub(crate) bare: Vec<ViewId>,
    pub(crate) items: Vec<Item>,
}

/// The drawings kept of routes, by route.
#[derive(Default)]
pub(crate) struct Kept {
    tapes: HashMap<NodeKey, Rc<Tape>>,
}

impl Painter {
    /// The stage for the next frame; drawings it neither draws nor keeps
    /// are let go.
    pub(crate) fn set_stage(&mut self, kernel: &Kernel, stage: Stage) {
        let key = |id: &ViewId| kernel.node(*id).map(|n| n.key);
        let kept: Vec<NodeKey> = (stage.items.iter())
            .filter_map(|item| match item {
                Item::Taped(key, _) => Some(*key),
                Item::Route { id, tape: true, .. } => key(id),
                _ => None,
            })
            .chain(stage.tape.iter().filter_map(key))
            .collect();
        self.kept.tapes.retain(|key, _| kept.contains(key));
        self.stage = stage;
    }

    /// Whether a drawing of `key` is kept.
    pub(crate) fn kept(&self, key: NodeKey) -> bool {
        self.kept.tapes.contains_key(&key)
    }

    /// Whether the walk leaves `id` to the stage.
    pub(super) fn staged(&self, id: ViewId) -> bool {
        self.stage.skip.contains(&id)
    }

    /// Draw the stage, after its route `after`.
    pub(super) fn paint_stage(&mut self, walk: &mut Walk<'_, '_>, after: ViewId) {
        if self.stage.after != Some(after) || self.stage.items.is_empty() {
            return;
        }
        self.row_refuse();
        self.damage.unsupported = true;
        let items = std::mem::take(&mut self.stage.items);
        for item in &items {
            match item {
                Item::Clip(shape, ts) => self.backend.push_clip(shape, *ts),
                Item::PopClip => self.backend.pop_clip(),
                Item::Opacity(a) => self.backend.push_opacity(a.clamp(0.0, 1.0)),
                Item::PopOpacity => self.backend.pop_opacity(),
                Item::Fill(shape, color, ts) => self.backend.fill(shape, *color, *ts),
                Item::Route { id, ts, tape } => self.stage_route(walk, *id, *ts, *tape),
                Item::Taped(key, ts) => {
                    if let Some(tape) = self.kept.tapes.get(key).cloned() {
                        tape.replay(self.backend.as_mut(), &mut self.text.borrow_mut(), *ts);
                    }
                }
                Item::Picture(tape, ts) => {
                    tape.replay(self.backend.as_mut(), &mut self.text.borrow_mut(), *ts)
                }
                Item::Hit(id, rect) => walk.boxes.push(PaintedBox {
                    id: *id,
                    pointer_hit: true,
                    projective: None,
                    affine: Some((Transform::identity(), *rect)),
                    press: 1.,
                    rect: *rect,
                    clip: None,
                    scroll: None,
                }),
            }
        }
        self.stage.items = items;
    }

    /// A live route at the origin of `ts`, and its drawing kept.
    fn stage_route(&mut self, walk: &mut Walk<'_, '_>, id: ViewId, ts: Transform, tape: bool) {
        let Some(node) = walk.scene.kernel.node(id) else {
            return;
        };
        let at = (node.frame.x, node.frame.y);
        self.node_at(walk, id, ts, at, None);
        if tape {
            self.keep_route(walk, id);
        }
    }

    /// Whether `id` stands on a sheet's ground, with no background unless
    /// its author wrote one.
    pub(super) fn bare(&self, node: &NodeRef<'_>) -> bool {
        self.stage.bare.contains(&node.id) && !node.style.written.has(StyleId::BackgroundColor)
    }

    /// Whether the walk keeps `id`'s drawing as it draws it.
    pub(super) fn keeps(&self, id: ViewId) -> bool {
        self.stage.tape.contains(&id)
    }

    /// Record `id`'s drawing, its border box at the origin, and keep it.
    pub(super) fn keep_route(&mut self, walk: &mut Walk<'_, '_>, id: ViewId) {
        let Some(node) = walk.scene.kernel.node(id) else {
            return;
        };
        let (key, at) = (node.key, (node.frame.x, node.frame.y));
        let recorder = Recorder::default();
        let real = std::mem::replace(&mut self.backend, Box::new(recorder.clone()));
        let rows = self.rows_pause();
        let boxes = walk.boxes.len();
        self.node_at(walk, id, Transform::identity(), at, None);
        walk.boxes.truncate(boxes);
        self.rows_resume(rows);
        self.backend = real;
        self.kept.tapes.insert(key, Rc::new(recorder.tape()));
    }
}

/// A picture of the presenter's own, recorded once and drawn by the stage.
pub(crate) fn picture(paint: impl FnOnce(&mut dyn Backend)) -> Rc<Tape> {
    Rc::new(record(paint))
}
