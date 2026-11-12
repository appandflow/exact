//! Scroll progress timelines' own grammar (Scroll-driven Animations 1 §2):
//! the name a scroller declares, and the anonymous `scroll()` an
//! `animation-timeline` may name instead.
//!
//! Exact takes one timeline per node and kind: a comma-separated list of
//! names or insets is refused, not read in part.

use crate::generated::TimelineAxis;

/// `scroll-timeline-name`: `none`, or one `<dashed-ident>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimelineName(pub Option<String>);

impl TimelineName {
    /// `none` or a `<dashed-ident>`; `None` also while unlinked.
    pub fn parse(css: &str) -> Option<Self> {
        (super::LINKED.get()?.name)(css)
    }

    pub(super) fn parse_text(css: &str) -> Option<Self> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("none") {
            return Some(Self(None));
        }
        super::dashed(t).map(|n| Self(Some(n)))
    }

    /// The declaration's value.
    pub fn css(&self) -> String {
        self.0.clone().unwrap_or_else(|| "none".into())
    }
}

/// The scroller `scroll()` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scroller {
    /// The nearest ancestor scroll container (the page's, when none).
    #[default]
    Nearest,
    /// The page's.
    Root,
    /// The consumer itself.
    Itself,
}

/// An anonymous timeline: `scroll()` in `animation-timeline`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Anonymous {
    /// `scroll(<scroller> || <axis>)`: a scroll progress timeline of the
    /// scroller named relative to the consumer.
    Scroll {
        /// Which scroller.
        scroller: Scroller,
        /// The axis it reads.
        axis: TimelineAxis,
    },
}

impl Anonymous {
    /// `scroll(…)`, either argument order, case-insensitive.
    pub(super) fn parse(css: &str) -> Option<Self> {
        let t = css.trim();
        let open = t.find('(')?;
        let args = t[open + 1..].strip_suffix(')')?;
        let function = t[..open].trim_end();
        let words: Vec<&str> = args.split_ascii_whitespace().collect();
        let axis_of = |w: &str| TimelineAxis::from_name(&w.to_ascii_lowercase());
        if function.eq_ignore_ascii_case("scroll") {
            let (mut scroller, mut axis) = (None, None);
            for w in &words {
                let named = match w.to_ascii_lowercase().as_str() {
                    "nearest" => Some(Scroller::Nearest),
                    "root" => Some(Scroller::Root),
                    "self" => Some(Scroller::Itself),
                    _ => None,
                };
                match (named, axis_of(w)) {
                    (Some(s), _) if scroller.is_none() => scroller = Some(s),
                    (None, Some(a)) if axis.is_none() => axis = Some(a),
                    _ => return None,
                }
            }
            return Some(Self::Scroll {
                scroller: scroller.unwrap_or_default(),
                axis: axis.unwrap_or_default(),
            });
        }
        None
    }

    /// The function as CSS serializes it: its arguments in order, the
    /// defaults left out.
    pub(super) fn css(&self) -> String {
        let mut words: Vec<String> = Vec::new();
        match self {
            Self::Scroll { scroller, axis } => {
                match scroller {
                    Scroller::Nearest => {}
                    Scroller::Root => words.push("root".into()),
                    Scroller::Itself => words.push("self".into()),
                }
                if *axis != TimelineAxis::Block {
                    words.push(axis.name().into());
                }
                format!("scroll({})", words.join(" "))
            }
        }
    }
}

/// Whether `axis` reads the horizontal offset. Exact lays out in a
/// horizontal writing mode, where the inline axis is `x`.
pub fn reads_x(axis: TimelineAxis) -> bool {
    matches!(axis, TimelineAxis::Inline | TimelineAxis::X)
}
