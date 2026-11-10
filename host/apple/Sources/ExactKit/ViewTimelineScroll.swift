#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    /// The view timelines this scroller moves, when the host marked it
    /// `viewTimeline`: presented in the frame it moves, from the scroll
    /// callback; inside a batch, once the batch has landed.
    func viewTimelinesScrolled() {
        guard props["viewTimeline"] != nil else { return }
        presenter?.afterBatch { [weak self] in
            guard let self, let session = self.presenter?.session, self.presenter?.views[self.id] === self,
                  let (left, top) = self.scrollOffset else { return }
            session.apply(session.runtime.viewScrolled(self.id, left: left, top: top))
        }
    }

    /// The offset `onScrolled` reports, in CSS px.
    private var scrollOffset: (Double, Double)? {
        #if canImport(UIKit)
        guard let sv = scrollView else { return nil }
        let room = extras?.nativeScrollContentInsets ?? .zero
        return (Double(sv.contentOffset.x + room.left), Double(sv.contentOffset.y + room.top + pageBand.top))
        #else
        guard let o = scroll?.contentView.bounds.origin else { return nil }
        return (Double(o.x), Double(o.y))
        #endif
    }
}
