//! A list's gap and snapping, read with its other options
//! ([`Collection::create`]), and its row estimate when computed.
use super::*;

/// What a list reads of its spacing: `row-gap` and `column-gap`
/// ([vertical, horizontal]), and whether its `scroll-snap-type` is mandatory.
#[derive(Default)]
pub(super) struct Spacing {
    gaps: [f64; 2],
    pub(super) snaps: bool,
}

impl Spacing {
    /// Read one of the list's bindings, when it is one of these.
    pub(super) fn read(
        &mut self,
        binding: &exact_plan::BindingsRow,
        u: &mut Update<'_>,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        use exact_kernel::StyleId;
        if binding.kind != BindingKind::Style {
            return Ok(());
        }
        let slot = match binding.id {
            id if id == StyleId::RowGap as u16 => 0,
            id if id == StyleId::ColumnGap as u16 => 1,
            id if id == StyleId::ScrollSnapType as u16 => {
                self.snaps = matches!(
                    u.eval(binding.expr, frames)?.as_str(),
                    Some("x mandatory" | "y mandatory")
                );
                return Ok(());
            }
            _ => return Ok(()),
        };
        if let Value::Number(n) = u.eval(binding.expr, frames)? {
            if n.is_finite() && n > 0.0 {
                self.gaps[slot] = n;
            }
        }
        Ok(())
    }

    /// The gap along the list's axis: a vertical list is a block, where CSS
    /// lays out no gap.
    pub(super) fn gap(&self, axis: ListAxis) -> f64 {
        match axis {
            ListAxis::Horizontal => self.gaps[1],
            ListAxis::Vertical => 0.0,
        }
    }
}

impl Collection {
    /// The list's estimate changed (a computed `estimated-item-width`): rows
    /// take it until measured again, so the offsets past them follow. A
    /// value that is not a positive number changes nothing.
    pub(super) fn reestimate(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        value: Option<&Value>,
    ) -> Result<(), InstanceError> {
        let Some(&Value::Number(n)) = value else {
            return Ok(());
        };
        let estimate = n + self.gap;
        if !n.is_finite() || n <= 0.0 || estimate == self.estimated_height {
            return Ok(());
        }
        self.end_preview(u)?;
        // A vertical list keeps what shows, as CSS anchors a block, and its
        // mounted rows' heights with it. A row list keeps its offset, as a
        // browser does on x, and every card takes the estimate until it is
        // measured again: a card the window lets go first is not left at
        // its old width.
        let (anchor, kept): (_, Vec<usize>) = match self.axis {
            ListAxis::Vertical => (
                self.anchor()?,
                self.mounted.iter().map(|m| m.position).collect(),
            ),
            ListAxis::Horizontal => (None, Vec::new()),
        };
        self.index
            .reestimate(estimate, &kept)
            .map_err(index_error)?;
        self.estimated_height = estimate;
        self.zero_heights
            .retain(|key| self.index.position(key).is_some_and(|p| kept.contains(&p)));
        self.restore_unless_unknown(anchor)?;
        self.realize_window(u, frames, false, CollectionFill::default())?;
        advance(&mut self.revision)?;
        Ok(())
    }
}
