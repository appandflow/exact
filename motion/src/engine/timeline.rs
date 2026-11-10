//! Drag and view timelines in the engine.
//!
//! A drag source node's presented `translate` drives a timeline on one axis;
//! a view timeline's position is its subject's progress through its
//! scroller, which the host hands over whenever the scroller moves or a
//! layout moves the subject ([`Engine::set_view_progress`]). A consumer's
//! `animation`s are bound to what its timeline name resolved to, over a
//! range. The host resolves names as CSS scopes them (the kernel's
//! `timeline-scope` lookup), so the engine hears a node, never a name. A
//! bound consumer's plays are held (as `animation-play-state: paused` holds
//! them) at the local time the source's position gives, re-sought whenever
//! a frame is taken, so a held value and a release spring move the consumer
//! in the frame they move the source. A sampling host paints it; a lowering
//! host samples the node (its compositor's time is the clock's) or seeks its
//! own executor's copy, as the web's glue does.
//!
//! What a name that finds no single source shows is Chrome's (Scroll-driven
//! Animations 1 §4.2, Web Animations 2's timeline switch):
//! - [`NamedTimeline::Inactive`]: a `timeline-scope` with no declaring
//!   descendant, or several. The animations' current time is unresolved, so
//!   they have no effect: the property's own value shows, whatever the fill.
//! - [`NamedTimeline::Missing`]: no timeline of that name in scope. The
//!   animation has no timeline and keeps the time it has: where its source
//!   left it, still out of effect after an inactive one, 0 if it is new.

use super::Engine;
use crate::property::Property;

/// What a consumer's timeline name resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedTimeline {
    /// The drag timeline this node's presented translate drives.
    Source(u64),
    /// The view timeline (CSS `view-timeline`) this node is the subject of:
    /// its position is the progress the host last gave, 0 to 1 over CSS's
    /// `cover` range. Until the host gives one it is inactive.
    View(u64),
    /// An inactive timeline: its animations have no effect.
    Inactive,
    /// No timeline: its animations keep the time they have.
    Missing,
}

/// The drag timelines, in declaration order. A page has a handful; a scan
/// is cheaper, in code and time, than a map.
#[derive(Debug, Default)]
pub(super) struct Timelines {
    /// Each source node, and whether it reads the `x` axis.
    sources: Vec<(u64, bool)>,
    /// Each consumer node, what its name resolved to, and its range in points.
    bound: Vec<(u64, NamedTimeline, [f64; 2])>,
    /// Each view timeline subject the host has placed, and its progress.
    views: Vec<(u64, f64)>,
}

impl Timelines {
    /// Whether `node` is a timeline's source or follows one.
    pub(super) fn names(&self, node: u64) -> bool {
        self.sources.iter().any(|s| s.0 == node)
            || self.bound.iter().any(|b| b.0 == node)
            || self.views.iter().any(|v| v.0 == node)
    }
}

impl Engine {
    /// Declare (or with `None`, retract) the timeline `node`'s presented
    /// translate drives, and whether it reads the `x` axis.
    pub fn set_drag_timeline(&mut self, node: u64, x: Option<bool>) {
        (self.links.set_drag_timeline)(self, node, x)
    }

    pub(super) fn set_drag_timeline_full(&mut self, node: u64, x: Option<bool>) {
        let at = self.timelines.sources.iter().position(|s| s.0 == node);
        match (x, at) {
            (Some(x), Some(i)) => self.timelines.sources[i].1 = x,
            (Some(x), None) => self.timelines.sources.push((node, x)),
            (None, Some(i)) => {
                self.timelines.sources.remove(i);
            }
            (None, None) => return,
        }
        for i in 0..self.timelines.bound.len() {
            if self.timelines.bound[i].1 == NamedTimeline::Source(node) {
                self.seek_timeline(i);
            }
        }
    }

    /// The progress of the view timeline whose subject is `node`: 0 where
    /// the subject's start edge meets the scrollport's end edge, 1 where its
    /// end edge leaves the scrollport's start edge (Scroll-driven Animations
    /// 1 §3.2, `cover`). Unclamped. Its consumers are held there at once.
    pub fn set_view_progress(&mut self, node: u64, progress: f64) {
        (self.links.set_view_progress)(self, node, progress)
    }

    pub(super) fn set_view_progress_full(&mut self, node: u64, progress: f64) {
        match self.timelines.views.iter_mut().find(|v| v.0 == node) {
            Some(v) if v.1.to_bits() == progress.to_bits() => return,
            Some(v) => v.1 = progress,
            None => self.timelines.views.push((node, progress)),
        }
        for i in 0..self.timelines.bound.len() {
            if self.timelines.bound[i].1 == NamedTimeline::View(node) {
                self.seek_timeline(i);
            }
        }
    }

    /// Bind (or with `None`, unbind) `node`'s animations to the timeline its
    /// name resolved to, over `range`: at `range[0]` they are at their
    /// start, at `range[1]` at their end.
    pub fn set_animation_timeline(
        &mut self,
        node: u64,
        binding: Option<(NamedTimeline, [f64; 2])>,
    ) {
        (self.links.set_animation_timeline)(self, node, binding)
    }

    pub(super) fn set_animation_timeline_full(
        &mut self,
        node: u64,
        binding: Option<(NamedTimeline, [f64; 2])>,
    ) {
        let at = self.timelines.bound.iter().position(|b| b.0 == node);
        match (binding, at) {
            (Some((timeline, range)), Some(i)) => {
                self.timelines.bound[i] = (node, timeline, range);
                self.seek_timeline(i);
            }
            (Some((timeline, range)), None) => {
                self.timelines.bound.push((node, timeline, range));
                self.seek_timeline(self.timelines.bound.len() - 1);
            }
            (None, Some(i)) => {
                self.timelines.bound.remove(i);
                // Back on the clock from where the timeline left it; from
                // an inactive one, from its start.
                // With the first-frame rule on, that resume waits for the
                // frame; a clock's join below decides for
                // its own.
                let now = self.sample_time();
                let pending = self.start_on_frame.then_some(now);
                for play in self.animations.get_mut(&node).into_iter().flatten() {
                    if !play.animation.paused {
                        if let Some(held) = play.hold.take() {
                            play.start = now - if held.is_nan() { 0.0 } else { held };
                            play.pending = pending;
                        }
                    }
                }
                if pending.is_some() {
                    self.schedule_animations(node);
                }
                // Onto a clock timeline: in its phase.
                self.join_clock_node(node);
            }
            (None, None) => {}
        }
    }

    /// Whether `node`'s animations follow a timeline instead of the clock.
    pub fn timeline_bound(&self, node: u64) -> bool {
        self.timelines.bound.iter().any(|b| b.0 == node)
    }

    pub(super) fn forget_timelines(&mut self, node: u64) {
        self.timelines.sources.retain(|s| s.0 != node);
        self.timelines.bound.retain(|b| b.0 != node);
        self.timelines.views.retain(|v| v.0 != node);
    }

    /// Hold every bound consumer's plays at the time its timeline gives.
    pub(super) fn seek_timelines(&mut self) {
        (self.links.seek_timelines)(self)
    }

    pub(super) fn seek_timelines_full(&mut self) {
        for i in 0..self.timelines.bound.len() {
            self.seek_timeline(i);
        }
    }

    /// Re-seek a bound node whose plays were just set.
    pub(super) fn seek_bound(&mut self, node: u64) {
        if let Some(i) = self.timelines.bound.iter().position(|b| b.0 == node) {
            self.seek_timeline(i);
        }
    }

    fn seek_timeline(&mut self, i: usize) {
        let (node, timeline, [a, b]) = self.timelines.bound[i];
        // The source's presented translate on its axis, as progress through
        // the range; `None` for no timeline, NaN for an inactive one. A
        // source the engine has not heard of yet is no timeline until it is.
        let progress = match timeline {
            NamedTimeline::Source(s) => self
                .timelines
                .sources
                .iter()
                .find(|x| x.0 == s)
                .and_then(|&(_, x)| Some((self.value(s, Property::Translate)?, x)))
                .map(|(v, x)| {
                    // Unclamped: outside the range the animation is before or
                    // after its interval, and its fill decides, as on a CSS
                    // scroll timeline.
                    let p = ((if x { v.x } else { v.y }) - a) / (b - a);
                    if p.is_finite() {
                        p
                    } else {
                        0.0
                    }
                }),
            // A subject the host has not placed is an inactive timeline.
            // Before or after the range the fill holds one value however far
            // out, so one time stands for each side: a subject that scrolls
            // while out of view (most of a long rail's) repaints nothing.
            NamedTimeline::View(s) => Some(self.timelines.views.iter().find(|v| v.0 == s).map_or(
                f64::NAN,
                |&(_, p)| match (p - a) / (b - a) {
                    p if p < 0.0 => -1.0,
                    p if p > 1.0 => 2.0,
                    p => p,
                },
            )),
            NamedTimeline::Inactive => Some(f64::NAN),
            NamedTimeline::Missing => None,
        };
        let Some(plays) = self.animations.get_mut(&node) else {
            return;
        };
        let mut moved = false;
        for play in plays.iter_mut() {
            // The range spans the delay and the active interval together, as
            // Web Animations 2 converts an animation's times on a
            // progress-based timeline (Chrome agrees: the parity fixture's
            // `tl-delay`). An endless animation has no end to span and holds
            // its start, a declared deviation: CSS gives it no duration and
            // shows its end. An unresolved time stays unresolved.
            let end = play.animation.end_time();
            let local = match progress {
                Some(p) if p.is_nan() => p,
                Some(p) if end.is_finite() => p * end,
                Some(_) => play.animation.delay.max(0.0),
                None => play.hold.unwrap_or(0.0),
            };
            // The source owns its time: nothing waits for a frame.
            play.pending = None;
            moved |= play.hold.replace(local).map(f64::to_bits) != Some(local.to_bits());
        }
        self.animating.remove(&node);
        if !moved {
            return;
        }
        // A lowering host's executor seeks its own copy (the web's glue); only
        // what the engine samples is painted from here.
        let properties: Vec<Property> = self.animations[&node]
            .iter()
            .flat_map(|play| play.animation.keyframes.properties())
            .filter(|p| !self.lowered_for(node, *p))
            .collect();
        for property in properties {
            self.dirty.insert((node, property));
        }
    }
}
