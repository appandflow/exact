// MetricKit's crash and hang diagnostics as Observe log rows. The system writes them
// on the device and delivers them at a later launch, with call stacks as binary UUIDs
// and offsets, which the build's dSYMs symbolicate offline. A crash's row follows
// expo-observe's `native.exception` (expo-app-metrics `CrashReport.toLogRecord`);
// Observe has no hang row, so a hang's is `native.hang` with `exact.hang.*` keys.
// MetricKit delivers nothing on a simulator.
import CryptoKit
import Foundation

/// One diagnostic, read from MetricKit (`init(crash:…)`, `init(hang:…)`) or made by a test.
struct ObserveDiagnostic {
    enum Kind { case crash, hang }
    var kind: Kind
    /// The payload's window. MetricKit gives no exact time; the end is the tightest bound.
    var begin: Date
    var end: Date
    var exceptionType: Int?
    var exceptionCode: Int?
    var signal: Int?
    var terminationReason: String?
    var virtualMemoryRegion: String?
    /// An uncaught Objective-C or Swift exception's name, type and message (iOS 17, macOS 14).
    var objcException: (name: String, type: String, message: String)?
    var hangSeconds: Double?
    /// `MXCallStackTree.jsonRepresentation()`.
    var callStackTree: Data

    static let maxFrames = 50
    static let maxText = 65_536

    /// The row's name, severity and attributes.
    func row() -> (name: String, severity: String, attributes: [String: Any]) {
        let tree = (try? JSONSerialization.jsonObject(with: callStackTree)) as? [String: Any]
        let stacks = tree?["callStacks"] as? [[String: Any]] ?? []
        let attributed = stacks.filter { $0["threadAttributed"] as? Bool == true }
        var frames: [[String: Any]] = [], total = 0
        func collect(_ list: [[String: Any]]) {
            for f in list {
                total += 1
                if frames.count < Self.maxFrames { frames.append(f) }
                collect(f["subFrames"] as? [[String: Any]] ?? [])
            }
        }
        for s in attributed.isEmpty ? stacks : attributed { collect(s["callStackRootFrames"] as? [[String: Any]] ?? []) }
        var lines = frames.map(Self.render)
        if total > frames.count { lines.append("… +\(total - frames.count) more frames") }
        var uuids: [String: String] = [:]
        for f in frames { if let name = f["binaryName"] as? String, let uuid = f["binaryUUID"] as? String { uuids[name] = uuid } }
        let binaries = uuids.sorted { $0.key < $1.key }.map { "\($0.key):\($0.value)" }
        let trace = lines.isEmpty ? nil : String(lines.joined(separator: "\n").prefix(Self.maxText))
        // The whole tree, for symbolication with every thread; a tree over the cap is left out.
        let json = String(decoding: callStackTree, as: UTF8.self)
        let fullTree = json.utf8.count <= Self.maxText && !json.isEmpty ? json : nil

        switch kind {
        case .hang:
            var a: [String: Any] = [:]
            a["exact.hang.duration"] = hangSeconds
            a["exact.hang.stacktrace"] = trace
            a["exact.hang.call_stack_tree"] = fullTree
            if !binaries.isEmpty { a["exact.hang.binary_uuids"] = binaries }
            return ("native.hang", "warn", a)
        case .crash:
            let message = [terminationReason, objcException?.message].compactMap { $0?.isEmpty == false ? $0 : nil }
            var a: [String: Any] = [
                "exception.type": objcException?.name ?? exceptionType.map(Self.exceptionName) ?? signal.map(Self.signalName) ?? "NativeCrash",
                "exception.message": message.isEmpty ? signal.map(Self.signalName) ?? exceptionType.map(Self.exceptionName) ?? "Native crash" : message.joined(separator: "\n"),
                "expo.error.source": "nativeCrash",
                "expo.error.is_fatal": true,
                "exact.crash.origin": "metrickit",
            ]
            a["exception.stacktrace"] = trace
            if let exceptionType {
                a["expo.crash.exception_type"] = Self.exceptionName(exceptionType)
                a["expo.crash.exception_type_code"] = exceptionType
            }
            a["expo.crash.exception_code"] = exceptionCode
            if let signal {
                a["expo.crash.signal"] = Self.signalName(signal)
                a["expo.crash.signal_number"] = signal
            }
            a["expo.crash.termination_reason"] = terminationReason
            a["expo.crash.virtual_memory_region"] = virtualMemoryRegion
            if let objcException {
                a["expo.crash.objc_exception_type"] = objcException.type
                a["expo.crash.objc_exception_message"] = objcException.message
            }
            if !stacks.isEmpty {
                a["expo.crash.thread_count"] = stacks.count
                a["expo.crash.thread_attributed"] = !attributed.isEmpty
            }
            if !frames.isEmpty {
                a["expo.crash.rendered_frame_count"] = frames.count
                a["expo.crash.total_frame_count"] = total
                if !binaries.isEmpty { a["expo.crash.binary_uuids"] = binaries }
            }
            a["exact.crash.call_stack_tree"] = fullTree
            return ("native.exception", "fatal", a)
        }
    }

    /// `<binary> + <offset>`, or the address when there is no offset.
    static func render(_ f: [String: Any]) -> String {
        if let name = f["binaryName"] as? String, let offset = (f["offsetIntoBinaryTextSegment"] as? NSNumber)?.uint64Value { return "\(name) + \(offset)" }
        if let address = (f["address"] as? NSNumber)?.uint64Value { return "0x\(String(address, radix: 16))" }
        return f["binaryName"] as? String ?? "<unknown>"
    }

    /// Mach exception types, `<mach/exception_types.h>`.
    static func exceptionName(_ type: Int) -> String {
        [1: "EXC_BAD_ACCESS", 2: "EXC_BAD_INSTRUCTION", 3: "EXC_ARITHMETIC", 4: "EXC_EMULATION", 5: "EXC_SOFTWARE",
         6: "EXC_BREAKPOINT", 10: "EXC_CRASH", 11: "EXC_RESOURCE", 12: "EXC_GUARD"][type] ?? "EXC_\(type)"
    }

    static func signalName(_ signal: Int) -> String {
        [4: "SIGILL", 5: "SIGTRAP", 6: "SIGABRT", 8: "SIGFPE", 9: "SIGKILL", 10: "SIGBUS", 11: "SIGSEGV"][signal] ?? "SIG\(signal)"
    }

    /// Identifies a diagnostic across launches: MetricKit hands past payloads to each launch again.
    var key: String {
        var d = Data()
        d.append(contentsOf: Array("\(kind)|\(begin.timeIntervalSince1970)|\(end.timeIntervalSince1970)|\(signal ?? -1)|\(exceptionType ?? -1)|\(hangSeconds ?? -1)|".utf8))
        d.append(callStackTree)
        return SHA256.hash(data: d).map { String(format: "%02x", $0) }.joined()
    }
}

extension ObserveService {
    /// Stores each diagnostic not stored before, attributed to the latest session that started
    /// by the end of its window: an earlier one for a crash, which ended its process, and
    /// this one when none did. Runs on `queue`.
    func ingest(_ diagnostics: [ObserveDiagnostic]) {
        guard let store else { return }
        for d in diagnostics {
            let key = d.key
            if !store.rows("SELECT 1 FROM diagnostics WHERE key = ?", [key]).isEmpty { continue }
            let crashed = store.rows("SELECT id FROM sessions WHERE id != ? AND start <= ? ORDER BY start DESC LIMIT 1",
                                     [d.kind == .crash ? session : "", d.end.timeIntervalSince1970]).first?.first as? String ?? session
            let row = d.row()
            if store.addLog(session: crashed, time: d.end.timeIntervalSince1970, severity: row.severity, name: row.name, body: nil, attributes: row.attributes, dropped: 0) {
                store.run("INSERT INTO diagnostics (key, time) VALUES (?, ?)", [key, Date().timeIntervalSince1970])
            }
        }
        if !diagnostics.isEmpty { scheduleDispatch() }
    }
}

#if canImport(MetricKit) && !os(tvOS) && !os(watchOS)
import MetricKit

extension ObserveDiagnostic {
    init(crash d: MXCrashDiagnostic, payload: MXDiagnosticPayload) {
        self.init(kind: .crash, begin: payload.timeStampBegin, end: payload.timeStampEnd, exceptionType: d.exceptionType?.intValue,
                  exceptionCode: d.exceptionCode?.intValue, signal: d.signal?.intValue, terminationReason: d.terminationReason,
                  virtualMemoryRegion: d.virtualMemoryRegionInfo, callStackTree: d.callStackTree.jsonRepresentation())
        if #available(iOS 17.0, macOS 14.0, *), let reason = d.exceptionReason {
            objcException = (reason.exceptionName, reason.exceptionType, reason.composedMessage)
        }
    }

    init(hang d: MXHangDiagnostic, payload: MXDiagnosticPayload) {
        self.init(kind: .hang, begin: payload.timeStampBegin, end: payload.timeStampEnd,
                  hangSeconds: d.hangDuration.converted(to: .seconds).value, callStackTree: d.callStackTree.jsonRepresentation())
    }

    static func all(_ payloads: [MXDiagnosticPayload]) -> [ObserveDiagnostic] {
        payloads.flatMap { p in
            (p.crashDiagnostics ?? []).map { ObserveDiagnostic(crash: $0, payload: p) } + (p.hangDiagnostics ?? []).map { ObserveDiagnostic(hang: $0, payload: p) }
        }
    }
}

/// MetricKit's subscriber. MetricKit calls it on its own queue; the service stores on its queue.
final class ObserveMetricKit: NSObject, MXMetricManagerSubscriber {
    weak var service: ObserveService?

    init(service: ObserveService) {
        self.service = service
        super.init()
        MXMetricManager.shared.add(self)
        // Payloads from earlier launches that the system kept, delivered again here.
        receive(MXMetricManager.shared.pastDiagnosticPayloads)
    }

    func didReceive(_ payloads: [MXDiagnosticPayload]) { receive(payloads) }

    private func receive(_ payloads: [MXDiagnosticPayload]) {
        let diagnostics = ObserveDiagnostic.all(payloads)
        guard !diagnostics.isEmpty, let service else { return }
        service.queue.async { service.ingest(diagnostics) }
    }
}
#endif
