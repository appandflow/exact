// `-exact-pan-hold`: a pan that begins only once the finger has rested, as
// a long press that keeps going (holding a tab bar to unfold a dial, then
// spinning it with the same touch). UIKit's long press keeps reporting the
// finger after it is recognized; it reports no velocity, so the release's is
// measured from the last samples, as UIKit's pan measures its own.
#if os(iOS)
import UIKit

final class HoldLayoutPan: UILongPressGestureRecognizer, UIGestureRecognizerDelegate {
    /// On a view with gestures of its own (UITabBar's press-and-slide), the hold tracks alongside them and,
    /// once recognized, cancels them, so the touch is the hold's whichever began first.
    var overridesSiblings = false
    /// Where the hold began and where it last reported, in the viewport.
    var origin = CGPoint.zero
    var last = CGPoint.zero
    /// Recent samples, for the release's velocity.
    var samples: [(time: TimeInterval, point: CGPoint)] = []
    override init(target: Any?, action: Selector?) {
        super.init(target: target, action: action)
        // The finger may drift as far as a press's slop while it rests.
        allowableMovement = 14
        delegate = self
    }
    override func canBePrevented(by preventing: UIGestureRecognizer) -> Bool {
        overridesSiblings ? false : super.canBePrevented(by: preventing)
    }
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        overridesSiblings
    }
    /// Ends the other recognizers holding this touch (toggling `isEnabled` cancels one in flight).
    func cancelSiblings() {
        guard overridesSiblings, let view else { return }
        var stack: [UIView] = [view]
        while let v = stack.popLast() {
            for g in v.gestureRecognizers ?? [] where g !== self && [.began, .changed, .possible].contains(g.state) {
                g.isEnabled = false; g.isEnabled = true
            }
            stack.append(contentsOf: v.subviews)
        }
    }
    override func reset() {
        super.reset()
        samples.removeAll(); origin = .zero; last = .zero
    }
    /// Points per second over the last ~80 ms of the contact.
    func velocity() -> CGPoint {
        guard let end = samples.last else { return .zero }
        guard let start = samples.first(where: { end.time - $0.time <= 0.08 }), end.time - start.time > 0.001 else { return .zero }
        let dt = end.time - start.time
        return CGPoint(x: (end.point.x - start.point.x) / dt, y: (end.point.y - start.point.y) / dt)
    }
}

extension NodeView {
    @objc func holdPanning(_ gesture: UIGestureRecognizer) {
        guard let gesture = gesture as? HoldLayoutPan else { return }
        // On the system tab bar the tablist node is hidden in the bar's place: the bar is what takes input.
        let allowed = gesture.overridesSiblings ? gesture.view?.window != nil : SwipeInput.allows(self)
        guard let presenter, presenter.views[id] === self, allowed else {
            if let presenter, presenter.views[id] === self, [.changed, .ended].contains(gesture.state) { presenter.panRelease(id, 0, 0) }
            gesture.isEnabled = false; gesture.isEnabled = true; return
        }
        let port = presenter.port(of: self)
        let point = gesture.location(in: port)
        gesture.samples.append((CACurrentMediaTime(), point))
        if gesture.samples.count > 12 { gesture.samples.removeFirst(gesture.samples.count - 12) }
        switch gesture.state {
        case .began:
            // One zero delta says the hold began, before the finger moves.
            gesture.origin = point; gesture.last = point
            gesture.cancelSiblings()
            presenter.pan(id, 0, 0)
        case .changed:
            let dx = point.x - gesture.last.x, dy = point.y - gesture.last.y
            gesture.last = point
            if dx != 0 || dy != 0 { presenter.pan(id, Double(dx), Double(dy)) }
        case .ended:
            let dx = point.x - gesture.last.x, dy = point.y - gesture.last.y
            if dx != 0 || dy != 0 { presenter.pan(id, Double(dx), Double(dy)) }
            let v = gesture.velocity()
            presenter.panRelease(id, Double(v.x), Double(v.y))
        case .cancelled, .failed:
            presenter.panRelease(id, 0, 0)
        default:
            break
        }
    }
}
#endif
