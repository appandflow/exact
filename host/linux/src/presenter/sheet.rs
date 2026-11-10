//! A modal route that declares detents (`navigationDetent`) is a bottom
//! sheet, as Material 3's modal bottom sheet shows one: laid out in a block
//! as tall as its detent and anchored to the window's bottom, over a scrim
//! that dims the page behind and dismisses it when pressed; it slides up
//! as it opens and down as it leaves (its last drawing, `paint/exit.rs`).
//! Its top edge is a drag handle: a drag there moves it between its detents
//! or down out of the window, and settles by the finger's speed. Dismissing
//! it is the route's Back, as iOS's sheet ends in `invokeBack`. `medium` is
//! half the window, `large` the window below the status bar, a percentage
//! a share of that; `fit-content` is taken as `large`.
use super::*;
use crate::paint::{Leaving, SheetPaint};
use exact_kernel::{BlockAxis, HostBlock, NodeKey};

/// Material 3's motion: an opening sheet's emphasized decelerate, its
/// exit and a settle between detents.
const ENTER_MS: f64 = 350.0;
const EXIT_MS: f64 = 200.0;
const SETTLE_MS: f64 = 300.0;
/// The drag handle's touch target at the sheet's top, points.
const GRAB: f32 = 48.0;
/// How far a finger goes on the handle before the sheet follows it,
/// points: Android's touch slop.
const SLOP: f32 = 8.0;
/// A release this fast (points a second) goes on to the next detent.
const FLING: f32 = 500.0;

#[derive(Default)]
pub(crate) struct Sheets {
    shown: Option<Shown>,
    closing: Option<Closing>,
    gesture: Option<Gesture>,
}

struct Shown {
    route: ViewId,
    key: NodeKey,
    parent: NodeKey,
    /// The detents' heights, shortest first, points.
    heights: Vec<f32>,
    at: usize,
    top: Motion,
}

struct Closing {
    key: NodeKey,
    parent: NodeKey,
    /// Its top edge when it was last drawn.
    from: f32,
    top: Motion,
}

#[derive(Clone, Copy)]
enum Gesture {
    Scrim,
    Drag {
        finger: f32,
        start: f32,
        top: f32,
        last: (f32, f64),
        velocity: f32,
        /// Past the slop: until then a tap there presses what it is on.
        moving: bool,
    },
}

#[derive(Clone, Copy)]
struct Motion {
    from: f32,
    to: f32,
    start: f64,
    ms: f64,
}

impl Motion {
    fn still(at: f32) -> Self {
        Self {
            from: at,
            to: at,
            start: 0.0,
            ms: 0.0,
        }
    }

    fn at(&self, now: f64) -> f32 {
        if self.ms <= 0.0 || now >= self.start + self.ms {
            return self.to;
        }
        let t = ((now - self.start) / self.ms).clamp(0.0, 1.0) as f32;
        self.from + (self.to - self.from) * (1.0 - (1.0 - t).powi(3))
    }

    fn moving(&self, now: f64) -> bool {
        self.ms > 0.0 && now < self.start + self.ms
    }
}

/// The detents' heights in a window `h` high whose status bar is `status`.
fn heights(detent: &str, h: f32, status: f32) -> Vec<f32> {
    let tallest = (h - status).max(0.0);
    let mut out: Vec<f32> = detent
        .split_whitespace()
        .filter_map(|d| match d {
            "medium" => Some(h * 0.5),
            "large" | "fit-content" => Some(tallest),
            d => match d.strip_suffix('%') {
                Some(p) => p.parse::<f32>().ok().map(|p| tallest * p / 100.0),
                None => d.parse::<f32>().ok(),
            },
        })
        .map(|v| v.clamp(0.0, tallest))
        .collect();
    out.sort_by(f32::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    out
}

impl<D: DataSource> Presenter<D> {
    /// The modal route with detents that shows now, its parent, and its
    /// detents' heights.
    fn sheet_route(&self) -> Option<(ViewId, NodeKey, NodeKey, Vec<f32>)> {
        let kernel = self.host.kernel();
        let status = kernel.env().top;
        let h = self.viewport.1;
        (self.host.bars().stacks.iter().flatten()).find_map(|route| {
            let node = kernel.node(*route)?;
            if node.props.str(PropId::NavigationPresentation) != Some("modal")
                || self.host.route_visibility(*route) != (false, false)
            {
                return None;
            }
            let heights = heights(node.props.str(PropId::NavigationDetent)?, h, status);
            let parent = kernel.node(node.parent?)?.key;
            (!heights.is_empty()).then_some((*route, node.key, parent, heights))
        })
    }

    fn sheet_top(&self, now: f64) -> Option<f32> {
        let s = self.sheets.shown.as_ref()?;
        Some(match self.sheets.gesture {
            Some(Gesture::Drag { top, .. }) => top,
            _ => s.top.at(now),
        })
    }

    /// Follow the route that shows: a new one slides up, one that went
    /// slides down; the block follows the sheet's height.
    pub(super) fn sync_sheet(&mut self) {
        let now = self.sheet_clock();
        let h = self.viewport.1;
        let found = self.sheet_route();
        let same = match (&self.sheets.shown, &found) {
            (Some(s), Some(f)) => s.key == f.1,
            (None, None) => true,
            _ => false,
        };
        if same {
            if let (Some(s), Some(f)) = (&mut self.sheets.shown, found) {
                if s.heights != f.3 {
                    s.at = s.at.min(f.3.len() - 1);
                    s.top = Motion::still(h - f.3[s.at]);
                    s.heights = f.3;
                }
            }
        } else {
            if let Some(s) = self.sheets.shown.take() {
                let from = match self.sheets.gesture {
                    Some(Gesture::Drag { top, .. }) => top,
                    _ => s.top.at(now),
                };
                self.sheets.closing = Some(Closing {
                    key: s.key,
                    parent: s.parent,
                    from,
                    top: Motion {
                        from,
                        to: h,
                        start: now,
                        ms: EXIT_MS,
                    },
                });
                if self.host.kernel().node(s.route).is_some() {
                    let _ = (self.host.runner_mut().kernel_mut()).set_host_block(s.route, None);
                    self.layout_for_sheet();
                }
            }
            self.sheets.gesture = None;
            if let Some((route, key, parent, heights)) = found {
                let still = self.host.runner().viewport().preferences.reduced_motion;
                self.sheets.shown = Some(Shown {
                    route,
                    key,
                    parent,
                    top: Motion {
                        from: h,
                        to: h - heights[0],
                        start: now,
                        ms: if still { 0.0 } else { ENTER_MS },
                    },
                    heights,
                    at: 0,
                });
            }
        }
        if (self.sheets.closing)
            .as_ref()
            .is_some_and(|c| !c.top.moving(now))
        {
            self.sheets.closing = None;
        }
        self.dirty |= self.sheet_moving();
        self.place_sheet(now);
    }

    /// The route's block: the window's width, and the sheet's height from
    /// its top edge to the window's bottom, never shorter than its detent.
    fn place_sheet(&mut self, now: f64) {
        let Some(top) = self.sheet_top(now) else {
            return;
        };
        let s = self.sheets.shown.as_ref().expect("a top");
        let (w, h) = self.viewport;
        let tallest = s.heights[s.heights.len() - 1];
        let height = (h - top).max(s.heights[s.at]).min(tallest).round();
        let block = HostBlock {
            width: BlockAxis::Is(w),
            height: BlockAxis::Is(height),
        };
        let route = s.route;
        if self.host.kernel().host_block(route) != Some(block) {
            match (self.host.runner_mut().kernel_mut()).set_host_block(route, Some(block)) {
                Ok(()) => self.layout_for_sheet(),
                Err(e) => self.host.log(format!("sheet: {e:?}")),
            }
        }
    }

    fn layout_for_sheet(&mut self) {
        if let Err(e) = self.host.layout_unauthored() {
            self.host.log(e);
        }
        self.dirty = true;
    }

    /// What the painter shows of the sheet, and a sheet's leaving drawing.
    pub(super) fn sheet_paint(&self) -> (Option<SheetPaint>, Option<Leaving>) {
        let now = self.sheet_clock();
        let h = self.viewport.1;
        if let (Some(s), Some(top)) = (&self.sheets.shown, self.sheet_top(now)) {
            let scrim = (h - top) / s.heights[0];
            let paint = SheetPaint {
                route: Some(s.route),
                parent: s.parent,
                top,
                scrim,
            };
            return (Some(paint), None);
        }
        let Some(c) = &self.sheets.closing else {
            return (None, None);
        };
        let top = c.top.at(now);
        let paint = SheetPaint {
            route: None,
            parent: c.parent,
            top,
            scrim: (h - top) / (h - c.from).max(1.0),
        };
        let leaving = Leaving {
            key: c.key,
            parent: c.parent,
            opacity: None,
            translate: Some(((0.0, top - c.from), (0.0, 0.0))),
            scale: None,
            rotate: None,
        };
        (Some(paint), Some(leaving))
    }

    /// Whether the sheet moves on its own: frames are owed.
    pub(crate) fn sheet_moving(&self) -> bool {
        let now = self.sheet_clock();
        (self.sheets.shown.as_ref()).is_some_and(|s| s.top.moving(now))
            || self.sheets.closing.is_some()
    }

    /// Whether a contact at `y` is the sheet's (its scrim or its handle),
    /// which claims both axes from the scrollers under it.
    pub(crate) fn sheet_claims(&self, y: f32) -> bool {
        self.sheet_top(self.sheet_clock())
            .is_some_and(|top| y < top + GRAB)
    }

    /// A contact down: on the scrim, or on the handle.
    pub(super) fn sheet_down(&mut self, y: f32, now: f64) -> bool {
        let Some(top) = self.sheet_top(self.sheet_clock()) else {
            return false;
        };
        self.sheets.gesture = if y < top {
            Some(Gesture::Scrim)
        } else if y < top + GRAB {
            Some(Gesture::Drag {
                finger: y,
                start: top,
                top,
                last: (y, now),
                velocity: 0.0,
                moving: false,
            })
        } else {
            None
        };
        matches!(self.sheets.gesture, Some(Gesture::Scrim))
    }

    pub(super) fn sheet_move(&mut self, y: f32, now: f64) -> bool {
        let h = self.viewport.1;
        let Some(s) = &self.sheets.shown else {
            return false;
        };
        let highest = h - s.heights[s.heights.len() - 1];
        // Past the slop the handle's drag is the sheet's: what the contact
        // pressed is let go.
        if let Some(Gesture::Drag {
            finger,
            moving: false,
            ..
        }) = self.sheets.gesture
        {
            if (y - finger).abs() <= SLOP {
                return false;
            }
            let gesture = self.sheets.gesture.take();
            let _ = self.pointer_cancel(now);
            self.sheets.gesture = gesture;
            if let Some(Gesture::Drag { moving, .. }) = &mut self.sheets.gesture {
                *moving = true;
            }
        }
        match &mut self.sheets.gesture {
            Some(Gesture::Drag {
                finger,
                start,
                top,
                last,
                velocity,
                ..
            }) => {
                *top = (*start + y - *finger).clamp(highest, h);
                let dt = (now - last.1) as f32 / 1000.0;
                if dt > 0.0 {
                    let v = (y - last.0) / dt;
                    *velocity = if dt > 0.1 {
                        v
                    } else {
                        0.6 * v + 0.4 * *velocity
                    };
                    *last = (y, now);
                }
                self.dirty = true;
                self.place_sheet(self.sheet_clock());
                true
            }
            Some(Gesture::Scrim) => true,
            None => false,
        }
    }

    /// A contact up: a press on the scrim dismisses; a drag settles on the
    /// detent its speed carries it to, or out of the window.
    pub(super) fn sheet_up(&mut self, y: f32, now: f64) -> bool {
        let Some(gesture) = self.sheets.gesture.take() else {
            return false;
        };
        match gesture {
            Gesture::Scrim => {
                let closes = (self.sheets.shown.as_ref())
                    .and_then(|s| self.host.kernel().node(s.route))
                    .is_some_and(|n| n.props.str(PropId::Closedby) != Some("none"));
                if closes
                    && self
                        .sheet_top(self.sheet_clock())
                        .is_some_and(|top| y < top)
                {
                    self.system_back(now);
                }
            }
            Gesture::Drag { moving: false, .. } => return false,
            Gesture::Drag { top, velocity, .. } => self.settle_sheet(top, velocity, now),
        }
        true
    }

    /// A contact cancelled: a drag that moved the sheet settles. One on
    /// the handle still within the slop stays, as the presser's own down
    /// cancels what came before it.
    pub(super) fn sheet_cancel(&mut self, now: f64) {
        if let Some(Gesture::Drag {
            top, moving: true, ..
        }) = self.sheets.gesture
        {
            self.sheets.gesture = None;
            self.settle_sheet(top, 0.0, now);
        }
    }

    fn settle_sheet(&mut self, top: f32, velocity: f32, now: f64) {
        let (h, clock) = (self.viewport.1, self.sheet_clock());
        let Some(s) = &mut self.sheets.shown else {
            return;
        };
        // Each rest's top edge, highest last, then out of the window.
        let mut rests: Vec<f32> = s.heights.iter().rev().map(|d| h - d).collect();
        rests.reverse();
        rests.push(h);
        rests.sort_by(f32::total_cmp);
        let target = if velocity > FLING {
            rests.iter().copied().find(|r| *r > top + 1.0).unwrap_or(h)
        } else if velocity < -FLING {
            rests
                .iter()
                .rev()
                .copied()
                .find(|r| *r < top - 1.0)
                .unwrap_or(rests[0])
        } else {
            rests
                .iter()
                .copied()
                .min_by(|a, b| (a - top).abs().total_cmp(&(b - top).abs()))
                .unwrap_or(h)
        };
        s.top = Motion {
            from: top,
            to: target,
            start: clock,
            ms: SETTLE_MS,
        };
        if target >= h - 0.5 {
            s.top = Motion::still(top);
            self.system_back(now);
            return;
        }
        if let Some(i) = s.heights.iter().position(|d| (h - d - target).abs() < 0.5) {
            s.at = i;
        }
        self.dirty = true;
        self.place_sheet(clock);
    }

    /// The clock the sheet moves by: the display's, as Material's animator
    /// runs, not the app's, which a driver holds still between its steps
    /// (a sheet held at its start would be out of the window). Tests step
    /// the host's.
    fn sheet_clock(&self) -> f64 {
        if cfg!(test) {
            return self.host.now();
        }
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1000.0
    }
}
