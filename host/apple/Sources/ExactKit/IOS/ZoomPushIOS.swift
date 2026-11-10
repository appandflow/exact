// The zoom transition on a push: a route pushed with a
// `navigationSource` arrives by UIKit's zoom from the source element in the
// route beneath, as a full-screen presentation does (ModalIOS `present`),
// and lands on its own element with the source's id (an image, as drawn).
// iOS 18's `preferredTransition` applies to a push as to a presentation
// (UIKit, "UIViewController.Transition.zoom"); the swipe back is the
// zoom's own, which `transitionEnded` dispatches as any interactive pop.
#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// Hears each touch on a stack before the view under it, and takes none.
final class ZoomTailTouch: UIGestureRecognizer {
    private let began: () -> Void
    init(began: @escaping () -> Void) {
        self.began = began
        super.init(target: nil, action: nil)
        cancelsTouchesInView = false
        delaysTouchesEnded = false
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        began()
        state = .failed
    }
}

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
            return presenter.views.values.filter {
                $0.props["id"] == name && $0.window != nil && ($0 === preceding || $0.isDescendant(of: preceding))
            }.min(by: { $0.id < $1.id })
        })
    }

    /// A zoom's pop is decided well before UIKit ends it: at the finger's lift for a swipe, at
    /// once for the Back button. Its tail runs for most of a second with the route beneath already
    /// taking touches, as no other pop's does. A tap there reached an app still on the popped route,
    /// and its push kept that route under the new one: Back then showed the product just left.
    /// So the app follows a swipe as the finger lifts, as it follows the Back button at once
    /// (`followDecidedPop`): the tab bar and its accessory a pushed route hid come back with
    /// the tail and not after it. A touch in the tail of any other decided pop does the same.
    func noteZoomPop(_ transition: UIViewControllerTransitionCoordinator, on nav: UINavigationController) {
        guard #available(iOS 18.0, *), let source = transition.viewController(forKey: .from) as? RouteController,
              source.preferredTransition != nil, !nav.viewControllers.contains(source) else { return }
        if !(nav.view.gestureRecognizers ?? []).contains(where: { $0 is ZoomTailTouch }) {
            nav.view.addGestureRecognizer(ZoomTailTouch { [weak self] in self?.followDecidedPop() })
        }
        if transition.initiallyInteractive {
            transition.notifyWhenInteractionChanges { [weak self, weak source] context in
                guard !context.isCancelled, let self else { return }
                decidedPop = source
                followDecidedPop()
            }
        } else {
            decidedPop = source
        }
    }

    /// iOS 27 keeps a large title in the route's scroller, placed from that scroller's geometry.
    /// A swiped zoom scales the route beneath, and UIKit places the title against that scale and
    /// not again as the route grows back: it comes to rest 17 points left and 27 up, stretched by
    /// the same scale. A layout of the scroller holding it puts it back, each frame of the zoom's
    /// tail (`revealTick`) so it never shows there, and once at rest.
    func settleLargeTitle(_ nav: UINavigationController) {
        guard let route = nav.topViewController as? RouteController, let scroll = contentScrollView(of: route) else { return }
        scroll.setNeedsLayout()
        scroll.layoutIfNeeded()
    }
}
#endif
