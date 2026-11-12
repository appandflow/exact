import Foundation
#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif
import XCTest
@testable import ExactKit

final class DateValueTests: XCTestCase {
    func testLocalDateTimesAcceptHTMLSeparatorsAndOptionalSeconds() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "control", "props": ["type": "datetime-local", "value": ""], "handlers": ["input", "change"]],
            ["op": "roots", "ids": [1]],
        ]))
        let node = try XCTUnwrap(p.views[1])
        #if os(macOS)
        let picker = try XCTUnwrap(p.controls.controls[1] as? NSDatePicker)
        #elseif os(iOS)
        let picker = try XCTUnwrap(p.controls.controls[1] as? UIDatePicker)
        #endif
        var heard: [String] = []
        p.onControlValue = { _, value, input, change in
            XCTAssertTrue(input && change)
            heard.append(value)
        }
        for text in ["2026-03-04T10:30", "2026-03-04T10:30:00", "2026-03-04 10:30", "2026-03-04T10:30:45.500"] {
            XCTAssertNotNil(DateValue.parse("datetime-local", text), text)
            #if os(macOS) || os(iOS)
            XCTAssertNil(p.controls.typeDate(picker, node, text)["error"], text)
            XCTAssertEqual(heard.last, text)
            #endif
        }
    }

    func testInitialValuesUseLocalFieldsAndTheSameRepresentationAsTheirBounds() throws {
        let instant = try XCTUnwrap(DateValue.parse("datetime-local", "2026-11-10T02:30"))
        let zone = try XCTUnwrap(TimeZone(secondsFromGMT: -8 * 60 * 60))
        for (kind, local, later, earlier) in [
            ("date", "2026-11-09", "2026-11-12", "2026-11-01"),
            ("time", "18:30", "19:00", "09:00"),
            ("datetime-local", "2026-11-09T18:30", "2026-11-12T19:00", "2026-11-01T09:00"),
        ] {
            let expected = try XCTUnwrap(DateValue.parse(kind, local))
            let minimum = try XCTUnwrap(DateValue.parse(kind, later))
            let maximum = try XCTUnwrap(DateValue.parse(kind, earlier))
            XCTAssertEqual(DateValue.initial(kind, now: instant, timeZone: zone), expected)
            XCTAssertEqual(DateValue.initial(kind, now: instant, timeZone: zone, minimum: minimum), minimum)
            XCTAssertEqual(DateValue.initial(kind, now: instant, timeZone: zone, maximum: maximum), maximum)
        }
    }
}
