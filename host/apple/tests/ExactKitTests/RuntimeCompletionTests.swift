import Foundation
import XCTest
@testable import ExactKit

final class RuntimeCompletionTests: XCTestCase {
    private typealias Receive = (RuntimeCompletion) -> Void
    private typealias Submit = (Runtime, @escaping Receive) -> Void

    private var paths: [(String, Submit)] {
        let timer = NativeTimerWake(index: 0, generation: 1, due: 10, interval: 10, tolerance: 0, once: false)
        return [
            ("replies", { $0.pumpAsync(now: 10, done: $1) }),
            ("fill", { $0.collectionFeedbackAsync(Data(), now: 10, done: $1) }),
            ("canvas", { $0.canvasDrawAsync(done: $1) }),
            ("notice", { $0.later([{ $0.pump(now: 10) }], done: $1) }),
            ("tick", { $0.tickAsync(now: 10, done: $1) }),
            ("timer admission", { $0.timerWaitsAsync(kinds: 1, cutoff: 10, done: $1) }),
            ("native timer", { $0.fireNativeTimerAsync(timer, now: 10, done: $1) }),
            ("advance", { $0.advanceAsync(now: 10, done: $1) }),
        ]
    }

    func testEveryPostedPathRefusesARetiredGenerationBeforeEnteringRust() {
        refuses { $0.completions.replace() }
    }

    func testEveryPostedPathRefusesADestroyedSessionBeforeEnteringRust() {
        refuses { $0.destroy() }
    }

    private func refuses(_ retire: (Runtime) -> Void) {
        let runtime = Runtime()
        defer { runtime.destroy() }
        var reads = 0
        runtime.observeBatch = { _, _ in reads += 1 }
        var replies: [(String, RuntimeCompletion)] = []
        // Retirement runs before these jobs without a clock, sleep or race.
        Owner.shared.sync {
            for (name, submit) in paths { submit(runtime) { replies.append((name, $0)) } }
            retire(runtime)
        }
        Owner.shared.sync {}
        XCTAssertEqual(replies.count, paths.count, "every refused job releases its caller")
        XCTAssertEqual(reads, 0, "a stale job must not call the C ABI, even if its batch would be dropped")
        for (name, reply) in replies {
            XCTAssertTrue(reply.batch.held, name)
            XCTAssertFalse(runtime.accepts(reply.lease), name)
        }
    }

    func testEveryPostedPathKeepsItsLeaseThroughPublication() {
        let runtime = Runtime(), other = Runtime()
        defer { runtime.destroy(); other.destroy() }
        var reads = 0
        runtime.observeBatch = { _, _ in reads += 1 }
        var replies: [(String, RuntimeCompletion)] = []
        for (name, submit) in paths { submit(runtime) { replies.append((name, $0)) } }
        Owner.shared.sync {}
        XCTAssertEqual(replies.count, paths.count)
        XCTAssertEqual(reads, paths.count, "live jobs reach the real runtime")
        for (name, reply) in replies {
            XCTAssertFalse(reply.batch.held, name)
            XCTAssertTrue(runtime.accepts(reply.lease), name)
            XCTAssertFalse(other.accepts(reply.lease), "another session at the same generation: \(name)")
        }
        Owner.shared.sync { runtime.completions.replace() }
        for (name, reply) in replies { XCTAssertFalse(runtime.accepts(reply.lease), "retired before apply: \(name)") }
    }

    func testADeferredSequenceChecksTheLeaseBeforeEachCall() {
        let runtime = Runtime()
        defer { runtime.destroy() }
        var calls = 0, replies: [RuntimeCompletion] = []
        runtime.later([
            { r in calls += 1; r.completions.replace(); return Runtime.busy },
            { _ in calls += 1; return Runtime.busy },
        ]) { replies.append($0) }
        Owner.shared.sync {}
        XCTAssertEqual(calls, 1)
        XCTAssertEqual(replies.count, 2)
        XCTAssertTrue(replies[1].batch.held)
    }

    func testAdmissionCannotMoveAJobToANewerGeneration() {
        let runtime = Runtime()
        defer { runtime.destroy() }
        var reads = 0, replies: [RuntimeCompletion] = []
        runtime.observeBatch = { _, _ in reads += 1 }
        runtime.beforeEvent = { [weak runtime] in runtime?.completions.replace() }
        runtime.pumpAsync(now: 10) { replies.append($0) }
        Owner.shared.sync {}
        XCTAssertEqual(reads, 0)
        XCTAssertEqual(replies.count, 1)
        XCTAssertFalse(runtime.accepts(replies[0].lease))
    }

    func testReplacementRetiresOnlyAfterTheNativeCommitSucceeds() throws {
        let runtime = Runtime()
        defer { runtime.destroy() }
        let before = runtime.completionLease
        XCTAssertNotNil(runtime.bootPlan(Data(), width: 400, height: 600).error)
        XCTAssertTrue(runtime.accepts(before), "a refused boot keeps the running generation")
        let plan = try fixturePlan("ASYNC_ADVANCE")
        XCTAssertNil(runtime.bootPlan(plan, width: 400, height: 600).error)
        XCTAssertFalse(runtime.accepts(before))
        let booted = runtime.completionLease
        XCTAssertNil(runtime.preparePlan(plan, width: 400, height: 600).error)
        XCTAssertTrue(runtime.accepts(booted), "preparing a candidate does not retire its predecessor")
        XCTAssertNil(runtime.commitPlan().error)
        XCTAssertFalse(runtime.accepts(booted))
        let committed = runtime.completionLease
        XCTAssertNotNil(runtime.commitPlan().error, "there is no second prepared candidate")
        XCTAssertTrue(runtime.accepts(committed))
    }

    func testACompletionCannotApplyToAnotherSession() {
        let a = ExactApp.shared.makeSession(label: "door-a"), b = ExactApp.shared.makeSession(label: "door-b")
        defer { a.destroy(); b.destroy() }
        let before = b.appliedBatches
        b.noticesInFlight += 1
        b.publish(.notice, RuntimeCompletion(lease: a.runtime.completionLease,
            batch: Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 123)))
        b.drainFill()
        XCTAssertEqual(b.appliedBatches, before)
        XCTAssertEqual(b.noticesInFlight, 0)
        XCTAssertNil(b.timerDue)
    }

    func testAStalePreparedCandidateKeepsTheLiveRunnersLeases() throws {
        let session = ExactApp.shared.makeSession(label: "door-stale-candidate")
        defer { session.destroy() }
        let runtime = session.runtime, plan = try fixturePlan("ASYNC_ADVANCE")
        XCTAssertNil(runtime.bootPlan(plan, width: 400, height: 600).error)
        let live = runtime.completionLease, platform = SessionLease(session)
        XCTAssertNil(runtime.preparePlan(plan, width: 400, height: 600).error)
        XCTAssertNil(runtime.advance(now: 1000).error)
        let state = runtime.agent(#"{"op":"state"}"#)

        let refused = runtime.commitPlan()
        XCTAssertNil(refused.error)
        XCTAssertTrue(refused.pending, "the live clock changed after preparation")
        XCTAssertEqual(runtime.agent(#"{"op":"state"}"#), state, "the live runner stays current")
        XCTAssertTrue(runtime.accepts(live), "outstanding completions still belong to the live runner")
        XCTAssertTrue(platform.session === session, "platform callbacks keep their requesting session")
    }
}
