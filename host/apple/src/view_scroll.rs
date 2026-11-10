//! View timelines on Apple. After each layout the host places every view
//! timeline at its scroller's offset and marks the scroller's view
//! (`viewTimeline`); the presenter's scroll callback then calls
//! [`Host::view_scrolled`] on the main thread, which presents the consumers in
//! the frame the scroller moves.

use super::*;
use exact_kernel::timeline::ViewTimeline;

/// What the last layout placed, and where each scroller stands.
#[derive(Default)]
pub(super) struct ViewScrolls {
    timelines: Vec<ViewTimeline>,
    /// Each scroller a view timeline scrolls in, and its offset as the
    /// presenter last gave it (zero until it scrolls).
    scrollers: Vec<(ViewId, NodeKey, f64, f64)>,
}

impl<D: DataSource> Host<D> {
    /// Place every view timeline from the layout just run, give the engine
    /// each one's progress, and mark the scrollers whose scroll moves one.
    pub(super) fn place_view_timelines(&mut self, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let timelines = kernel.view_timelines();
        if timelines.is_empty() && self.view_scrolls.scrollers.is_empty() {
            return;
        }
        let mut scrollers: Vec<(ViewId, NodeKey, f64, f64)> = Vec::new();
        for t in &timelines {
            if scrollers.iter().any(|s| s.1 == t.scroller) {
                continue;
            }
            let Some(id) = kernel.node_by_key(t.scroller).map(|n| n.id) else {
                continue;
            };
            let kept = self.view_scrolls.scrollers.iter().find(|s| s.0 == id);
            if kept.is_none() || batch.creates(id) {
                batch.props(id, &[("viewTimeline", "1".into())], &[]);
            }
            scrollers.push(kept.copied().unwrap_or((id, t.scroller, 0.0, 0.0)));
        }
        for s in &self.view_scrolls.scrollers {
            if !scrollers.iter().any(|n| n.0 == s.0) && kernel.node(s.0).is_some() {
                batch.props(s.0, &[], &["viewTimeline"]);
            }
        }
        for t in &timelines {
            if let Some(&(_, _, left, top)) = scrollers.iter().find(|s| s.1 == t.scroller) {
                self.engine
                    .set_view_progress(t.subject, t.progress(left, top));
            }
        }
        self.view_scrolls = ViewScrolls {
            timelines,
            scrollers,
        };
    }

    /// The presenter's scroller `view` now stands at `(left, top)` CSS px:
    /// the view timelines it moves, presented.
    pub fn view_scrolled(&mut self, view: ViewId, left: f64, top: f64) -> String {
        let mut batch = Batch::new();
        let scrolls = &mut self.view_scrolls;
        if let Some(s) = scrolls.scrollers.iter_mut().find(|s| s.0 == view) {
            (s.2, s.3) = (left, top);
            for t in scrolls.timelines.iter().filter(|t| t.scroller == s.1) {
                self.engine
                    .set_view_progress(t.subject, t.progress(left, top));
            }
        }
        self.present(&mut batch, false);
        self.finish(batch, None)
    }
}
