#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Exact's tab container built from `UITab`s (TabsIOS.swift) over
/// `scripts/fixtures/search-tabs.contract`: the search tab a `UISearchTab`
/// (LLP 1141.002 D1–D3).
final class TabsIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []
    private var animationsWereEnabled = true

    override func setUp() {
        super.setUp()
        animationsWereEnabled = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
    }

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        UIView.setAnimationsEnabled(animationsWereEnabled)
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The fixture `key` names in a window the size of the scene's screen.
    private func fixture(_ key: String) throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment[key], "build.mjs --test --ios compiles \(key)")
        let session = ExactApp.shared.makeSession(label: key)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        let frame = window.windowScene?.screen.bounds ?? CGRect(x: 0, y: 0, width: 402, height: 874)
        window.frame = frame
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: frame.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the navigation is ready") {
            let selected = session.presenter.navigation.tabController?.selectedViewController ?? session.presenter.navigation.primaryNavigation
            return selected?.viewIfLoaded?.window != nil && !Agent(session: session).nativeInFlight()
        }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }
            ?? session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func topRoute(_ session: ExactSession) -> String? {
        let navigation = session.presenter.navigation
        let stack = navigation.tabController?.selectedViewController ?? navigation.primaryNavigation
        return (stack?.stackRoutes.last as? RouteController)?.node.props["testId"]
    }

    /// LLP 1141.002 D1, D2: the tab that says `rel="search"` is a
    /// `UISearchTab` with its authored face, the others `UITab`s named by
    /// their panels; a tap on it presses the authored tab, and the router's
    /// selection of another selects that one.
    func testTheSearchTabIsAUISearchTabWithItsFace() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("UITab is iOS 18") }
        let session = try fixture("EXACT_SEARCH_TABS_PLAN")
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        XCTAssertNil(tabs.viewControllers, "the container holds UITabs")
        XCTAssertEqual(tabs.tabs.count, 3)
        XCTAssertEqual(tabs.tabs.prefix(2).map(\.identifier), ["panel-home", "panel-saved"], "a tab is its panel")
        let search = try XCTUnwrap(tabs.tabs.last as? UISearchTab, "rel=\"search\" is the search tab")
        XCTAssertEqual(search.title, "Speakers", "its own label")
        XCTAssertNotNil(search.image, "its own symbol")
        XCTAssertEqual(search.accessibilityIdentifier, "tab-find", "its test id")
        XCTAssertEqual(tabs.tabs[0].title, "Home")
        XCTAssertEqual(tabs.tabs[0].accessibilityIdentifier, "tab-home")
        XCTAssertEqual(tabs.selectedTab?.identifier, "panel-home")
        // The panel is built after the first frame, so the field's `autofocus` is known before a person can tap.
        if #available(iOS 26.0, *) {
            until("the search tab activates its field when selected") { search.automaticallyActivatesSearch }
        }
        tapTab(tabs, 2)
        until("the router shows the search tab's root") { self.topRoute(session) == "route-find" }
        XCTAssertTrue(tabs.selectedTab === search)
        XCTAssertEqual(navigation.routerTab, 2)
        let root = try XCTUnwrap(navigation.tabStack(2)?.stackRoutes.first as? RouteController)
        XCTAssertNotNil(root.navigationItem.searchController, "the search tab's field is its root's header search")
        // The router selects another tab: UIKit's selection follows.
        tapTab(tabs, 0)
        until("Home again") { self.topRoute(session) == "route-home" }
        XCTAssertEqual(tabs.selectedTab?.identifier, "panel-home")
        XCTAssertEqual(navigation.routerTab, 0)
    }

    /// The router's handshake on the UITab path: a push in a tab, a visit to
    /// another tab that keeps it, and a tap on the tab above its root that
    /// pops it, which the router follows as a Back.
    func testATabKeepsItsStackAndAReselectPopsIt() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("UITab is iOS 18") }
        let session = try fixture("EXACT_SEARCH_TABS_PLAN")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        XCTAssertFalse(tabs.tabs.isEmpty)
        _ = Agent(session: session).tap(["id": Int(try node(session, "open-detail").id)])
        until("Detail pushed") { self.topRoute(session) == "route-detail" }
        tapTab(tabs, 1)
        until("Saved shown") { self.topRoute(session) == "route-saved" }
        tapTab(tabs, 0)
        until("Home's stack kept its Detail") { self.topRoute(session) == "route-detail" }
        tapTab(tabs, 0)
        until("a tap above the root pops to it") { self.topRoute(session) == "route-home" }
        XCTAssertEqual(session.presenter.navigation.selectedKey, (tabs.selectedViewController?.stackRoutes.first as? RouteController)?.key,
                       "the router followed the pop")
    }

    /// A tab bar with no search tab stays on `viewControllers`.
    func testATabBarWithNoSearchTabKeepsItsControllers() throws {
        let session = try fixture("EXACT_TAB_EDGE_PLAN")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        XCTAssertNotNil(tabs.viewControllers)
        XCTAssertTrue(session.presenter.navigation.builtTabs.isEmpty)
    }
}
#endif
