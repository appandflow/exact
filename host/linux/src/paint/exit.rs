//! A leaving node's drawing (`-exact-exit-animation`).
//!
//! The kernel destroys a node before the host hears that it leaves, and this
//! painter walks the live tree, so each node that declares an exit row is
//! recorded as the walk paints it: its surface and subtree drawn once more,
//! in its parent's space and without its own transform and opacity, into a
//! [`Tape`] of owned draw calls (paragraphs, pictures and paths kept alive).
//! The recording walks no kept rows and opens no reader layers, so it needs
//! nothing a backend frees when the subtree goes.
//!
//! While the host plays the exit ([`Leaving`]), the walk replays the tape
//! after its surviving parent's children: under the parent's clips and
//! scroll, moved by however far the parent moved, through the exit's
//! sampled transform about the node's `transform-origin` and at its
//! opacity. It pushes no box, so input passes through it. The node's own
//! `mask-image` and `clip-path` are not replayed.
use super::svg::Ink;
use super::*;
use exact_kernel::NodeKey;
use std::cell::RefCell;

/// A leaving node the host plays an exit for, as sampled now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Leaving {
    pub(crate) key: NodeKey,
    /// The surviving parent it leaves from.
    pub(crate) parent: NodeKey,
    /// The exit's sampled values; `None` keeps what the node showed.
    pub(crate) opacity: Option<f32>,
    pub(crate) translate: Option<((f32, f32), (f32, f32))>,
    pub(crate) scale: Option<f32>,
    pub(crate) rotate: Option<f32>,
}

/// One node's drawing as last walked.
pub(super) struct Tape {
    ops: Vec<Op>,
    /// The parent's space and the children's offset it was drawn in.
    ts: Transform,
    offset: (f32, f32),
    /// Its border box in that space, its `transform-origin` in the box, and
    /// what it showed.
    rect: Rect4,
    origin: (f32, f32),
    shown: Presented,
}

/// The tapes kept: each exit-declaring node's last drawing, and the leaving
/// nodes the host plays.
#[derive(Default)]
pub(crate) struct Exits {
    tapes: HashMap<NodeKey, Tape>,
    pub(crate) leaving: Vec<Leaving>,
    taping: bool,
}

enum Ink2 {
    Solid([u8; 4]),
    Gradient(
        exact_kernel::svg::server::Server,
        Vec<(f32, [u8; 4])>,
        [f32; 6],
    ),
    Pattern(Arc<Pixmap>, [f32; 6], f32),
}

impl Ink2 {
    fn own(ink: &Ink<'_>) -> Self {
        match ink {
            Ink::Solid(c) => Self::Solid(*c),
            Ink::Gradient {
                server,
                stops,
                transform,
            } => Self::Gradient((*server).clone(), stops.clone(), *transform),
            Ink::Pattern {
                tile,
                transform,
                opacity,
            } => Self::Pattern(tile.clone(), *transform, *opacity),
        }
    }

    fn ink(&self) -> Ink<'_> {
        match self {
            Self::Solid(c) => Ink::Solid(*c),
            Self::Gradient(server, stops, transform) => Ink::Gradient {
                server,
                stops: stops.clone(),
                transform: *transform,
            },
            Self::Pattern(tile, transform, opacity) => Ink::Pattern {
                tile: tile.clone(),
                transform: *transform,
                opacity: *opacity,
            },
        }
    }
}

struct Svg {
    path: exact_kernel::svg::Path,
    fill: Option<Ink2>,
    even_odd: bool,
    stroke: Option<Ink2>,
    order: [u8; 3],
    width: f32,
    cap: u8,
    join: u8,
    miter: f32,
    dash: Vec<f32>,
    phase: f32,
}

enum Op {
    Fill(Shape, [u8; 4], Transform),
    Gradient(Shape, GradientPaint, Transform),
    Backdrop(Shape, BackdropFilter, Transform),
    Shadow(Shape, [u8; 4], f32, Shape, Transform),
    TextBlur(f32),
    Border(border::BorderFill, Transform),
    Image(Arc<Bitmap>, Rect4, Vec<Shape>, Transform, Option<[u8; 4]>),
    Island(Arc<Pixmap>, Rect4, Transform, u8),
    PlaneBegin([f32; 9]),
    PlaneEnd,
    Canvas(Arc<Pixmap>, Rect4, Vec<Shape>, Transform),
    Text(Rc<Paragraph>, Vec<RunPaint>, (f32, f32), Transform),
    Svg(Box<Svg>, Transform),
    SvgClip(exact_kernel::svg::scene::Clip, Transform),
    Clip(Shape, Transform),
    CssClip(exact_kernel::clip::ClipPath, Transform),
    PopClip,
    Opacity(f32),
    PopOpacity,
    Mask(Shape, Transform),
    PopMask(Shape, Result<GradientPaint, [u8; 4]>, Transform),
}

/// The backend a tape is recorded through.
struct Recorder(Rc<RefCell<Vec<Op>>>);

impl Recorder {
    fn push(&self, op: Op) {
        self.0.borrow_mut().push(op);
    }
}

impl Backend for Recorder {
    fn name(&self) -> &'static str {
        "tape"
    }
    fn begin(&mut self, _: f32, _: f32, _: f32) {}
    fn fill(&mut self, shape: &Shape, color: [u8; 4], ts: Transform) {
        self.push(Op::Fill(*shape, color, ts));
    }
    fn fill_gradient(&mut self, shape: &Shape, gradient: &GradientPaint, ts: Transform) {
        self.push(Op::Gradient(*shape, gradient.clone(), ts));
    }
    fn backdrop_filter(&mut self, shape: &Shape, filter: &BackdropFilter, ts: Transform) {
        self.push(Op::Backdrop(*shape, filter.clone(), ts));
    }
    fn blurred_shadow(
        &mut self,
        shape: &Shape,
        color: [u8; 4],
        sigma: f32,
        outer: &Shape,
        ts: Transform,
    ) -> bool {
        self.push(Op::Shadow(*shape, color, sigma, *outer, ts));
        true
    }
    fn text_blur(&mut self, sigma: f32) {
        self.push(Op::TextBlur(sigma));
    }
    fn fill_border(&mut self, part: &border::BorderFill, ts: Transform) {
        self.push(Op::Border(part.clone(), ts));
    }
    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    ) {
        self.push(Op::Image(image.clone(), dst, clips.to_vec(), ts, tint));
    }
    fn supports_island_image(&self) -> bool {
        true
    }
    fn island_image(&mut self, pixels: Arc<Pixmap>, dst: Rect4, ts: Transform, mode: u8) {
        self.push(Op::Island(pixels, dst, ts, mode));
    }
    fn surface_image(&mut self, pixels: Arc<Pixmap>, dst: Rect4) {
        self.push(Op::Canvas(pixels, dst, Vec::new(), Transform::identity()));
    }
    fn plane_begin(&mut self, h: [f32; 9]) -> bool {
        self.push(Op::PlaneBegin(h));
        true
    }
    fn plane_end(&mut self) {
        self.push(Op::PlaneEnd);
    }
    fn canvas(&mut self, pixels: &Arc<Pixmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
        self.push(Op::Canvas(pixels.clone(), dst, clips.to_vec(), ts));
    }
    fn text(
        &mut self,
        _: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        if let Some(p) = paragraph.retained() {
            self.push(Op::Text(p, palette.to_vec(), origin, ts));
        }
    }
    fn svg_path(&mut self, s: &SvgPaint<'_>, ts: Transform) {
        let svg = Svg {
            path: s.path.clone(),
            fill: s.fill.as_ref().map(Ink2::own),
            even_odd: s.even_odd,
            stroke: s.stroke.as_ref().map(Ink2::own),
            order: s.order,
            width: s.width,
            cap: s.cap,
            join: s.join,
            miter: s.miter,
            dash: s.dash.clone(),
            phase: s.phase,
        };
        self.push(Op::Svg(Box::new(svg), ts));
    }
    fn push_svg_clip(&mut self, clip: &exact_kernel::svg::scene::Clip, ts: Transform) -> usize {
        self.push(Op::SvgClip(clip.clone(), ts));
        1
    }
    fn push_clip(&mut self, shape: &Shape, ts: Transform) {
        self.push(Op::Clip(*shape, ts));
    }
    fn push_css_clip(&mut self, path: &exact_kernel::clip::ClipPath, ts: Transform) -> bool {
        self.push(Op::CssClip(path.clone(), ts));
        true
    }
    fn pop_clip(&mut self) {
        self.push(Op::PopClip);
    }
    fn push_opacity(&mut self, alpha: f32) {
        self.push(Op::Opacity(alpha));
    }
    fn pop_opacity(&mut self) {
        self.push(Op::PopOpacity);
    }
    fn push_mask(&mut self, shape: &Shape, ts: Transform) {
        self.push(Op::Mask(*shape, ts));
    }
    fn pop_mask(&mut self, shape: &Shape, mask: &Result<GradientPaint, [u8; 4]>, ts: Transform) {
        self.push(Op::PopMask(*shape, mask.clone(), ts));
    }
    fn pointer(&mut self, _: f32, _: f32) {}
    fn finish(&mut self) -> Result<Pixmap, String> {
        Err("a tape has no pixels".into())
    }
}

impl Painter {
    /// At a walk's start: the leaving nodes the host plays now; tapes of
    /// nodes that are gone and do not leave are dropped.
    pub(crate) fn set_leaving(&mut self, kernel: &Kernel, leaving: Vec<Leaving>) {
        let exits = &mut self.exits;
        exits.tapes.retain(|key, _| {
            kernel.node_by_key(*key).is_some() || leaving.iter().any(|l| l.key == *key)
        });
        exits.leaving = leaving;
    }

    /// Record `node`'s drawing if it declares an exit, as [`Painter::node_at`]
    /// walked it: `ts` its parent's space, `given` the offset its parent
    /// gave and `offset` the one it walked at (a sticky box's moves).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tape(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        (ts, given, offset): (Transform, (f32, f32), (f32, f32)),
        rect: Rect4,
        origin: (f32, f32),
        shown: Presented,
        clip_rect: Option<Rect4>,
    ) {
        let sheet = self.sheet.is_some_and(|s| s.route == Some(node.id));
        if self.exits.taping || (node.style.rare.exit_animation.0.is_empty() && !sheet) {
            return;
        }
        if node.parent.is_none() {
            return;
        }
        let ops = Rc::new(RefCell::new(Vec::new()));
        let real = std::mem::replace(&mut self.backend, Box::new(Recorder(ops.clone())));
        let recording = self.rows_pause();
        self.exits.taping = true;
        let (boxes, dark) = (walk.boxes.len(), self.dark);
        self.dark = shown.dark.unwrap_or(dark);
        self.content(walk, node, rect, ts, offset, clip_rect);
        self.dark = dark;
        walk.boxes.truncate(boxes);
        self.exits.taping = false;
        self.rows_resume(recording);
        self.backend = real;
        let ops = std::mem::take(&mut *ops.borrow_mut());
        self.exits.tapes.insert(
            node.key,
            Tape {
                ops,
                ts,
                offset: given,
                rect,
                origin,
                shown,
            },
        );
    }

    /// The leaving children of `parent`, after its own children: `ts` and
    /// `offset` are what it gives its children now.
    pub(super) fn paint_exits(&mut self, parent: NodeKey, ts: Transform, offset: (f32, f32)) {
        if self.exits.taping {
            return;
        }
        let leaving: Vec<Leaving> = (self.exits.leaving.iter())
            .filter(|l| l.parent == parent)
            .copied()
            .collect();
        for l in leaving {
            let Some(tape) = self.exits.tapes.get(&l.key) else {
                continue;
            };
            let mut shown = tape.shown;
            shown.press = 1.0;
            shown.opacity = l.opacity.unwrap_or(shown.opacity);
            if let Some((t, percent)) = l.translate {
                (shown.translate, shown.translate_percent) = (t, percent);
            }
            shown.scale = l.scale.unwrap_or(shown.scale);
            shown.rotate = l.rotate.unwrap_or(shown.rotate);
            let opacity = shown.opacity.clamp(0.0, 1.0);
            let Some(back) = tape.ts.invert() else {
                continue;
            };
            if opacity <= 0.0 {
                continue;
            }
            let own = match shown.moves() {
                true => shown.transform(tape.rect, tape.origin),
                false => Transform::identity(),
            };
            let g = ts
                .pre_translate(offset.0 - tape.offset.0, offset.1 - tape.offset.1)
                .pre_concat(own)
                .pre_concat(back);
            self.damage.unsupported = true;
            if opacity < 1.0 {
                self.backend.push_opacity(opacity);
            }
            replay(self, l.key, g);
            if opacity < 1.0 {
                self.backend.pop_opacity();
            }
        }
    }
}

/// Draw a tape's ops through `g` (its parent then to now) into the
/// painter's backend. Inside a plane, ops are in the plane's own space.
fn replay(painter: &mut Painter, key: NodeKey, g: Transform) {
    let mut planes = 0usize;
    let mut pops: Vec<usize> = Vec::new();
    let Painter {
        backend,
        exits,
        text,
        ..
    } = painter;
    let b = backend.as_mut();
    for op in &exits.tapes[&key].ops {
        let t = |ts: Transform| if planes == 0 { g.pre_concat(ts) } else { ts };
        match op {
            Op::Fill(s, c, ts) => b.fill(s, *c, t(*ts)),
            Op::Gradient(s, p, ts) => b.fill_gradient(s, p, t(*ts)),
            Op::Backdrop(s, f, ts) => b.backdrop_filter(s, f, t(*ts)),
            Op::Shadow(s, c, sigma, outer, ts) => {
                b.blurred_shadow(s, *c, *sigma, outer, t(*ts));
            }
            Op::TextBlur(sigma) => b.text_blur(*sigma),
            Op::Border(part, ts) => b.fill_border(part, t(*ts)),
            Op::Image(image, dst, clips, ts, tint) => b.image(image, *dst, clips, t(*ts), *tint),
            Op::Island(pixels, dst, ts, mode) => {
                b.island_image(pixels.clone(), *dst, t(*ts), *mode)
            }
            Op::PlaneBegin(h) => {
                let h = match planes {
                    0 => mul(g, h),
                    _ => *h,
                };
                b.plane_begin(h);
                planes += 1;
            }
            Op::PlaneEnd => {
                planes = planes.saturating_sub(1);
                b.plane_end();
            }
            Op::Canvas(pixels, dst, clips, ts) => b.canvas(pixels, *dst, clips, t(*ts)),
            Op::Text(p, palette, origin, ts) => {
                b.text(&mut text.borrow_mut(), p, palette, *origin, t(*ts))
            }
            Op::Svg(s, ts) => {
                let paint = SvgPaint {
                    path: &s.path,
                    fill: s.fill.as_ref().map(Ink2::ink),
                    even_odd: s.even_odd,
                    stroke: s.stroke.as_ref().map(Ink2::ink),
                    order: s.order,
                    width: s.width,
                    cap: s.cap,
                    join: s.join,
                    miter: s.miter,
                    dash: s.dash.clone(),
                    phase: s.phase,
                };
                b.svg_path(&paint, t(*ts));
            }
            Op::SvgClip(clip, ts) => pops.push(b.push_svg_clip(clip, t(*ts))),
            Op::Clip(s, ts) => {
                b.push_clip(s, t(*ts));
                pops.push(1);
            }
            Op::CssClip(path, ts) => pops.push(usize::from(b.push_css_clip(path, t(*ts)))),
            Op::PopClip => {
                for _ in 0..pops.pop().unwrap_or(0) {
                    b.pop_clip();
                }
            }
            Op::Opacity(a) => b.push_opacity(*a),
            Op::PopOpacity => b.pop_opacity(),
            Op::Mask(s, ts) => b.push_mask(s, t(*ts)),
            Op::PopMask(s, mask, ts) => b.pop_mask(s, mask, t(*ts)),
        }
    }
}

/// `g` (affine) after the projective map `h`, both row-major.
fn mul(g: Transform, h: &[f32; 9]) -> [f32; 9] {
    let a = [g.sx, g.kx, g.tx, g.ky, g.sy, g.ty, 0.0, 0.0, 1.0];
    let mut out = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            out[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * h[k * 3 + c]).sum();
        }
    }
    out
}
