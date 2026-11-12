// @ref LLP 1141.002 D2 — Exact's tab container built from `UITab`s
// (`UITabBarController.tabs`, iOS 18), which the app's search tab needs and
// `viewControllers` cannot reach: a `UISearchTab` (LLP 1141.002 D1, D3).
// A bar of more than five tabs keeps the `viewControllers` path, as do
// iOS 17, tvOS and a bar with no search tab (NavigationTabsIOS.swift): in a
// compact width UIKit lists a sixth tab under More, which Exact follows
// through More's navigation controller, and on the `UITab` path More shows
// a tab's routes but `selectedViewController` names the tab's own emptied
// controller (iPadOS 27, measured), which that following does not handle.
//
// Each `UITab` provides the tab's existing stack (`tabStacks`), so what a
// tab holds and the router's handshake are the same on both paths:
// `shouldSelectTab` asks `mayShow` with the tab's controller, and
// `didSelectTab` is `followUIKit`. UIKit sends `didSelectTab` for a
// selection made in code too (iOS 26 and 27, measured), which `followUIKit`
// takes as the router's own. Tab identity is the panel's `id`; a
// `UISearchTab`'s identifier is UIKit's, so tabs are found by object.
#if os(iOS) || os(tvOS)
import UIKit

extension NavigationHost {
    /// Whether the root's tabs are built from `UITab`s: on iOS 18 and later,
    /// five or fewer, which no width lists under More, with a search tab.
    func buildsTabs(_ tabs: NavigationTabs) -> Bool {
        #if os(iOS)
        guard #available(iOS 18.0, tvOS 18.0, *), tabs.panels.count <= 5 else { return false }
        return Self.searchTab(tabs) != nil
        #else
        return false
        #endif
    }

    /// The app's search tab: the first whose `rel` holds the token `search`,
    /// ASCII case-insensitively (LLP 1141.002 D1, `Kernel::root_tabs`).
    static func searchTab(_ tabs: NavigationTabs) -> Int? {
        tabs.tabs.firstIndex { tab in
            (tab.props["rel"] ?? "").split(whereSeparator: { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" || $0 == "\u{0C}" })
                .contains { $0.lowercased() == "search" }
        }
    }

    /// The tabs for `navs`, the stacks in tab order: a `UISearchTab` for
    /// the search tab, else a `UITab` whose identifier is its panel's `id`.
    @available(iOS 18.0, tvOS 18.0, *)
    func makeTabs(_ navs: [UIViewController], _ tabs: NavigationTabs) -> [UITab] {
        let search = Self.searchTab(tabs)
        return zip(tabs.panels, navs).enumerated().map { index, pair in
            let (panel, nav) = pair
            if index == search { return UISearchTab { _ in nav } }
            return UITab(title: "", image: nil, identifier: panel.props["id"] ?? "panel-\(panel.id)") { _ in nav }
        }
    }

    /// Fill `container` with tabs for `navs`, selecting tab `selected`.
    @available(iOS 18.0, tvOS 18.0, *)
    func buildTabs(_ container: UITabBarController, _ navs: [UIViewController], _ tabs: NavigationTabs, selected: Int) {
        let made = makeTabs(navs, tabs)
        builtTabs = made
        // The faces `syncItems` already wrote on each stack's item
        for (index, nav) in navs.enumerated() { writeBuiltTab(index, item: nav.tabBarItem, title: nav.title ?? "") }
        container.setTabs(made, animated: false)
        if made.indices.contains(selected) { container.selectedTab = made[selected] }
        let search = Self.searchTab(tabs)
        presenter.session?.log("navigation: the tabs are UITabs\(search.map { ", tab \($0) the search tab" } ?? "")")
        // The search tab's form is its root's field's: build its panel now, after this first frame, so the bar knows
        // it before a person can reach it (a panel is otherwise built when its tab is first selected)
        if let search, search != selected, tabs.panels.indices.contains(search) {
            let panel = tabs.panels[search].id
            presenter.afterBatch { [weak self] in
                guard let self, logicalChildren[panel]?.isEmpty != false, tabPanels.contains(panel) else { return }
                presenter.session?.buildTabs([panel])
            }
        }
    }

    /// Whether the tabs as built differ from what the root's tabs now ask
    /// for: the path, or the search tab (`reshape`).
    func builtTabsChanged(_ p: Projection) -> Bool {
        guard tabController != nil, tabOwner === tabController, let tabs = p.tabs else { return false }
        let wanted = buildsTabs(tabs)
        return wanted != !builtTabs.isEmpty || wanted && builtSearch != Self.searchTab(tabs)
    }

    /// The built search tab's index.
    var builtSearch: Int? {
        guard #available(iOS 18.0, tvOS 18.0, *) else { return nil }
        return builtTabs.firstIndex { $0 is UISearchTab }
    }

    /// The tab whose `UITab` is `tab`.
    func tabIndex(ofTab tab: AnyObject) -> Int? { builtTabs.firstIndex { $0 === tab } }

    /// Tab `index`'s controller for UIKit's `tab`: its own stack, or for a
    /// tab not Exact's (UIKit's More) the controller UIKit gives it.
    @available(iOS 18.0, tvOS 18.0, *)
    func controller(ofTab tab: UITab) -> UIViewController? {
        tabIndex(ofTab: tab).flatMap(tabStack) ?? tab.viewController
    }

    /// The tabs again, when an app's `tabContainer` took their stacks and
    /// returned no container (`replayTabs`); whether it did.
    func restoreBuiltTabs(_ container: UITabBarController, _ navs: [UIViewController]) -> Bool {
        guard #available(iOS 18.0, tvOS 18.0, *), !builtTabs.isEmpty else { return false }
        guard navs.contains(where: { $0.parent == nil }), let root = self.container, let tabs = NavigationTabs.of(root, presenter),
              tabs.panels.count == navs.count else { return true }
        presenter.session?.log("hatch: tabContainer took the tabs' stacks but returned no container; Exact's has them back")
        let made = makeTabs(navs, tabs)
        builtTabs = made
        for (index, nav) in navs.enumerated() { writeBuiltTab(index, item: nav.tabBarItem, title: nav.title ?? "") }
        container.setTabs(made, animated: false)
        if made.indices.contains(routerTab) { container.selectedTab = made[routerTab] }
        return true
    }

    /// What tab `index` shows of its face on its `UITab`: the title, the
    /// image (both states), the badge and enablement as its item has them,
    /// and its test id. A search tab with no symbol or label keeps UIKit's
    /// magnifying glass and "Search" (LLP 1141.002 D1).
    func writeBuiltTab(_ index: Int, item: UITabBarItem, title: String) {
        guard #available(iOS 18.0, tvOS 18.0, *), builtTabs.indices.contains(index), let tab = builtTabs[index] as? UITab else { return }
        let search = tab is UISearchTab
        if !(search && title.isEmpty), tab.title != title { tab.title = title }
        if !(search && item.image == nil) { tab.image = item.image }
        if #available(iOS 26.1, tvOS 26.1, *), !(search && item.image == nil) { tab.selectedImage = item.selectedImage }
        if tab.badgeValue != item.badgeValue { tab.badgeValue = item.badgeValue }
        if #available(iOS 18.4, tvOS 18.4, *), tab.isEnabled != item.isEnabled { tab.isEnabled = item.isEnabled }
        tab.accessibilityIdentifier = item.accessibilityIdentifier
    }

    /// The search tab activates its field on selection when its root route's
    /// header field says `autofocus` (LLP 1141.002 D3), and so stands apart
    /// at the bar's trailing end (iOS 26, measured); unsaid, it is a tab
    /// among the others, as UIKit's default is.
    func followSearchTab() {
        #if os(iOS)
        guard #available(iOS 26.0, *), let index = builtSearch, let tab = builtTabs[index] as? UISearchTab,
              let root = tabStack(index)?.stackRoutes.first as? RouteController, let field = root.search?.field else { return }
        let activates = field.props["autofocus"].map { $0 != "false" } ?? false
        guard tab.automaticallyActivatesSearch != activates else { return }
        tab.automaticallyActivatesSearch = activates
        presenter.session?.log("navigation: the search tab \(activates ? "activates its field when selected" : "is a landing page")")
        #endif
    }

    /// Whether tab `index` has a place in the bar UIKit shows, rather than
    /// a row of More's list: UIKit fills a phone's bar with the first tabs
    /// and More; an iPad's floating bar and sidebar show them all.
    func builtTabInBar(_ index: Int) -> Bool {
        guard let bar = tabController?.tabBar, bar.window != nil, let count = bar.items?.count, count < builtTabs.count else { return true }
        return index < count - 1
    }
}

extension TabDelegateProxy {
    /// A tap on a `UITab`, before UIKit selects it: the app's delegate, then
    /// the router (`mayShow`), as `shouldSelect` asks for a controller.
    @available(iOS 18.0, tvOS 18.0, *)
    @objc func tabBarController(_ tabs: UITabBarController, shouldSelectTab tab: UITab) -> Bool {
        if app?.tabBarController?(tabs, shouldSelectTab: tab) == false { return false }
        guard let host, !host.builtTabs.isEmpty, tabs === host.tabController, let controller = host.controller(ofTab: tab) else { return true }
        return host.mayShow(tabs, controller)
    }

    @available(iOS 18.0, tvOS 18.0, *)
    @objc func tabBarController(_ tabs: UITabBarController, didSelectTab tab: UITab, previousTab: UITab?) {
        if let host, !host.builtTabs.isEmpty, tabs === host.tabController { host.followUIKit(tabs) }
        app?.tabBarController?(tabs, didSelectTab: tab, previousTab: previousTab)
    }

    #if os(iOS)
    /// The sidebar's Edit hid or moved tabs, as More's Edit moves them (`syncNextTurn`).
    @available(iOS 18.0, tvOS 18.0, *)
    @objc func tabBarController(_ tabs: UITabBarController, displayOrderDidChangeFor group: UITabGroup) {
        host?.syncNextTurn()
        app?.tabBarController?(tabs, displayOrderDidChangeFor: group)
    }

    @available(iOS 18.0, tvOS 18.0, *)
    @objc func tabBarController(_ tabs: UITabBarController, visibilityDidChangeFor changed: [UITab]) {
        host?.syncNextTurn()
        app?.tabBarController?(tabs, visibilityDidChangeFor: changed)
    }
    #endif
}
#endif
