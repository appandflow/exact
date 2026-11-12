//! Bounded, opt-in Contract snapshots. No samples or allocations while disabled.
use crate::{perf::Ring, Stats};
use exact_gpu::{Frame, FrameTiming};
use std::fmt::Write;

#[derive(Default)]
pub(crate) struct Report {
    pub phases: [Ring; 3],
    frames: u64,
    late: u64,
    missed: u64,
    gap: f64,
    has_period: bool,
    pixels: (u32, u32),
    scale: f32,
    ratio: f32,
    samples: u32,
    stats: Stats,
    pending: bool,
    pending_gap: Option<(f64, f64)>,
    ready: bool,
}
impl Report {
    pub fn encoded(
        &mut self,
        frame: &Frame,
        timing: &FrameTiming,
        scale: f32,
        samples: u32,
        stats: Stats,
        cap: u32,
    ) {
        self.pixels = frame.pixels();
        self.scale = scale;
        self.ratio = frame.scale;
        self.samples = samples;
        self.stats = stats;
        self.pending = true;
        let period = timing.display_period_ms;
        let expected = if cap > 0 {
            // The module honors caps down to 5 Hz (exact_gpu::Module::max_fps).
            period.max(1000. / f64::from(cap.max(5)))
        } else {
            period
        };
        self.pending_gap = (!frame.seekable && expected.is_finite() && expected > 0.)
            .then_some((timing.interval_ms, expected));
    }

    pub fn submitted(&mut self) {
        if std::mem::take(&mut self.pending) {
            self.frames += 1;
            self.ready = true;
            if let Some((gap, expected)) = self.pending_gap.take() {
                if gap.is_finite() && gap > 0. {
                    self.has_period = true;
                    self.gap = self.gap.max(gap);
                    self.late += u64::from(gap > expected * 1.5);
                    self.missed += ((gap / expected).round() as u64).saturating_sub(1);
                }
            }
        }
    }
    pub fn take(&mut self) -> Option<String> {
        if !self.ready {
            return None;
        }
        self.ready = false;
        let mut out = format!("{{\"pixelWidth\":{},\"pixelHeight\":{},\"renderScale\":{},\"effectivePixelRatio\":{},\"devicePixelRatio\":{},\"samples\":{},\"draws\":{},\"triangles\":{},\"instances\":{},\"frames\":{}",
            self.pixels.0, self.pixels.1, self.scale, self.ratio, self.ratio / self.scale,
            self.samples, self.stats.draws, self.stats.triangles, self.stats.instances, self.frames);
        if self.has_period {
            write!(
                out,
                ",\"lateFrames\":{},\"missedFrames\":{},\"longestGapMs\":{}",
                self.late, self.missed, self.gap
            )
            .unwrap();
        }
        for (name, ring) in ["Tick", "Feed", "Encode"].into_iter().zip(&mut self.phases) {
            if ring.count > 0 {
                write!(
                    out,
                    ",\"cpu{name}Ms\":{},\"cpu{name}MaxMs\":{}",
                    ring.sum / ring.count as f64,
                    ring.max
                )
                .unwrap();
            }
            *ring = Ring::default();
        }
        out.push('}');
        self.frames = 0;
        self.late = 0;
        self.missed = 0;
        self.gap = 0.;
        self.has_period = false;
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_count_submitted_frames_honor_caps_and_reset_the_window() {
        let mut report = Report::default();
        let frame = Frame {
            width: 100.,
            height: 50.,
            scale: 1.5,
            now_ms: 0.,
            seekable: false,
            period_ms: 1000. / 120.,
            children_generation: 0,
            shader_generation: 0,
            headroom: 1.,
        };
        let mut timing = FrameTiming {
            interval_ms: 0.,
            display_period_ms: 1000. / 120.,
            ..Default::default()
        };
        report.encoded(&frame, &timing, 0.75, 4, Stats::default(), 30);
        report.submitted();
        let value: serde_json::Value = serde_json::from_str(&report.take().unwrap()).unwrap();
        assert!(
            value.get("longestGapMs").is_none(),
            "first-after-rest has no interval"
        );
        assert!(value.get("lateFrames").is_none());
        timing.interval_ms = 1000. / 30.;
        report.encoded(
            &frame,
            &timing,
            0.75,
            4,
            Stats {
                draws: 5,
                ..Default::default()
            },
            30,
        );
        assert!(report.take().is_none(), "encoding is not submission");
        report.phases[0].push(2.);
        report.phases[0].push(4.);
        report.submitted();
        let value: serde_json::Value = serde_json::from_str(&report.take().unwrap()).unwrap();
        assert_eq!(value["pixelWidth"], 150);
        assert_eq!(value["devicePixelRatio"], 2.);
        assert_eq!(value["lateFrames"], 0);
        assert_eq!(value["missedFrames"], 0);
        assert_eq!(value["cpuTickMs"], 3.);
        assert_eq!(value["cpuTickMaxMs"], 4.);
        assert!(value.get("gpuMs").is_none());
        timing.interval_ms *= 2.;
        report.encoded(&frame, &timing, 0.75, 4, Stats::default(), 30);
        report.submitted();
        let value: serde_json::Value = serde_json::from_str(&report.take().unwrap()).unwrap();
        assert_eq!(value["frames"], 1);
        assert_eq!(value["lateFrames"], 1);
        assert_eq!(value["missedFrames"], 1);
        assert!(value.get("cpuTickMs").is_none());
        timing.interval_ms = 400.;
        report.encoded(&frame, &timing, 0.75, 4, Stats::default(), 1);
        report.submitted();
        let value: serde_json::Value = serde_json::from_str(&report.take().unwrap()).unwrap();
        assert_eq!(value["missedFrames"], 1, "caps below 5 Hz draw at 5 Hz");
    }
}
