// UIKit's pinch zoom on a scroll container with one child
// (`minimumZoomScale`, `maximumZoomScale`, `zoomScale`). UIKit zooms a view
// of its own that it alone writes: a node's transform is the host's
// (`applyTransform`), and a view two writers transform shows whichever wrote
// last. The child is drawn from the scroller's scale through
// `hostPlacement`, so its own transform, a press and an animation compose
// with it. The child keeps its layout: its box in the scroller is its size
// times the scale, what layout put around it stays as it is, and it is
// centred while the whole is smaller than the port, as a photo is.
#if os(iOS)
import UIKit

extension NodeView {
    /// The range a pinch zooms through, when the props give one.
    var zoomRange: ClosedRange<CGFloat>? {
        guard let low = props["minimumZoomScale"].flatMap(Double.init), let high = props["maximumZoomScale"].flatMap(Double.init),
              low.isFinite, high.isFinite, low > 0, high > low else { return nil }
        return CGFloat(low)...CGFloat(high)
    }
    /// The content extent with the zoomed child at its scale.
    var zoomedContent: CGSize {
        guard let e = extras, e.zoomProxy != nil, let sv = scroll, let child = e.zoomed, let size = child.grid?.drawn.size else { return content }
        let grown = sv.zoomScale - 1
        return CGSize(width: content.width + size.width * grown, height: content.height + size.height * grown)
    }

    /// After a batch: the range and the scale the props give, and the child
    /// where they put it.
    func settleZoom() {
        guard let sv = scroll else { return }
        let nodes = sv.subviews.compactMap { $0 as? NodeView }
        guard let range = zoomRange, nodes.count == 1, nodes[0].grid != nil else { return dropZoom() }
        let opening = extras?.zoomProxy == nil
        if extras?.zoomed !== nodes[0] { dropZoom() }
        if extras?.zoomProxy == nil {
            let proxy = UIView()
            proxy.isUserInteractionEnabled = false
            sv.insertSubview(proxy, at: 0)
            more.zoomProxy = proxy
            more.zoomed = nodes[0]
            placeZoom()
        }
        if sv.minimumZoomScale != range.lowerBound { sv.minimumZoomScale = range.lowerBound }
        if sv.maximumZoomScale != range.upperBound { sv.maximumZoomScale = range.upperBound }
        let asked = extras?.pendingZoomScale.map { CGFloat($0) }
        more.pendingZoomScale = nil
        if !sv.isZooming, !sv.isZoomBouncing {
            let scale = min(max(asked ?? sv.zoomScale, range.lowerBound), range.upperBound)
            if scale != sv.zoomScale {
                let offset = sv.contentOffset
                sv.zoomScale = scale
                // A scroller that opens zoomed opens at its start.
                if opening { sv.contentOffset = offset }
            }
        }
        placeZoom()
    }

    /// The child drawn at the scroller's scale, the extent that gives, and
    /// UIKit's view over where the child shows, which its next step zooms from.
    func placeZoom() {
        guard let sv = scroll, let e = extras, let proxy = e.zoomProxy, let child = e.zoomed, let place = child.grid?.drawn else { return }
        let scale = sv.zoomScale, extent = zoomedContent
        let shift = CGPoint(x: max(0, (sv.bounds.width - extent.width) / 2), y: max(0, (scrollFloor(sv) - extent.height) / 2))
        if child.zoomFactor != scale || child.zoomShift != shift {
            child.zoomFactor = scale; child.zoomShift = shift
            child.applyTransform()
        }
        fitScroll()
        if proxy.bounds.size != place.size { proxy.bounds.size = place.size }
        let center = CGPoint(x: place.minX + shift.x + place.width * scale / 2, y: place.minY + shift.y + place.height * scale / 2)
        if proxy.center != center { proxy.center = center }
    }

    /// No zoom any more: the child as layout placed it.
    func dropZoom() {
        guard let e = extras, let proxy = e.zoomProxy else { return }
        if let sv = scroll {
            if sv.zoomScale != 1 { sv.zoomScale = 1 }
            sv.minimumZoomScale = 1; sv.maximumZoomScale = 1
        }
        proxy.removeFromSuperview()
        e.zoomProxy = nil
        if let child = e.zoomed, child.zoomFactor != 1 || child.zoomShift != .zero {
            child.zoomFactor = 1; child.zoomShift = .zero
            child.applyTransform()
        }
        e.zoomed = nil
        fitScroll()
    }

    package func viewForZooming(in scrollView: UIScrollView) -> UIView? {
        scrollView === scroll ? extras?.zoomProxy : nil
    }
    package func scrollViewDidZoom(_ scrollView: UIScrollView) {
        guard scrollView === scroll else { return }
        placeZoom()
        queueScrollEvent()
    }
}
#endif
