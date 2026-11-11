#if !os(tvOS)
import Foundation
import ObjectiveC
import XCTest
import UserNotifications
@testable import ExactKit

private final class StubRequest: UNNotificationRequest {
    static var triggerValue: UNNotificationTrigger?
    override var trigger: UNNotificationTrigger? { Self.triggerValue }
}

private final class StubNotification: UNNotification {
    static var requestValue: UNNotificationRequest!
    override var request: UNNotificationRequest { Self.requestValue }
    override var date: Date { Date(timeIntervalSince1970: 1_800_000_000) }
}

private final class StubResponse: UNNotificationResponse {
    static var notificationValue: UNNotification!
    override var notification: UNNotification { Self.notificationValue }
    override var actionIdentifier: String { UNNotificationDefaultActionIdentifier }
}

/// A remote notification's `userInfo` is its sender's payload, so a push
/// opens no route and raises no module event, whatever activation entry it
/// carries; a local notification's tap is unchanged.
final class NotificationOriginTests: XCTestCase {
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        NotificationPresenter.shared.app = nil
        NotificationPresenter.shared.activate = nil
        super.tearDown()
    }

    /// The system's response to a tap, as the delegate receives it; a push
    /// trigger has no public initializer.
    private func tap(_ info: [AnyHashable: Any], push: Bool) {
        let content = UNMutableNotificationContent()
        content.userInfo = info
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        StubRequest.triggerValue = push ? class_createInstance(UNPushNotificationTrigger.self, 0) as? UNPushNotificationTrigger : nil
        object_setClass(request, StubRequest.self)
        StubNotification.requestValue = request
        StubResponse.notificationValue = class_createInstance(StubNotification.self, 0) as? StubNotification
        let response = class_createInstance(StubResponse.self, 0) as! StubResponse
        let center = class_createInstance(UNUserNotificationCenter.self, 0) as! UNUserNotificationCenter
        let done = expectation(description: "completion")
        NotificationPresenter.shared.userNotificationCenter(center, didReceive: response) { done.fulfill() }
        wait(for: [done], timeout: 2)
    }

    /// Three forged entries for a live session: the module's activation
    /// entry, an older build's top-level shape from another process, and
    /// that shape with an empty `navigate` naming a module event.
    private func forgeries(_ session: ExactSession) -> [[AnyHashable: Any]] {
        [["exact": ["navigate": "/forged", "session": session.label, "owner": session.notificationOwnerID,
                    "process": ExactApp.shared.notificationProcessID]],
         ["navigate": "/forged", "session": session.label, "owner": "x", "process": "another"],
         ["navigate": "", "session": session.label, "owner": "x", "process": "another",
          "extension": "notifications", "event": "notificationclick", "args": "[\"t\",{}]"]]
    }

    private func session() -> (ExactSession, () -> Int) {
        let session = ExactApp.shared.makeSession(label: "origin-\(UUID().uuidString)")
        sessions.append(session)
        NotificationPresenter.shared.app = ExactApp.shared
        var activations = 0
        NotificationPresenter.shared.activate = { if $0 === session { activations += 1 } }
        return (session, { activations })
    }

    func testAPushOpensNoRouteAndRaisesNoEvent() {
        for index in 0..<3 {
            let (session, activations) = self.session()
            let forged = forgeries(session)[index]
            tap(forged, push: true)
            XCTAssertNil(session.launchLocation, "\(forged)")
            XCTAssertTrue(session.pendingLaunchLocations.isEmpty, "\(forged)")
            XCTAssertTrue(session.heldModuleEvents.isEmpty, "\(forged)")
            XCTAssertEqual(activations(), 0, "\(forged)")
        }
    }

    func testALocalNotificationsTapIsUnchanged() {
        var (session, activations) = self.session()
        tap(forgeries(session)[0], push: false)
        XCTAssertEqual(session.launchLocation, "/forged")
        XCTAssertEqual(activations(), 1)
        (session, activations) = self.session()
        tap(forgeries(session)[1], push: false)
        XCTAssertEqual(session.launchLocation, "/forged")
        (session, activations) = self.session()
        tap(forgeries(session)[2], push: false)
        XCTAssertNil(session.launchLocation)
        XCTAssertEqual(session.heldModuleEvents.count, 1)
        XCTAssertEqual(activations(), 1)
    }
}
#endif
