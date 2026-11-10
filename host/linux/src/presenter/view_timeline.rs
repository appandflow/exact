//! View timelines (`view-timeline`): after each layout and each scroll the
//! engine is given each subject's progress at its scroller's offset, so a
//! scroll moves the consumers in its own frame, with no action in the path.
use super::*;

impl<D: DataSource> Presenter<D> {
    pub(super) fn place_view_timelines(&mut self) {
        let kernel = self.host.kernel();
        let timelines = kernel.view_timelines();
        let progress: Vec<(u64, f64)> = (timelines.iter())
            .map(|t| {
                let at = (kernel.node_by_key(t.scroller))
                    .and_then(|n| self.scroll.get(&n.id).copied())
                    .unwrap_or((0.0, 0.0));
                (t.subject, t.progress(at.0.into(), at.1.into()))
            })
            .collect();
        // Placed again only when one moved: presenting asks for a frame.
        // Before a paint, or in a scroll that marks the presenter dirty
        // itself, the values reach the next picture either way.
        if progress != self.view_progress {
            self.host.view_progress(&progress);
            self.view_progress = progress;
        }
    }
}
