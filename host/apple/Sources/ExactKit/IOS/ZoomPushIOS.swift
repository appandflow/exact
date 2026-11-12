// @ref LLP 1035.001, the zoom transition on a push: a route pushed with a
// `navigationSource` arrives by UIKit's zoom from the source element in the
// route beneath, as a full-screen presentation does (ModalIOS `present`),
// and lands on its own element with the source's id (an image, as drawn).
// iOS 18's `preferredTransition` applies to a push as to a presentation
// (UIKit, "UIViewController.Transition.zoom"); the swipe back is the
// zoom's own, which `transitionEnded` dispatches as any interactive pop.
#if os(iOS)
import UIKit

extension NavigationHost {
    /// Give `top`, the route about to be pushed over `beneath`, its zoom
    /// transition when it names a `navigationSource`; nothing otherwise, and
    /// nothing before iOS 18.
    func prepareZoomPush(_ top: RouteController, beneath: RouteController?) {
        guard #available(iOS 18.0, *), let beneath, top.node.props["navigationSource"] != nil,
              top.preferredTransition == nil else { return }
        let route = top.node, preceding = beneath.node
        let options = UIViewController.Transition.ZoomOptions()
        let landing = ZoomLanding()
        // The zoom's own swipe back pops the route (`transitionEnded`
        // dispatches it); it begins only where an edge swipe may.
        options.interactiveDismissShouldBegin = { [weak self] context in
            guard let self, context.willBegin else { return false }
            return landing.dismissal(begins: back != nil && !changing && !presenter.modals.inTransition)
        }
        top.appeared = landing.appeared
        let resolve: (UIView, NodeView?) -> CGRect? = { [weak self, weak route] zoomed, sourceView in
            guard let self, let route, let name = route.props["navigationSource"],
                  let target = presenter.views.values.filter({
                      $0.props["id"] == name && $0.window != nil && $0.isDescendant(of: route)
                  }).min(by: { $0.id < $1.id }) else { return nil }
            let source = sourceView.flatMap(ModalHost.zoomImage)
            return ModalHost.zoomAlignment(target: target, fallbackNatural: source?.raster?.image.naturalSize, in: zoomed)
        }
        options.alignmentRectProvider = { context in
            landing.answer(context.zoomedViewController.viewIfLoaded.flatMap { resolve($0, context.sourceView as? NodeView) })
        }
        top.preferredTransition = .zoom(options: options, sourceViewProvider: { [weak self, weak route, weak preceding] _ in
            guard let self, let route, let preceding, presenter.views[preceding.id] === preceding,
                  let name = route.props["navigationSource"] else { return nil }
            let source = presenter.views.values.filter {
                $0.props["id"] == name && $0.window != nil && ($0 === preceding || $0.isDescendant(of: preceding))
            }.min(by: { $0.id < $1.id })
            if let source { prepareZoomSource(source, in: preceding) }
            return source
        })
    }

    /// UIKit's zoom draws its source view's layer inside the container it morphs, clipped to that container's
    /// rounded shape, so an outer `box-shadow`, which lies outside the source's bounds, shows as a hard band
    /// under the card and then an outline round it. A `perspective` on a box holding the source shows the same
    /// band in black on the swipe back, though the source lies flat in it. Until the transition ends
    /// (`restoreZoomSource`), the source casts no outer shadow and the boxes up to its route hold it flat; a
    /// source turned in that space zooms as if it were not.
    func prepareZoomSource(_ source: NodeView, in route: NodeView) {
        restoreZoomSource()
        source.shadowCaster?.isHidden = true
        zoomSource = source
        var next = source.superview
        while let view = next, view !== route {
            if let box = view as? NodeView, box.number("perspective") > 0 {
                box.container.layer.sublayerTransform = CATransform3DIdentity
                zoomFlattened.append(box)
            }
            next = view.superview
        }
    }

    func restoreZoomSource() {
        zoomSource?.shadowCaster?.isHidden = false
        zoomSource = nil
        for box in zoomFlattened { box.applyPerspective() }
        zoomFlattened = []
    }

    /// The zoom's swipe back is decided as the finger lifts, and UIKit ends it most of a second later, after
    /// its tail. The app follows it at the lift, as it follows the Back button at once, so the tab bar's
    /// accessory a pushed route hid returns with the tail and not after it.
    func followZoomSwipe(_ transition: UIViewControllerTransitionCoordinator) {
        guard #available(iOS 18.0, *), let source = transition.viewController(forKey: .from) as? RouteController,
              source.preferredTransition != nil else { return }
        transition.notifyWhenInteractionChanges { [weak self, weak source] context in
            guard let self, let source, !context.isCancelled, source.key == selectedKey, source.search == nil,
                  !presenter.modals.inTransition, presenter.views[source.node.id] === source.node else { return }
            followedPop = source
            goBack()
        }
    }
}
#endif
