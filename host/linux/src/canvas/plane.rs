//! A box turned in space (`paint/space.rs`) drawn as ops under its plane's
//! homography: `MATRIX3 a b c d e f g h i` is the whole 3×3 matrix, row-major
//! as Android's `Matrix.setValues` takes it, perspective included, so the
//! reader draws the box's own rects, pictures and shadows turned, rather than
//! a picture of it painted and warped on the CPU.
//! Inside a plane nothing is a rect in page space: every clip is written as
//! it is pushed, the clips around the box before it, and nothing is culled.
use super::*;
use crate::paint::space::mul3;

impl Recorder {
    /// Draw what follows through `h` (its points to the points the walk's
    /// transforms otherwise give), within the plane already open.
    pub(super) fn plane_open(&mut self, h: [f32; 9]) -> bool {
        if !self.clips.is_empty() {
            self.materialize(self.clips.len() - 1);
        }
        let h = match self.planes.last() {
            Some(outer) => mul3(outer, &h),
            None => h,
        };
        self.planes.push(h);
        true
    }

    pub(super) fn plane_close(&mut self) {
        self.planes.pop();
        self.matrix = None;
    }

    /// In a plane: write `MATRIX3` for `ts` within it, and true.
    pub(super) fn plane_matrix(&mut self, ts: Transform) -> bool {
        let Some(p) = self.planes.last() else {
            return false;
        };
        let m = mul3(p, &[ts.sx, ts.kx, ts.tx, ts.ky, ts.sy, ts.ty, 0., 0., 1.]);
        let (s, (ox, oy)) = (self.scale, self.origin);
        self.matrix = None;
        self.ops.push(MATRIX3);
        for v in [
            (m[0] - ox * m[6]) * s,
            (m[1] - ox * m[7]) * s,
            (m[2] - ox * m[8]) * s,
            (m[3] - oy * m[6]) * s,
            (m[4] - oy * m[7]) * s,
            (m[5] - oy * m[8]) * s,
            m[6],
            m[7],
            m[8],
        ] {
            self.f(v);
        }
        true
    }

    /// `r` under `ts` in page space, when that is a rect: outside any plane,
    /// and only scaled and moved.
    pub(super) fn on_page(&self, r: Rect4, ts: Transform) -> Option<Rect4> {
        if self.planes.is_empty() {
            clip::map(r, ts)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::{PainterChoice, Presenter};

    /// A presenter painting on the recorder and its last stream, with the
    /// process's stream queue held (its later paints fill it) and emptied
    /// when the test ends.
    struct Booted {
        p: Presenter<()>,
        ops: Vec<u32>,
        _queue: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for Booted {
        fn drop(&mut self) {
            stream::reader_replaced();
        }
    }

    fn ops(source: &str) -> Booted {
        let queue = stream::QUEUE.lock().unwrap_or_else(|e| e.into_inner());
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            (),
            (400., 600.),
            2.,
            std::path::PathBuf::new(),
            PainterChoice::Canvas,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        let ops = FINISHED.with(|f| f.borrow_mut().take()).expect("a stream");
        Booted {
            p,
            ops,
            _queue: queue,
        }
    }

    fn map(m: &[f32], x: f32, y: f32) -> (f32, f32) {
        let w = m[6] * x + m[7] * y + m[8];
        (
            (m[0] * x + m[1] * y + m[2]) / w,
            (m[3] * x + m[4] * y + m[5]) / w,
        )
    }

    #[test]
    fn a_box_turned_under_a_perspective_is_its_ops_under_matrix3() {
        let mut booted = ops(
            "component App\n  view\n    view perspective=600 width=200 height=100 margin=100\n      \
             view rotate=\"y 40deg\" width=200 height=100 background-color=\"#ff0000\" testId=\"card\"\n",
        );
        let (p, ops) = (&mut booted.p, std::mem::take(&mut booted.ops));
        let mut found = None;
        let mut i = 0;
        while i < ops.len() {
            match ops[i] {
                IMAGE_DEF => panic!("the box was painted apart into a picture"),
                MATRIX3 if ops.get(i + 10) == Some(&RRECT) => {
                    let m: Vec<f32> = ops[i + 1..i + 10]
                        .iter()
                        .map(|v| f32::from_bits(*v))
                        .collect();
                    let r: Vec<f32> = ops[i + 12..i + 16]
                        .iter()
                        .map(|v| f32::from_bits(*v))
                        .collect();
                    found = Some((m, r));
                }
                _ => (),
            }
            i = stream::end(&ops, i).unwrap();
        }
        let (m, r) = found.expect("the box's fill under MATRIX3");
        let (x, y, w, h) = (r[0], r[1], r[2], r[3]);
        // Turned about its centre line, which stays where it was (device
        // pixels, scale 2): the perspective origin is that line's centre.
        let (cx, cy) = map(&m, x + w / 2., y + h / 2.);
        assert!(
            (cx - 400.).abs() < 0.01 && (cy - 300.).abs() < 0.01,
            "{cx} {cy}"
        );
        let edge = |ex: f32| {
            let (top, bottom) = (map(&m, ex, y), map(&m, ex, y + h));
            (top.0, bottom.1 - top.1)
        };
        let ((lx, left), (rx, right)) = (edge(x), edge(x + w));
        // rotate y with a positive angle brings the left edge nearer: taller,
        // and both edges come in from where the flat box had them.
        assert!(left > 200. && right < 200., "left {left}, right {right}");
        assert!(lx > 200. && rx < 600., "{lx} {rx}");
        // The box is hit where it paints, and not beside its turned edge.
        let card = p.host().kernel().find_by_test_id("card")[0];
        let card = p.host().kernel().node_by_key(card).unwrap().id;
        assert_eq!(p.hit(200., 150.), Some(card));
        assert_ne!(p.hit(lx / 2. - 4., 150.), Some(card));
    }

    #[test]
    fn a_turned_box_in_a_scrollers_row_is_hit_where_the_row_is() {
        let card = |n: u32| {
            format!(
                "      view perspective=600 width=150 height=100 flex-shrink=0\n        \
                 view rotate=\"y 20deg\" width=150 height=100 background-color=\"#ff0000\" testId=\"c{n}\"\n"
            )
        };
        let mut booted = ops(&format!(
            "component App\n  view\n    row overflow-x=\"auto\" width=400 height=100 margin-top=50\n{}{}{}",
            card(0),
            card(1),
            card(2)
        ));
        let p = &mut booted.p;
        let id = |p: &Presenter<()>, n: &str| {
            let k = p.host().kernel();
            k.node_by_key(k.find_by_test_id(n)[0]).unwrap().id
        };
        let (c1, c2) = (id(p, "c1"), id(p, "c2"));
        assert_eq!(p.hit(225., 100.), Some(c1));
        assert_eq!(p.hit(375., 100.), Some(c2));
    }
}
