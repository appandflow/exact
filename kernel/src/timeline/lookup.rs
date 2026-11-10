//! Named timeline lookup (Scroll-driven Animations 1 §4.2).
//!
//! A named timeline is referenceable by the node that declares it and its
//! descendants; `timeline-scope: <name>` on a node declares the name in
//! scope for its subtree, attached to the one descendant that declares it
//! and is not already captured by a nearer `timeline-scope`, or else to an
//! inactive timeline; `all` does that for every name below it. So a
//! consumer's lookup walks up from itself, and the first node that declares
//! the name or scopes it decides; nothing found is no timeline. Chrome 154
//! agrees case by case (the phase 3 probe), `all` aside: it parses none.
//!
//! The kernel keeps a registry of the few nodes that bear a timeline row
//! (sources, consumers, scopes), kept from each commit's receipt, and after
//! a commit re-resolves every consumer; the receipt names those whose
//! answer changed, and `motion_sync` sends them (`MotionSync::timelines`).

use super::{Axis, TimelineScope, LINKED};
use crate::arena::NodeArena;
use crate::generated::StyleProps;
use crate::id::NodeKey;
use crate::motion::{motion_node, TimelineRows};
use crate::sorted::SlotSet;
use crate::txn::CommitReceipt;
use exact_motion::NamedTimeline;

/// The linked lookup's signature: [`resolve`].
pub(super) type Resolve = fn(&mut NodeArena, &CommitReceipt) -> Vec<NodeKey>;

/// The timeline-bearing nodes: each slot whose style declares a drag or
/// view timeline, scopes a name or follows one, and what each follower's name
/// resolved to at the last commit. A page has a handful.
#[derive(Debug, Clone, Default)]
pub(crate) struct Registry {
    slots: SlotSet,
    resolved: Vec<(NodeKey, NamedTimeline)>,
}

impl Registry {
    fn resolved(&self, consumer: NodeKey) -> Option<NamedTimeline> {
        self.resolved
            .iter()
            .find(|(k, _)| *k == consumer)
            .map(|(_, t)| *t)
    }
}

impl crate::kernel::Kernel {
    /// What `consumer`'s `animation-timeline` name resolved to at the last
    /// commit; `None` for a node that follows no name, or
    /// while the lookup is unlinked.
    pub fn timeline_of(&self, consumer: NodeKey) -> Option<NamedTimeline> {
        self.arena().timelines.resolved(consumer)
    }

    /// Whether any node bears a timeline row: a source, a scope or a
    /// consumer. A host whose page seeks consumers itself (the web's glue)
    /// follows every commit while one does.
    pub fn has_timelines(&self) -> bool {
        !self.arena().timelines.slots.is_empty()
    }
}

/// After a commit, the consumers whose timeline now resolves differently,
/// leaving out any the commit created or touched (their rows reach the
/// engine anyway). Nothing while unlinked.
pub(crate) fn refresh(arena: &mut NodeArena, receipt: &CommitReceipt) -> Vec<NodeKey> {
    match LINKED.get() {
        Some(grammar) => (grammar.resolve)(arena, receipt),
        None => Vec::new(),
    }
}

fn bears(s: &StyleProps) -> bool {
    s.rare.drag_timeline.name.is_some()
        || s.rare.view_timeline.name.is_some()
        || s.rare.animation_timeline.name().is_some()
        || s.rare.timeline_scope != TimelineScope::None
}

/// [`refresh`], linked: keep the registry, then re-resolve every consumer.
/// Any change can move a scope (a node inserted, removed or moved, a row
/// set), so any commit that finds the registry in use re-resolves; each
/// lookup walks one consumer's ancestors and, at a scope, the registry's
/// declarations, never the tree.
pub(super) fn resolve(arena: &mut NodeArena, receipt: &CommitReceipt) -> Vec<NodeKey> {
    for key in &receipt.destroyed {
        arena.timelines.slots.remove(key.index);
    }
    for key in receipt.created.iter().chain(&receipt.touched) {
        let Some(slot) = arena.resolve(*key) else {
            continue;
        };
        if bears(arena.style(slot)) {
            arena.timelines.slots.insert(slot);
        } else {
            arena.timelines.slots.remove(slot);
        }
    }
    if arena.timelines.slots.is_empty() && arena.timelines.resolved.is_empty() {
        return Vec::new();
    }
    let at = &*arena;
    let resolved: Vec<(NodeKey, NamedTimeline)> = at
        .timelines
        .slots
        .iter()
        .filter_map(|slot| {
            let name = at.style(slot).rare.animation_timeline.name()?;
            Some((at.key(slot), lookup(at, slot, name)))
        })
        .collect();
    let changed = resolved
        .iter()
        .filter(|(key, timeline)| {
            at.timelines.resolved(*key) != Some(*timeline)
                && !receipt.created.contains(key)
                && receipt.touched.binary_search(key).is_err()
        })
        .map(|(key, _)| *key)
        .collect();
    arena.timelines.resolved = resolved;
    changed
}

/// What `name` resolves to for the consumer at `slot`: walking up from it,
/// the first node that declares the name (its own `-exact-drag-timeline` or
/// `view-timeline`, first) or scopes it (`timeline-scope`) decides.
fn lookup(arena: &NodeArena, consumer: u32, name: &str) -> NamedTimeline {
    let slots = &arena.timelines.slots;
    let declares = |slot: u32| declared(arena, slot, name).is_some();
    let mut at = Some(consumer);
    while let Some(slot) = at {
        if slots.contains(slot) {
            if let Some(timeline) = declared(arena, slot, name) {
                return timeline;
            }
            let scope = &arena.style(slot).rare.timeline_scope;
            if scope.scopes(name) {
                // What this scope captures: the declarations whose nearest
                // scope of the name, themselves included, is this one. One
                // is the timeline; none or several are an inactive one, but
                // `all` declares only the names below it.
                let mut captured = slots
                    .iter()
                    .filter(|&s| declares(s) && scope_of(arena, s, name) == Some(slot));
                match (captured.next(), captured.next()) {
                    (Some(source), None) => {
                        return declared(arena, source, name).unwrap_or(NamedTimeline::Inactive)
                    }
                    (None, _) if *scope == TimelineScope::All => {}
                    _ => return NamedTimeline::Inactive,
                }
            }
        }
        at = arena.parent(slot);
    }
    NamedTimeline::Missing
}

/// The timeline `slot` declares as `name`: its drag timeline, else its view
/// timeline.
fn declared(arena: &NodeArena, slot: u32, name: &str) -> Option<NamedTimeline> {
    let rare = &arena.style(slot).rare;
    let node = motion_node(arena.key(slot));
    if rare.drag_timeline.name.as_deref() == Some(name) {
        Some(NamedTimeline::Source(node))
    } else if rare.view_timeline.name.as_deref() == Some(name) {
        Some(NamedTimeline::View(node))
    } else {
        None
    }
}

/// The nearest node at or above `slot` whose `timeline-scope` names `name`.
fn scope_of(arena: &NodeArena, slot: u32, name: &str) -> Option<u32> {
    let mut at = Some(slot);
    while let Some(s) = at {
        if arena.timelines.slots.contains(s) && arena.style(s).rare.timeline_scope.scopes(name) {
            return Some(s);
        }
        at = arena.parent(s);
    }
    None
}

/// One node's timeline rows as the engine takes them: whether the timeline
/// it drives reads `x`, and what its animations follow over their range.
pub(crate) fn rows(arena: &NodeArena, slot: u32) -> TimelineRows {
    let key = arena.key(slot);
    let style = arena.style(slot);
    let source = style
        .rare
        .drag_timeline
        .name
        .as_ref()
        .map(|_| style.rare.drag_timeline.axis == Axis::X);
    // A view timeline runs over `cover`, its progress 0 to 1, whatever the
    // range says. On a drag, `normal` has no length range to map it onto:
    // unbound.
    let binding = style.rare.animation_timeline.name().and_then(|_| {
        let timeline = arena
            .timelines
            .resolved(key)
            .unwrap_or(NamedTimeline::Missing);
        if let NamedTimeline::View(_) = timeline {
            return Some((timeline, [0.0, 1.0]));
        }
        let [a, b] = style.animation_range.0?;
        Some((timeline, [a as f64, b as f64]))
    });
    (motion_node(key), source, binding)
}

impl crate::kernel::Kernel {
    /// Every view timeline from the last layout: its subject, the nearest
    /// scroll container it scrolls in, and the scroll offsets on its axis
    /// where its progress is 0 and 1. A host gives the engine the subject's
    /// progress at the scroller's offset after each layout and each scroll
    /// (`Engine::set_view_progress`). A subject with no scroll container
    /// above it has none.
    pub fn view_timelines(&self) -> Vec<ViewTimeline> {
        let arena = self.arena();
        let slots = &arena.timelines.slots;
        slots
            .iter()
            .filter_map(|slot| {
                let declared = &arena.style(slot).rare.view_timeline;
                declared.name.as_ref()?;
                let mut at = arena.parent(slot);
                let scroller = loop {
                    let s = at?;
                    if crate::kernel::sticky::scrolls(arena, s) {
                        break s;
                    }
                    at = arena.parent(s);
                };
                // Layout frames are border boxes, untransformed, as CSS
                // measures a view timeline's subject. The scroller's border
                // box stands in for its scrollport: a bordered scroller's
                // range is off by its start border.
                let (subject, port) = (arena.frame(slot), arena.frame(scroller));
                let x = declared.axis == Axis::X;
                let (start, size, port) = if x {
                    (subject.x - port.x, subject.width, port.width)
                } else {
                    (subject.y - port.y, subject.height, port.height)
                };
                Some(ViewTimeline {
                    subject: motion_node(arena.key(slot)),
                    scroller: arena.key(scroller),
                    x,
                    range: [(start - port) as f64, (start + size) as f64],
                })
            })
            .collect()
    }
}

/// A view timeline, as the last layout places it ([`crate::Kernel::view_timelines`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTimeline {
    /// The subject's engine node.
    pub subject: u64,
    /// The scroll container it scrolls in.
    pub scroller: NodeKey,
    /// Whether it follows the horizontal offset.
    pub x: bool,
    /// The scroller's offsets on that axis where the subject's progress is
    /// 0 (its start edge at the scrollport's end edge) and 1 (its end edge
    /// at the scrollport's start edge).
    pub range: [f64; 2],
}

impl ViewTimeline {
    /// The subject's progress at the scroller's offset `(left, top)`.
    pub fn progress(&self, left: f64, top: f64) -> f64 {
        let [a, b] = self.range;
        ((if self.x { left } else { top }) - a) / (b - a)
    }
}
