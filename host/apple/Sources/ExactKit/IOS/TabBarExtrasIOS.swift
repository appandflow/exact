// The system tab bar's iOS 26 behaviours that have no CSS word, read from the
// authored tablist: `-exact-tab-minimize` (UIKit's `tabBarMinimizeBehavior`),
// a `pan` on the tablist heard on the bar (`-exact-pan-hold`, HoldPanIOS), and
// `-exact-tab-accessory`, a tablist child shown as the bar's bottom accessory
// (`UITabAccessory`).
#if os(iOS) || os(tvOS)
import UIKit

/// What the bar's accessory holds: the node, the view UIKit sizes for it, the
/// `UITabAccessory`, and a picture of a node a batch destroyed, which the
/// accessory fades out with instead of an empty capsule.
final class TabBarExtras {
    var node: NodeView?
    var view: TabAccessoryView?
    var accessory: AnyObject?
    var snapshot: UIView?
}

/// The view UIKit sizes for the tab bar's bottom accessory: the accessory node's
/// containing block, told again whenever UIKit gives it another size (the bar
/// minimizing puts the accessory inline beside it).
final class TabAccessoryView: UIView {
    weak var host: NavigationHost?
    private var told: CGSize?

    override func layoutSubviews() {
        super.layoutSubviews()
        guard bounds.size != told else { return }
        told = bounds.size
        host?.presenter.owners.publish()
    }
}

extension NavigationHost {
    var tabAccessory: NodeView? { barExtras.node }

    /// A new container's bar: its minimize behaviour and the tablist's pan.
    func installBarExtras(_ container: UITabBarController, _ tabs: NavigationTabs) {
        #if os(iOS)
        if #available(iOS 26.0, *) {
            container.tabBarMinimizeBehavior = switch tabs.tablist.props["tabMinimize"] {
            case "on-scroll-down": .onScrollDown
            case "on-scroll-up": .onScrollUp
            case "never": .never
            default: .automatic
            }
        }
        // The tablist node is hidden in the bar's place, so its pan is installed on the bar, held or not.
        if tabs.tablist.handlers.contains("pan") {
            let hold = tabs.tablist.props["panHold"].flatMap(Double.init) ?? 0
            let g = HoldLayoutPan(target: tabs.tablist, action: #selector(NodeView.holdPanning(_:)))
            g.minimumPressDuration = max(hold, 1) / 1000
            g.cancelsTouchesInView = true
            g.overridesSiblings = true
            container.tabBar.addGestureRecognizer(g)
        }
        #endif
    }

    /// The accessory's block, and the tablist's: an empty one, so the
    /// tablist, no longer `display: none` while its child is laid out, takes
    /// no room.
    var accessoryBlocks: [UInt32: HostBlock] {
        guard let node = barExtras.node, let view = barExtras.view, let list = adoptedTablist else { return [:] }
        return [list: .size(.zero), node.id: .size(view.bounds.size)]
    }

    /// Before a batch applies: what it destroys of the accessory.
    func prepareExtras(_ batch: Batch) {
        prepareAccessory(batch)
    }

    /// A picture of an accessory node the batch destroys.
    private func prepareAccessory(_ batch: Batch) {
        guard let node = barExtras.node, node.superview === barExtras.view,
              batch.ops.contains(where: { $0.op == .destroy && $0.id == node.id }),
              let picture = node.snapshotView(afterScreenUpdates: false) else { return }
        picture.frame = node.frame
        barExtras.snapshot?.removeFromSuperview()
        barExtras.view?.addSubview(picture)
        barExtras.snapshot = picture
    }

    /// Where the bar shows again for a root whose tablist shows: its accessory
    /// in the same call and with the same animation, as UIKit's animated
    /// install shows nothing for about 100 ms.
    func syncAccessoryWithBar(animated: Bool) {
        if let authored = container.flatMap({ NavigationTabs.of($0, presenter) }) { syncAccessory(authored, animated: animated) }
    }

    /// Each batch: the tablist's `-exact-tab-accessory` child, shown and not
    /// `display: none`, is the bar's bottom accessory; none, the bar has none.
    /// iOS 26, and only where the system bar takes the tablist's place.
    func syncAccessory(_ tabs: NavigationTabs, animated: Bool = true) {
        #if os(iOS)
        guard #available(iOS 26.0, *) else { return }
        let list = tabs.tablist
        let shown = { (node: NodeView) in
            self.presenter.views[node.id] === node && node.props["tabAccessory"] == "true" && node.style["display"]?.string != "none"
        }
        var wanted: NodeView?
        if let container = tabController, tabOwner === container, tabBarShows, !tablistHidBar, adopts(tablist: list) {
            if let held = barExtras.node, held.superview === barExtras.view, shown(held) { wanted = held }
            else { wanted = list.container.subviews.lazy.compactMap { $0 as? NodeView }.first(where: shown) }
        }
        guard wanted !== barExtras.node else { return }
        if barExtras.node != nil { releaseAccessory(into: list) }
        guard let node = wanted, let container = tabController else { return }
        let view = barExtras.view ?? TabAccessoryView()
        view.host = self
        barExtras.view = view
        barExtras.snapshot?.removeFromSuperview()
        barExtras.snapshot = nil
        view.addSubview(node)
        barExtras.node = node
        let made = (barExtras.accessory as? UITabAccessory) ?? UITabAccessory(contentView: view)
        barExtras.accessory = made
        container.setBottomAccessory(made, animated: animated && container.view.window != nil)
        reportCovers()
        presenter.owners.publish()
        #endif
    }

    /// The accessory goes: its node back under the tablist (`list`, else the
    /// one adopted) while it is still the tablist's, and the bar's accessory with it.
    func releaseAccessory(into list: NodeView? = nil) {
        guard let node = barExtras.node else { return }
        barExtras.node = nil
        #if os(iOS)
        if #available(iOS 26.0, *), let container = tabController, container.bottomAccessory != nil {
            container.setBottomAccessory(nil, animated: container.view.window != nil)
        }
        #endif
        if node.superview === barExtras.view {
            node.removeFromSuperview()
            if presenter.views[node.id] === node, let parent = list ?? adoptedTablist.flatMap({ presenter.views[$0] }) {
                parent.container.addSubview(node)
                presenter.regrid(node)
            }
        }
        reportCovers()
        presenter.owners.publish()
    }
}
#endif
