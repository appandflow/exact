//! The display's scale as a layout input (LLP 1115.000 D10.3): the page's
//! `devicePixelRatio`, which a hairline (`-exact-hairline`) is one device
//! pixel of. The glue says it after boot and when the page's facts change,
//! beside the root font size.

use super::*;

/// What the glue reads of the page (`exact_set_viewport_geometry`,
/// `exact_resize`): the document's safe-area insets, the segment rectangles
/// row-major, and the software keyboard's overlap, in CSS pixels.
pub type PageGeometry = ([f32; 4], Vec<[f32; 4]>, f32);

impl<D: DataSource> Host<D> {
    /// Publish ephemeral HUD metrics without involving the application data seam.
    pub fn canvas_performance_record(&mut self, name: &str, json: Option<&str>) -> String {
        let (receipts, error) = match self.runner.set_canvas_performance(name, json) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("canvas performance: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }

    /// Keep numeric viewport geometry in the same coordinate space as CSS
    /// env(): the page's lengths, the keyboard's overlap among them, answered
    /// in one commit (LLP 1039 D9).
    pub fn viewport_geometry(&mut self, geometry: PageGeometry) -> String {
        let before = self.runner.kernel().env();
        if let Err(error) = self.put_geometry(&geometry, self.runner.viewport().fold) {
            return self.batch_for(&[], Some(&error));
        }
        let answer = self.runner.set_view(None, Some(f64::from(geometry.2)));
        if answer.is_err() {
            self.restore_geometry(before);
        }
        let (receipts, error) = match answer {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("viewport geometry: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }

    /// The page's insets and segment rectangles into the kernel, on `fold`'s
    /// grid, for the runner to read in its next viewport commit; a refused
    /// geometry leaves the kernel as it was.
    pub(crate) fn put_geometry(
        &mut self,
        (insets, rects, _): &PageGeometry,
        fold: exact_runner::Fold,
    ) -> Result<(), String> {
        let [top, right, bottom, left] = *insets;
        let (Ok(cols), Ok(rows)) = (u8::try_from(fold.cols), u8::try_from(fold.rows)) else {
            return Err("viewport geometry: grid exceeds 255".into());
        };
        let before = self.runner.kernel().env();
        let env = before.with_insets(top, right, bottom, left).with_segments(
            cols,
            rows,
            (rects.iter())
                .map(|[x, y, w, h]| exact_kernel::Rect::new(*x, *y, *w, *h))
                .collect(),
        );
        if !env.is_finite() || !env.segments_consistent() {
            return Err("viewport geometry: invalid insets or segment grid".into());
        }
        let kernel = self.runner.kernel_mut();
        if let Err(error) = kernel
            .set_env(env.clone())
            .and_then(|_| kernel.set_segments(cols, rows, env.segments))
        {
            self.restore_geometry(before);
            return Err(format!("viewport geometry: {error:?}"));
        }
        Ok(())
    }

    pub(crate) fn restore_geometry(&mut self, before: exact_kernel::Env) {
        let kernel = self.runner.kernel_mut();
        let _ = kernel.set_env(before.clone());
        let _ = kernel.set_segments(before.cols, before.rows, before.segments);
    }

    /// Tell the runner the display's scale: a relayout in one batch where a
    /// row is a hairline, nothing where none is.
    pub fn set_display_scale(&mut self, scale: f64) -> String {
        let (receipts, error) = match self.runner.set_display_scale(scale) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("display scale: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }
}
