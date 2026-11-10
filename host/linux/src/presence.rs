//! Exit animation and layout transition on Linux (LLP 1063).
//!
//! A node with a `-exact-layout-transition` has its laid-out box in its parent
//! (`Kernel::layout_box`) observed as `Property::Layout` after every layout;
//! the engine's transition rules apply. What it shows against the laid-out
//! box is an offset, which the painter applies outermost, and a size
//! difference, which only its surface and the clip on its children take, from
//! the box's top-left corner (`Presented::layout`), so a moving parent carries
//! its children and nothing is laid out per frame. A node that gains the row is
//! seeded with the box it had before the commit's layout, so its first move
//! after gaining it animates.
//!
//! A receipt's exits name the one node of each removed subtree that leaves
//! (the kernel's rule). Its motion node is spared from the commit's removals
//! and plays its `-exact-exit-animation` from the commit's clock, sampled by
//! the engine even where a reader plays the node's other animations; the
//! painter draws the node's last recorded drawing under its parent with the
//! sampled values (`paint/exit.rs`) until the engine passes the exit's end,
//! or the parent goes.
//!
//! Shared-element flights (LLP 1013.000) are refused for a like reason in
//! stage 1: an arriver appears in place, and the journal says so once.

use super::*;
use exact_kernel::motion::LayoutMotion;
use exact_kernel::CommitReceipt;

/// A node leaving with its exit.
#[derive(Debug)]
struct Exiting {
    key: NodeKey,
    parent: NodeKey,
    animations: exact_motion::Animations,
    played: bool,
    /// The engine time its exit ends, once known (a pending exit has none).
    end: Option<f64>,
}

/// What the host keeps for layout transitions.
#[derive(Debug, Default)]
pub(super) struct Presence {
    /// Nodes declaring a layout transition.
    pub(super) layout: LayoutMotion,
    /// The nodes leaving with their exits.
    leaving: Vec<Exiting>,
    /// Whether the refusal of shared-element flights is.
    refused_flights: bool,
    /// A resize lays out next: positions are taken, not animated.
    pub(super) snap: bool,
    /// The next `observe_layout` is for a host's fact, not an app's commit
    /// (an inset, the text size, a picture's size): a box it finds moved
    /// takes its place unless it is moving already, as on the web, where a
    /// layout transition plays only what a commit moved.
    pub(super) unauthored: bool,
}

impl<D: DataSource> Host<D> {
    /// Which nodes declare a layout transition, after a commit and before
    /// its layout; and the refusal, the first time a node leaves with an exit.
    pub(super) fn track_presence(&mut self, receipts: &[Timed]) {
        for t in receipts {
            if !t.receipt.handoffs.is_empty() && !self.presence.refused_flights {
                self.presence.refused_flights = true;
                self.log("shared element: flights are not drawn on Linux (LLP 1013.000); an arriver appears in place");
            }
            let retired =
                self.presence
                    .layout
                    .seed(self.runner.kernel(), &t.receipt, &mut self.engine);
            self.retire_layout(retired);
        }
    }

    /// A receipt's exits, before its destroys are handled; an exit whose
    /// parent this commit destroyed ends.
    pub(super) fn begin_exits(&mut self, r: &CommitReceipt) {
        let (ended, kept) = std::mem::take(&mut self.presence.leaving)
            .into_iter()
            .partition(|l| r.destroyed.contains(&l.parent));
        self.presence.leaving = kept;
        for l in ended {
            self.end_exit(l);
        }
        for exit in &r.exits {
            self.presence.leaving.push(Exiting {
                key: exit.key,
                parent: exit.parent,
                animations: exit.animations.clone(),
                played: false,
                end: None,
            });
        }
    }

    /// Take the leaving nodes out of a commit's removals, so the engine
    /// keeps them; they go back once applied, for what else reads them.
    pub(super) fn spare_exits(&self, sync: &mut MotionSync) -> Vec<u64> {
        let leaving = |n: &u64| (self.presence.leaving.iter()).any(|l| motion_node(l.key) == *n);
        let spared = sync.removed.iter().copied().filter(leaving).collect();
        sync.removed.retain(|n| !leaving(n));
        spared
    }

    /// Start every exit the engine has not heard, at its clock, sampled
    /// by the engine: a reader's layer for the node is gone with it.
    pub(super) fn play_exits(&mut self) {
        for l in &mut self.presence.leaving {
            if !l.played {
                let node = motion_node(l.key);
                self.engine.set_node_sampled(node, true);
                let played = self.engine.play_exit(node, &l.animations);
                debug_assert!(played.is_ok(), "the kernel validated the row");
                l.played = true;
            }
        }
    }

    /// A presentation the engine reported for a leaving node: its parent
    /// paints again. Whether it was one.
    pub(super) fn exit_presented(&mut self, key: NodeKey) -> bool {
        let Some(parent) = self.presence.leaving.iter().find(|l| l.key == key) else {
            return false;
        };
        self.row_dirty.node(parent.parent);
        true
    }

    /// End the exits the engine's clock has passed; whether any did.
    pub(super) fn end_exits(&mut self) -> bool {
        let now = self.engine.now();
        for l in &mut self.presence.leaving {
            if l.played && l.end.is_none() {
                l.end = (self.engine).exit_end(motion_node(l.key), l.animations.0.len());
            }
        }
        let (ended, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.presence.leaving)
            .into_iter()
            .partition(|l| l.end.is_some_and(|end| end <= now));
        self.presence.leaving = kept;
        let any = !ended.is_empty();
        for l in ended {
            self.end_exit(l);
        }
        any
    }

    fn end_exit(&mut self, l: Exiting) {
        self.engine.remove(motion_node(l.key));
        self.row_dirty.node(l.parent);
    }

    /// Whether a node is leaving now.
    pub(super) fn leaving(&self) -> bool {
        !self.presence.leaving.is_empty()
    }

    /// The leaving nodes as the engine samples them now, for the painter.
    pub(crate) fn leaving_paint(&self) -> Vec<crate::paint::Leaving> {
        let sampled = |l: &Exiting, p: Property| {
            l.animations
                .properties()
                .contains(&p)
                .then(|| self.engine.sampled_value(motion_node(l.key), p))
                .flatten()
        };
        (self.presence.leaving.iter())
            .filter(|l| l.played)
            .map(|l| crate::paint::Leaving {
                key: l.key,
                parent: l.parent,
                opacity: sampled(l, Property::Opacity).map(|v| v.x as f32),
                translate: sampled(l, Property::Translate)
                    .map(|v| ((v.x as f32, v.y as f32), (v.z as f32, v.w as f32))),
                scale: sampled(l, Property::Scale).map(|v| v.x as f32),
                rotate: sampled(l, Property::Rotate).map(|v| v.x as f32),
            })
            .collect()
    }

    /// An agent's jump to `to`, stopping after a timer that sends
    /// ([`exact_runner::Runner::advance_until_request`]). The engine hears
    /// each commit of a jump at its timer's due time (LLP 1002 D3), but the
    /// kernel holds only the jump's last tree, laid out at its end: a box a
    /// timer moved inside the jump would start its layout transition there.
    /// While a node declares a layout transition, the jump lays out at each
    /// due time, as real frames would; only a request stops it short of `to`,
    /// so these stops spend none of the agent's request stops.
    pub(super) fn advance_in_layout_steps(&mut self, to: f64) -> Option<String> {
        if self.presence.layout.is_empty() {
            let a = self.runner.advance_until_request(to);
            return self.advanced(a).0;
        }
        let mut error = None;
        for _ in 0..LAYOUT_STOPS {
            let due = self.runner.timer_due_ms().unwrap_or(to);
            let step = due.max(self.runner.now_ms()).min(to);
            let a = self.runner.advance_until_request(step);
            // A commit that sent stops the jump: the next send would drop
            // its reply (LLP 1016 D5).
            let stopped = a.error.is_some() || a.stopped_on_request || step >= to;
            error = error.or(self.advanced(a).0);
            if stopped || self.presence.layout.is_empty() {
                break;
            }
        }
        error
    }

    /// Lay out again for a host's fact and observe it: a box it moved takes
    /// its place unless it is moving already.
    pub(crate) fn layout_unauthored(&mut self) -> Result<bool, String> {
        self.presence.unauthored = true;
        let laid = self.layout();
        self.observe_layout();
        laid
    }

    /// Observe every tracked node's box, after a layout.
    pub(super) fn observe_layout(&mut self) {
        let snap = std::mem::take(&mut self.presence.snap);
        let unauthored = std::mem::take(&mut self.presence.unauthored);
        let (kernel, layout) = (self.runner.kernel(), &mut self.presence.layout);
        let retired = if unauthored && !snap {
            layout.observe_all_unauthored(kernel, &mut self.engine)
        } else {
            layout.observe_all(kernel, &mut self.engine, snap)
        };
        self.retire_layout(retired);
    }

    fn retire_layout(&mut self, retired: Vec<NodeKey>) {
        for key in retired {
            if let Some(p) = self.keys.get(&key).and_then(|v| self.presented.get_mut(v)) {
                p.layout = Presented::IDENTITY.layout;
            }
        }
    }
}

/// The most layout stops one agent call makes: a frame task's every frame
/// across ten minutes. The agent's loop calls again past it.
const LAYOUT_STOPS: usize = 60 * 60 * 10;
