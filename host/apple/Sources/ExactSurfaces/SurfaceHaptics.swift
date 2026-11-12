// @ref LLP 1009 D10. A GPU surface's haptics: the messages `exact_gpu::haptics`
// writes (`exact:haptics <json>`), consumed here and never dispatched to the
// app. On an iPhone they play through Core Haptics: a surface's texture is one
// looping continuous event whose intensity and sharpness move by dynamic
// parameters, sent as each message arrives, and a pattern is a
// `CHHapticPattern` of its events and parameter curves, played at once beside
// it. The host owns their life: a surface's texture stops when the surface is
// destroyed, and every texture stops while the canvases are hidden or
// interrupted (CanvasLifecycle); a later texture message starts it again.
// Elsewhere (the Mac, tvOS, a device with no haptic engine, the driver) the
// messages are consumed and nothing plays.
import ExactKit
import Foundation
#if os(iOS)
import CoreHaptics
#endif

/// One request, parsed.
enum SurfaceHaptic: Equatable {
    case texture(intensity: Float, sharpness: Float)
    case stop
    case pattern(events: [Event], curves: [Curve])

    struct Event: Equatable {
        var transient: Bool
        var time: Double, duration: Double, intensity: Float, sharpness: Float
    }
    struct Curve: Equatable {
        /// Sharpness (added to each event's), else intensity (multiplying it).
        var sharpness: Bool
        /// `[time, value]` pairs.
        var points: [[Double]]
    }

    /// The request in `message`, or nil when it is not one.
    static func parse(_ message: String) -> SurfaceHaptic? {
        guard message.hasPrefix(SurfaceHaptics.prefix),
              let data = message.dropFirst(SurfaceHaptics.prefix.count).data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        func number(_ v: Any?) -> Double { (v as? NSNumber)?.doubleValue ?? 0 }
        if json.keys.contains("texture") {
            guard let t = json["texture"] as? [String: Any] else { return .stop }
            return .texture(intensity: Float(number(t["intensity"])), sharpness: Float(number(t["sharpness"])))
        }
        guard let p = json["pattern"] as? [String: Any] else { return nil }
        let events = (p["events"] as? [[String: Any]] ?? []).map {
            Event(transient: $0["type"] as? String == "transient", time: number($0["time"]), duration: number($0["duration"]),
                  intensity: Float(number($0["intensity"])), sharpness: Float(number($0["sharpness"])))
        }
        let curves = (p["curves"] as? [[String: Any]] ?? []).map {
            Curve(sharpness: $0["parameter"] as? String == "sharpness",
                  points: ($0["points"] as? [[Any]] ?? []).map { $0.map(number) }.filter { $0.count == 2 })
        }
        return .pattern(events: events, curves: curves)
    }
}

/// What plays: Core Haptics on an iPhone, a recorder in tests.
protocol HapticPlayer: AnyObject {
    /// Start `surface`'s texture, or move it to these values.
    func texture(_ surface: UInt32, intensity: Float, sharpness: Float)
    func stopTexture(_ surface: UInt32)
    func play(_ events: [SurfaceHaptic.Event], _ curves: [SurfaceHaptic.Curve])
}

/// Every canvas's haptics on one session's page.
final class SurfaceHaptics {
    static let prefix = "exact:haptics "
    /// Nil where nothing plays.
    let player: HapticPlayer?
    /// The surfaces whose texture plays.
    private(set) var playing: Set<UInt32> = []
    /// While the canvases are hidden or interrupted: no texture plays, and
    /// a message starts none.
    var suspended = false {
        didSet { if suspended { for s in playing { player?.stopTexture(s) }; playing = [] } }
    }

    init(player: HapticPlayer?) { self.player = player }

    static func make() -> SurfaceHaptics {
        #if os(iOS)
        if !ExactEnv.agentMode, CHHapticEngine.capabilitiesForHardware().supportsHaptics {
            return SurfaceHaptics(player: CoreHapticsPlayer())
        }
        #endif
        return SurfaceHaptics(player: nil)
    }

    /// Whether `message` was a haptic message, which the host consumes, played for `surface`.
    func take(_ surface: UInt32, _ message: String) -> Bool {
        guard message.hasPrefix(Self.prefix) else { return false }
        guard let request = SurfaceHaptic.parse(message) else {
            fputs("exact gpu: surface \(surface): a haptic message that is not one: \(message)\n", stderr)
            return true
        }
        switch request {
        case let .texture(intensity, sharpness):
            guard !suspended else { break }
            playing.insert(surface)
            player?.texture(surface, intensity: intensity, sharpness: sharpness)
        case .stop:
            if playing.remove(surface) != nil { player?.stopTexture(surface) }
        case let .pattern(events, curves):
            if !suspended { player?.play(events, curves) }
        }
        return true
    }

    /// The surface is gone: its texture stops.
    func remove(_ surface: UInt32) {
        if playing.remove(surface) != nil { player?.stopTexture(surface) }
    }
}

#if os(iOS)
/// Core Haptics: one engine for the process's canvases, made at the first
/// request, haptics only (no audio session), stopping itself when idle.
final class CoreHapticsPlayer: HapticPlayer {
    private var engine: CHHapticEngine?
    private var textures: [UInt32: CHHapticAdvancedPatternPlayer] = [:]

    /// The engine, started; a stopped or reset one makes its players invalid,
    /// so they are made again at the next request.
    private func ready() -> CHHapticEngine? {
        if let engine { return engine }
        guard let made = try? CHHapticEngine() else { return nil }
        made.playsHapticsOnly = true
        made.isAutoShutdownEnabled = true
        made.stoppedHandler = { [weak self] _ in DispatchQueue.main.async { self?.textures = [:] } }
        made.resetHandler = { [weak self] in DispatchQueue.main.async { self?.textures = [:]; self?.engine = nil } }
        do { try made.start() } catch { fputs("exact gpu: haptics: \(error)\n", stderr); return nil }
        engine = made
        return made
    }

    /// The texture's dynamic parameters: intensity multiplies the event's 1;
    /// sharpness is added to the event's 0.5.
    static func controls(intensity: Float, sharpness: Float) -> [CHHapticDynamicParameter] {
        [CHHapticDynamicParameter(parameterID: .hapticIntensityControl, value: intensity, relativeTime: 0),
         CHHapticDynamicParameter(parameterID: .hapticSharpnessControl, value: sharpness - 0.5, relativeTime: 0)]
    }

    /// A texture's pattern: one continuous event as long as Core Haptics allows, looped.
    static func texturePattern() throws -> CHHapticPattern {
        let event = CHHapticEvent(eventType: .hapticContinuous, parameters: [
            CHHapticEventParameter(parameterID: .hapticIntensity, value: 1),
            CHHapticEventParameter(parameterID: .hapticSharpness, value: 0.5),
        ], relativeTime: 0, duration: 30)
        return try CHHapticPattern(events: [event], parameters: [])
    }

    static func pattern(_ events: [SurfaceHaptic.Event], _ curves: [SurfaceHaptic.Curve]) throws -> CHHapticPattern {
        let made = events.map { e in
            let parameters = [CHHapticEventParameter(parameterID: .hapticIntensity, value: e.intensity),
                              CHHapticEventParameter(parameterID: .hapticSharpness, value: e.sharpness)]
            return e.transient
                ? CHHapticEvent(eventType: .hapticTransient, parameters: parameters, relativeTime: e.time)
                : CHHapticEvent(eventType: .hapticContinuous, parameters: parameters, relativeTime: e.time, duration: e.duration)
        }
        let moved = curves.filter { !$0.points.isEmpty }.map { c in
            CHHapticParameterCurve(parameterID: c.sharpness ? .hapticSharpnessControl : .hapticIntensityControl,
                                   controlPoints: c.points.map { .init(relativeTime: $0[0], value: Float($0[1])) },
                                   relativeTime: 0)
        }
        return try CHHapticPattern(events: made, parameterCurves: moved)
    }

    func texture(_ surface: UInt32, intensity: Float, sharpness: Float) {
        guard let engine = ready() else { return }
        do {
            if let player = textures[surface] {
                try player.sendParameters(Self.controls(intensity: intensity, sharpness: sharpness), atTime: CHHapticTimeImmediate)
                return
            }
            let player = try engine.makeAdvancedPlayer(with: Self.texturePattern())
            player.loopEnabled = true
            try player.start(atTime: CHHapticTimeImmediate)
            try player.sendParameters(Self.controls(intensity: intensity, sharpness: sharpness), atTime: CHHapticTimeImmediate)
            textures[surface] = player
        } catch {
            fputs("exact gpu: haptics: \(error)\n", stderr)
        }
    }

    func stopTexture(_ surface: UInt32) {
        try? textures.removeValue(forKey: surface)?.stop(atTime: CHHapticTimeImmediate)
    }

    func play(_ events: [SurfaceHaptic.Event], _ curves: [SurfaceHaptic.Curve]) {
        guard !events.isEmpty, let engine = ready() else { return }
        do {
            try engine.makePlayer(with: Self.pattern(events, curves)).start(atTime: CHHapticTimeImmediate)
        } catch {
            fputs("exact gpu: haptics: \(error)\n", stderr)
        }
    }
}
#endif
