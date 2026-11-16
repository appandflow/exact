#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class CollectionFillIOSTests: XCTestCase {
    private func snapshot(_ revision: Int, measured: Bool) -> [String: Any] {
        let rows: [[String: Any]] = (0..<4).map {
            ["view": 2 + $0, "root": 2 + $0, "epoch": 1, "index": $0, "size": 72, "measured": measured]
        }
        return ["op": "collections", "items": [["view": 1, "revision": revision, "scrollSequence": 0,
            "count": 100, "totalExtent": 7200, "pending": false, "rows": rows, "correction": NSNull()]]]
    }

    /// Four 72-point rows from index `0` (or the indices given), each at its
    /// place in a 100-row list under 20 points of padding.
    private func fixture(_ indices: [Int] = Array(0..<4)) throws -> (Presenter, UIWindow, UIScrollView) {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 200))
        window.addSubview(p.viewport)
        p.viewport.frame = window.bounds
        window.makeKeyAndVisible()
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll", "padding_top": 20]],
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0,
                "count": 100, "totalExtent": 7200, "pending": false,
                "rows": indices.enumerated().map { ["view": 2 + $0.offset, "root": 2 + $0.offset, "epoch": 1, "index": $0.element] },
                "correction": NSNull()]]],
        ]
        for (i, index) in indices.enumerated() {
            ops.append(["op": "create", "id": 2 + i, "kind": "view"])
            ops.append(["op": "frame", "id": 2 + i, "x": 10, "y": 20 + 72 * index, "w": 280, "h": 72])
        }
        ops += [["op": "children", "id": 1, "ids": indices.indices.map { 2 + $0 }], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
                ["op": "content", "id": 1, "w": 300, "h": 7220]]
        p.apply(wireBatch(ops))
        window.layoutIfNeeded()
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        return (p, window, scroll)
    }

    /// Advance offsets and slices at 60 Hz, draining owner work between frames.
    /// The bounds include the final fill and retirement after the reversal.
    func testScriptedFlingCountsFillsFlushesAndSlices() throws {
        for asynchronous in [false, true] {
            let previous = ExactSession.asyncFills
            ExactSession.asyncFills = asynchronous
            defer { ExactSession.asyncFills = previous }
            let session = ExactApp.shared.makeSession(label: "counted-fling")
            let p = session.presenter
            let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
            defer { session.destroy(); window.isHidden = true }
            window.addSubview(p.viewport)
            p.viewport.frame = window.bounds
            window.makeKeyAndVisible()
            XCTAssertNil(session.boot(plan: try fixturePlan("SCROLL_SNAP"), size: window.bounds.size).error)
            let node = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "strip" })
            let scroll = try XCTUnwrap(node.scrollView)
            p.scrollPump.settle()
            waitForMainQueue("initial list continuations")
            var time = CACurrentMediaTime()
            p.scrollPump.clock = { time }
            let fills = p.scrollPump.fillPasses, flushes = p.collections.flushes, slices = p.collections.slices
            for frame in 0..<240 {
                time += 1.0 / 60
                let step = frame < 120 ? frame + 1 : 239 - frame
                scroll.contentOffset.x = CGFloat(step * 12)
                p.scrollPump.pump()
                session.drainFill()
                FrameClock.shared.drop(p.scrollPump)
                waitForMainQueue("frame's queued pump and collection continuation")
                XCTAssertTrue(p.collections.covers(node.id), "frame \(frame) has every row it shows")
            }
            for _ in 0..<20 {
                time += 1.0 / 60
                p.scrollPump.pump()
                session.drainFill()
                FrameClock.shared.drop(p.scrollPump)
                waitForMainQueue("final fill and retirement")
                if p.collections.fillPending.isEmpty { break }
            }
            XCTAssertTrue(p.collections.fillPending.isEmpty)
            XCTAssertEqual(scroll.contentOffset.x, 0, accuracy: 0.001)
            let counts = [p.scrollPump.fillPasses - fills, p.collections.flushes - flushes, p.collections.slices - slices]
            // What the host made before frames skipped text discovery, with a
            // margin of two: unrelated changes to slicing move these by one.
            let baseline = asynchronous ? [248, 305, 124] : [240, 280, 120]
            for (count, bound) in zip(counts, baseline) { XCTAssertLessThanOrEqual(count, bound + 2) }
            print("J fling async=\(asynchronous) frames=240 fills=\(counts[0]) flushes=\(counts[1]) slices=\(counts[2])")
        }
    }

    func testOffsetReportsReuseRowsAndAChangedHeightIsMeasuredOnce() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        p.collections.settle()
        let records = try XCTUnwrap(p.collections.entries[1]?.records)
        let measured = records.measuredRows, spans = records.spanReads
        let reportStart = reports.count
        for offset in 1...100 {
            scroll.contentOffset.y = CGFloat(offset % 60)
            p.collections.fillSlice(1, limit: 5)
            XCTAssertTrue(p.collections.covers(1))
            _ = p.collections.rowsToCover(1, ahead: 100)
        }
        let offsetMeasurements = records.measuredRows - measured
        let offsetSpans = records.spanReads - spans
        let offsetReports = reports.count - reportStart
        XCTAssertEqual(offsetMeasurements, 0, "offset-only reports measure no rows")
        XCTAssertEqual(offsetSpans, 0, "coverage reuses the mounted spans")
        XCTAssertEqual(reports.count - reportStart, 100, "every offset still sends its report")
        let before = reports.count
        p.apply(wireBatch([
            ["op": "props", "id": 2, "set": ["text": "a taller row"]],
            ["op": "frame", "id": 2, "x": 10, "y": 20, "w": 280, "h": 90],
        ]))
        p.collections.settle()
        XCTAssertEqual(records.measuredRows, measured + 1, "content's changed border box is measured once")
        XCTAssertGreaterThan(reports.count, before)
        XCTAssertEqual(records.rows[2]?.measurement?.size, 90)
        XCTAssertEqual(records.rows[3]?.measurement?.size, 72)
        print("M offset_reports=\(offsetReports) row_measurements=\(offsetMeasurements) span_reads=\(offsetSpans) changed_height_measurements=\(records.measuredRows - measured - offsetMeasurements)")
    }

    func testAReadBeforeTheBatchAppliesItsFramesCannotCacheTheOldHeight() throws {
        let (p, window, _) = try fixture()
        defer { p.reset(); window.isHidden = true }
        p.collections.onFeedback = { _ in }
        p.collections.settle()
        let records = try XCTUnwrap(p.collections.entries[1]?.records)
        let before = records.measuredRows
        let batch = wireBatch([["op": "frame", "id": 2, "x": 10, "y": 20, "w": 280, "h": 90]])
        p.collections.beginBatch(batch)
        _ = p.collections.geometry(1)
        XCTAssertEqual(records.rows[2]?.measurement?.size, 72)
        p.applyGeometry(batch.ops[0])
        p.collections.endBatch()
        p.collections.settle()
        XCTAssertEqual(records.rows[2]?.measurement?.size, 90)
        XCTAssertEqual(p.collections.entries[1]?.lastFacts?.measurements.first?.size, 90)
        XCTAssertEqual(records.measuredRows, before + 2, "only the changed row is read, before and after its frame")
    }

    func testAReadBeforePaddingAppliesCannotSettleTheOuterBatchPort() throws {
        for nested in [false, true] {
            let (p, window, scroll) = try fixture()
            defer { p.reset(); window.isHidden = true }
            p.collections.onFeedback = { _ in }
            p.collections.settle()
            let records = try XCTUnwrap(p.collections.entries[1]?.records)
            let measured = records.measuredRows, spans = records.spanReads
            let batch = wireBatch([contentOp(1, 300, 200, padding: [20, 0, 40, 0])])
            p.collections.beginBatch(batch)
            if nested {
                p.collections.beginBatch(wireBatch([]))
                p.collections.endBatch()
            } else {
                XCTAssertEqual(p.collections.geometry(1)?.paddingEnd, 0)
            }
            p.applyGeometry(batch.ops[0])
            p.collections.endBatch()
            p.collections.settle()
            XCTAssertEqual(p.collections.geometry(1)?.paddingEnd, 40, "nested=\(nested)")
            XCTAssertEqual(p.collections.entries[1]?.lastFacts?.paddingEnd, 40, "the report uses the applied padding")
            for offset in 1...100 {
                scroll.contentOffset.y = CGFloat(offset % 60)
                XCTAssertEqual(p.collections.geometry(1)?.paddingEnd, 40)
            }
            XCTAssertEqual(records.measuredRows, measured, "padding and offsets do not remeasure rows")
            XCTAssertEqual(records.spanReads, spans, "padding and offsets keep the row spans")
        }
    }

    func testAcknowledgedMeasurementsStayCachedAndEpochChangesInvalidateThem() throws {
        let (p, window, _) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        p.collections.settle()
        let records = try XCTUnwrap(p.collections.entries[1]?.records)
        let measured = records.measuredRows
        p.apply(wireBatch([snapshot(2, measured: true)]))
        p.collections.settle()
        p.collections.userIntent(1, travel: true)
        p.collections.settle()
        let acknowledged = try XCTUnwrap(reports.last)
        XCTAssertEqual(acknowledged.count, 100, "acknowledged rows leave the report")
        XCTAssertEqual(records.measuredRows, measured, "acknowledgement does not remeasure")
        var next = snapshot(3, measured: false)
        var items = try XCTUnwrap(next["items"] as? [[String: Any]])
        var rows = try XCTUnwrap(items[0]["rows"] as? [[String: Any]])
        rows[0]["epoch"] = 2
        items[0]["rows"] = rows; next["items"] = items
        p.apply(wireBatch([next]))
        p.collections.settle()
        XCTAssertEqual(records.measuredRows, measured + 1)
        XCTAssertEqual(records.rows[2]?.measurement?.epoch, 2)
        XCTAssertEqual(records.unacknowledged.count, 4, "unacknowledged rows are retried")
        rows.append(["view": 99, "root": 99, "epoch": 1, "index": 4])
        items[0]["rows"] = rows; items[0]["revision"] = 4; next["items"] = items
        p.apply(wireBatch([next]))
        p.collections.settle()
        XCTAssertNil(records.rows[99]?.measurement)
        p.apply(wireBatch([
            ["op": "create", "id": 99, "kind": "view"],
            ["op": "frame", "id": 99, "x": 10, "y": 308, "w": 280, "h": 72],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 99]],
        ]))
        p.collections.settle()
        XCTAssertEqual(records.measuredRows, measured + 2, "a wrapper mounted later is measured")
        XCTAssertEqual(records.rows[99]?.measurement?.size, 72)
    }

    func testCachedCoverageMatchesTheSortedWalkAcrossGapsOverlapsAndReordering() throws {
        let (p, window, scroll) = try fixture([0, 1, 2, 3, 99])
        defer { p.reset(); window.isHidden = true }
        p.collections.onFeedback = { _ in }
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 300]]))
        let phases = [
            [(20, 72), (92, 72), (164, 72), (236, 72), (7148, 72)],
            [(20, 72), (92, 72), (80, 180), (236, 72), (7148, 72)],
            [(1200, 100), (1000, 100), (1100, 100), (1300, 100), (7148, 72)],
            [(1200, 100), (1000, 100), (1100, 100), (1300, 100), (900, 100)],
        ]
        let records = try XCTUnwrap(p.collections.entries[1]?.records)
        var offsets = 0
        for (phase, frames) in phases.enumerated() {
            let indices = phase == 2 ? [10, 11, 12, 13, 99] : [0, 1, 2, 3, 99]
            var ops: [[String: Any]] = [["op": "collections", "items": [[
                "view": 1, "revision": phase + 2, "scrollSequence": 0, "count": 100, "totalExtent": 7200,
                "rows": indices.enumerated().map { ["view": 2 + $0.offset, "root": 2 + $0.offset, "epoch": 1, "index": $0.element] },
            ]]]]
            ops += frames.enumerated().map { i, frame in
                ["op": "frame", "id": 2 + i, "x": 10, "y": frame.0, "w": 280, "h": frame.1]
            }
            p.apply(wireBatch(ops))
            let snapshot = try XCTUnwrap(p.collections.entries[1]?.snapshot)
            let spans = try snapshot.rows.map { row -> (Int, CGRect) in
                let view = try XCTUnwrap(p.views[row.view])
                return (row.index, view.convert(view.bounds, to: scroll))
            }
            let sorted = spans.sorted { $0.1.minY < $1.1.minY }
            _ = p.collections.covers(1)
            let measured = records.measuredRows, reads = records.spanReads
            for offset in [1000] + Array(stride(from: 0, through: 7200, by: 37)) {
                scroll.contentOffset.y = CGFloat(offset)
                let port = scroll.bounds
                let head = try XCTUnwrap(sorted.first), tail = try XCTUnwrap(sorted.last)
                var reached = head.0 == 0 ? max(port.minY, head.1.minY) : port.minY
                let end = tail.0 == snapshot.count - 1 ? min(port.maxY, tail.1.maxY) : port.maxY
                var covered = reached >= end
                if !covered {
                    for (_, frame) in sorted {
                        if frame.maxY <= reached { continue }
                        if frame.minY > reached + 0.5 { break }
                        reached = frame.maxY
                        if reached >= end { covered = true; break }
                    }
                }
                XCTAssertEqual(p.collections.covers(1), covered, "offset \(offset), phase \(phase)")
                if phase == 2, offset == 1000 {
                    XCTAssertTrue(covered, "reordered contiguous rows cover the whole port")
                }
                let mean = spans.reduce(CGFloat(0)) { $0 + $1.1.height } / CGFloat(spans.count)
                for ahead in [CGFloat(-200), CGFloat(200)] {
                    var edge = ahead > 0 ? port.minY : port.maxY
                    var last: Int?
                    for (index, frame) in ahead > 0 ? spans : Array(spans.reversed()) {
                        if ahead > 0, frame.minY <= edge + 0.5, frame.maxY >= edge { edge = frame.maxY; last = index }
                        if ahead < 0, frame.maxY >= edge - 0.5, frame.minY <= edge { edge = frame.minY; last = index }
                    }
                    let shortfall = ahead > 0 ? port.maxY + ahead - edge : edge - (port.minY + ahead)
                    let atEnd = ahead > 0 ? last == 99 : last == 0
                    let expected: UInt32 = atEnd || shortfall <= 0 ? 0 : UInt32(min(64, (shortfall / mean).rounded(.up)))
                    XCTAssertEqual(p.collections.rowsToCover(1, ahead: ahead), expected)
                }
                offsets += 1
            }
            XCTAssertEqual(records.measuredRows, measured, "coverage never remeasures rows for an offset")
            XCTAssertEqual(records.spanReads, reads, "coverage never reads row views for an offset")
        }
        print("M sorted_coverage_offsets=\(offsets) row_measurements=0 span_reads=0")
    }

    func testAReversedFillRetiresFromFreshGeometryAndCurrentPins() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = [], fills: [Data] = []
        var inFlight = false
        p.collections.onFeedback = { reports.append($0) }
        p.collections.settle()
        p.collections.onFill = { _, bytes in fills.append(bytes); inFlight = true }
        p.collections.filling = { inFlight }
        p.collections.fillSlice(1, limit: 3)
        XCTAssertEqual(fills.count, 1)
        let before = reports.count
        scroll.contentOffset.y = 70
        scroll.contentOffset.y = 30
        p.collections.pointer(3)
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 10, "y": 20, "w": 280, "h": 90]]))
        XCTAssertEqual(reports.count, before, "all reports wait for the fill")
        let offset = try XCTUnwrap(p.collections.geometry(1)).offset
        inFlight = false
        p.collections.landed(1, at: ProcessInfo.processInfo.systemUptime)
        p.collections.fillSlice(1, limit: 3)
        let report = try XCTUnwrap(reports.last)
        XCTAssertGreaterThan(reports.count, before)
        let word = { (at: Int) in report.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: at, as: UInt32.self) } }
        XCTAssertEqual(word(92) & 6, 4, "retirement stays synchronous and cannot build")
        XCTAssertEqual(word(76), 3, "the row under the current contact stays pinned")
        let sentOffset = report.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 24, as: Double.self) }
        XCTAssertEqual(sentOffset, offset)
        XCTAssertEqual(p.collections.entries[1]?.lastFacts?.measurements.first?.size, 90)
    }

    func testCachedFixedCoverageKeepsGapsAndRefreshesWhenFixedSizesChange() throws {
        let indices = [0, 1, 2, 3, 99]
        let (p, window, scroll) = try fixture(indices)
        defer { p.reset(); window.isHidden = true }
        p.apply(wireBatch([contentOp(1, 300, 200, padding: [20, 0, 0, 0])]))
        let records = try XCTUnwrap(p.collections.entries[1]?.records)
        let phases = [(180, 240, indices), (80, 40, indices), (600, 800, []), (900, 500, [])]
        for (revision, phase) in phases.enumerated() {
            let (lead, trail, mounted) = phase
            let extent = lead + 7200 + trail
            let rows = mounted.enumerated().map { offset, index in
                ["view": 2 + offset, "root": 2 + offset, "epoch": 1, "index": index]
            }
            var ops: [[String: Any]] = [["op": "collections", "items": [[
                "view": 1, "revision": revision + 2, "scrollSequence": 0, "count": 100,
                "totalExtent": extent, "rows": rows, "fixed": [
                    ["view": 10, "roots": [], "epoch": 1, "size": lead],
                    ["view": 11, "roots": [], "epoch": 1, "size": trail],
                ]]]]]
            for (offset, index) in mounted.enumerated() {
                ops.append(["op": "frame", "id": 2 + offset, "x": 10, "y": 20 + lead + 72 * index, "w": 280, "h": 72])
            }
            ops.append(["op": "content", "id": 1, "w": 300, "h": 20 + extent])
            p.apply(wireBatch(ops))
            _ = p.collections.covers(1)
            let measured = records.measuredRows, reads = records.spanReads
            var spans = mounted.map { index in CGFloat(20 + lead + 72 * index)...CGFloat(20 + lead + 72 * (index + 1)) }
            spans += [CGFloat(20)...CGFloat(20 + lead), CGFloat(20 + extent - trail)...CGFloat(20 + extent)]
            spans.sort { $0.lowerBound < $1.lowerBound }
            for offset in stride(from: 0, through: 20 + extent, by: 37) {
                scroll.contentOffset.y = CGFloat(offset)
                var reached = max(scroll.bounds.minY, 20)
                let end = min(scroll.bounds.maxY, CGFloat(20 + extent))
                var covered = reached >= end
                if !covered {
                    for span in spans {
                        if span.upperBound <= reached { continue }
                        if span.lowerBound > reached + 0.5 { break }
                        reached = span.upperBound
                        if reached >= end { covered = true; break }
                    }
                }
                XCTAssertEqual(p.collections.covers(1), covered, "phase \(revision), offset \(offset)")
            }
            XCTAssertEqual(records.measuredRows, measured, "fixed coverage never remeasures rows for an offset")
            XCTAssertEqual(records.spanReads, reads, "fixed coverage never reads row views for an offset")
        }
        scroll.contentOffset.y = 750
        XCTAssertFalse(p.collections.covers(1))
        let padding = wireBatch([contentOp(1, 300, 200, padding: [80, 0, 0, 0])])
        p.collections.beginBatch(padding)
        XCTAssertFalse(p.collections.covers(1), "a read during application still sees the old padding")
        p.apply(padding)
        p.collections.endBatch()
        XCTAssertTrue(p.collections.covers(1), "padding moves fixed spans even with no mounted data rows")
    }

    // Baseline coverage: callback rationing predates this lane.
    func testCoveredScrollDefersItsReportAndAGapRescuesOnlyWhatShows() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        let start = CACurrentMediaTime()
        for i in 1...1000 { scroll.contentOffset.y = CGFloat(1 + i % 90) }
        let milliseconds = (CACurrentMediaTime() - start) * 1000
        XCTAssertTrue(p.collections.covers(1))
        XCTAssertTrue(reports.isEmpty, "covered UIKit callbacks return before a row report")
        XCTAssertTrue(p.collections.fillPending.contains(1))
        p.collections.fillSlice(1, limit: 5)
        func limit(_ data: Data) -> UInt32 { data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 88, as: UInt32.self) } }
        XCTAssertEqual(reports.count, 1)
        XCTAssertTrue(p.collections.observingTurns)
        p.scrollPump.reset()
        XCTAssertTrue(p.scrollPump.observingTurns, "the next native callback reads this turn's elapsed time")
        XCTAssertTrue(p.collections.observingTurns, "later native input needs a fresh report budget before continuations")
        XCTAssertEqual(limit(try XCTUnwrap(reports.last)), 5)
        scroll.contentOffset.y = 200
        XCTAssertFalse(p.collections.covers(1))
        XCTAssertEqual(reports.count, 2, "a visible gap is rescued in its UIKit callback")
        p.collections.reset()
        XCTAssertTrue(p.collections.observingTurns)
        XCTAssertEqual(limit(try XCTUnwrap(reports.last)), 2, "visible rows plus two, rather than a whole window")
        print("D3 scroll callbacks=1000 covered_reports=0 ms=\(milliseconds) slice_limit=5 rescue_limit=2")
    }
    func testCoveredScrollPublishesItsDeferredReportThroughTheScheduledPump() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        p.collections.onFeedback = { _ in }
        p.collections.flush()
        let ready = expectation(description: "initial collection reports drained")
        DispatchQueue.main.async { ready.fulfill() }
        wait(for: [ready], timeout: 5)
        let reported = expectation(description: "covered scroll's pump report")
        var inScrollCallback = true
        var reports: [Data] = []
        p.collections.onFeedback = {
            XCTAssertFalse(inScrollCallback, "row reports run after UIKit's offset callback")
            reports.append($0)
            if reports.count == 1 { reported.fulfill() }
        }
        let started = CACurrentMediaTime()
        scroll.contentOffset.y = 40
        XCTAssertTrue(p.collections.covers(1))
        XCTAssertTrue(p.collections.fillPending.contains(1))
        XCTAssertTrue(reports.isEmpty)
        inScrollCallback = false
        wait(for: [reported], timeout: 5)
        XCTAssertEqual(reports.count, 1)
        XCTAssertFalse(p.collections.fillPending.contains(1), "the scheduled pump services the queued fill")
        let limit = try XCTUnwrap(reports.first).withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 88, as: UInt32.self) }
        XCTAssertGreaterThan(limit, 0)
        XCTAssertLessThan(limit, UInt32.max - 1, "the pump rations its slice")
        print("D3 scheduled scroll reports=\(reports.count) limit=\(limit) elapsed_ms=\((CACurrentMediaTime() - started) * 1000)")
    }

    /// Steps of a port or more each frame (a scroll indicator dragged)
    /// show rows no frame before showed: past the first, a jump's, each
    /// rescue is a glimpse, its rows bounded.
    func testCrossingAPortEachFrameRescuesAGlimpse() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        func word(_ data: Data, _ at: Int) -> UInt32 { data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: at, as: UInt32.self) } }
        scroll.contentOffset.y = 10
        reports.removeAll()
        scroll.contentOffset.y = 900
        XCTAssertEqual(word(try XCTUnwrap(reports.last), 92) & 16, 0, "a lone jump builds what shows")
        scroll.contentOffset.y = 1800
        let glimpse = try XCTUnwrap(reports.last)
        XCTAssertEqual(word(glimpse, 92) & 16, 16, "the next step across a port is a glimpse")
        XCTAssertEqual(word(glimpse, 88), 2, "two rows before a glimpse's cost is measured")
        XCTAssertTrue(p.collections.glimpses.contains(1))
    }

    func testScrollReportsOnlyNewRowMeasurements() throws {
        let (p, window, scroll) = try fixture()
        defer { p.reset(); window.isHidden = true }
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        func count(_ data: Data) -> UInt32 {
            data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 96, as: UInt32.self) }
        }
        p.collections.flush()
        XCTAssertEqual(count(try XCTUnwrap(reports.last)), 4)
        p.apply(wireBatch([snapshot(2, measured: true)]))
        p.collections.settle()
        reports.removeAll()
        let started = CACurrentMediaTime()
        for y in [30, 40, 50] {
            scroll.contentOffset.y = CGFloat(y)
            p.collections.fillSlice(1, limit: 0)
        }
        let sizes = reports.map(count)
        print("QUEUE 675 scroll reports=\(sizes.count) measurements=\(sizes) bytes=\(reports.reduce(0) { $0 + $1.count }) ms=\((CACurrentMediaTime() - started) * 1000)")
        XCTAssertEqual(sizes, [0, 0, 0])
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 10, "y": 20, "w": 280, "h": 73]]))
        p.collections.settle()
        XCTAssertEqual(count(try XCTUnwrap(reports.last)), 1, "a changed height is sent")
        p.apply(wireBatch([snapshot(3, measured: false)]))
        p.collections.settle()
        // Invalidation withdraws the acknowledgement, even if a row's
        // frame and epoch have not changed.
        p.collections.fillSlice(1, limit: 1)
        XCTAssertEqual(count(try XCTUnwrap(reports.last)), 4)
    }

    func testARejectedPinDoesNotAcknowledgeItsRowMeasurements() throws {
        let (p, window, _) = try fixture()
        defer { p.reset(); window.isHidden = true }
        p.apply(wireBatch([snapshot(1, measured: true)]))
        var storedHeight = 72.0, rejected = 0
        p.collections.onFeedback = { data in
            data.withUnsafeBytes { bytes in
                let pin = UInt32(littleEndian: bytes.loadUnaligned(fromByteOffset: 76, as: UInt32.self))
                // The runner rejects the whole report for an unpublished pin.
                if pin == 99 { rejected += 1; return }
                let count = UInt32(littleEndian: bytes.loadUnaligned(fromByteOffset: 96, as: UInt32.self))
                for i in 0..<Int(count) {
                    let at = 100 + i * 20
                    let id = UInt32(littleEndian: bytes.loadUnaligned(fromByteOffset: at, as: UInt32.self))
                    if id == 2 {
                        storedHeight = Double(bitPattern: UInt64(littleEndian: bytes.loadUnaligned(fromByteOffset: at + 12, as: UInt64.self)))
                    }
                }
            }
        }
        p.collections.flush()
        // Model a host view the runner does not own, inside a mounted row.
        p.apply(wireBatch([
            ["op": "create", "id": 99, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 2, "ids": [99]],
            ["op": "frame", "id": 2, "x": 10, "y": 20, "w": 280, "h": 73],
        ]))
        p.collections.pointer(99)
        p.collections.settle()
        XCTAssertEqual(rejected, 1)
        XCTAssertEqual(storedHeight, 72)
        p.collections.pointer(nil)
        p.collections.settle()
        XCTAssertEqual(storedHeight, 73, "the valid report retries the rejected height at the same revision")
        XCTAssertEqual(p.collections.entries[1]?.snapshot.revision, 1)
    }


    // @ref LLP 1010 §6.11 — a kept end row past a gap of unbuilt rows does
    // not end the travel: the rows the port will uncover are still owed.
    func testAKeptEndRowPastAGapStillLeadsTheSlice() throws {
        do {
            let (p, window, scroll) = try fixture([0, 1, 2, 3, 99])
            defer { p.reset(); window.isHidden = true }
            // The port reaches 260 of the run's 308; 200 points more need
            // ceil(152 / 72) = 3 rows, the kept last row notwithstanding.
            scroll.contentOffset.y = 60
            XCTAssertEqual(p.collections.rowsToCover(1, ahead: 200), 3)
            XCTAssertEqual(p.collections.rowsToCover(1, ahead: -200), 0, "the first item is built and joined")
        }
        do {
            let (p, window, scroll) = try fixture([0, 50, 51, 52, 53])
            defer { p.reset(); window.isHidden = true }
            // Rows 50–53 span 3620–3908; from 3680 back 200 points needs
            // ceil(140 / 72) = 2 rows, the kept first row notwithstanding.
            scroll.contentOffset.y = 3680
            XCTAssertEqual(p.collections.rowsToCover(1, ahead: -200), 2)
        }
    }
}
#endif
