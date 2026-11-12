//! Runtime resource rows; the compiled plan contains declarations only.
use super::*;

#[derive(Clone, Default)]
pub(crate) struct ResourceRow {
    pub(super) state: Option<ResourceState>,
    pub(super) value: Option<crate::held::Held>,
    /// Failed arguments suppress another ask until they change or refresh.
    pub(super) failed_args: Option<Vec<Value>>,
    pub(super) failed_why: Option<crate::failure::Failure>,
    pub(super) pending: bool,
    pub(super) current: bool,
    pub(super) overlay: Option<writes::OverlayCache>,
    /// Store dependence determines what the bake may keep.
    pub(super) store_reader: bool,
    /// Once randomness was drawn, even by a refused call, bake cannot keep an answer.
    pub(super) entropy_reader: bool,
    /// It read what the store holds (a secret, a file, a database, an
    /// argument derived from a reader), not only randomness.
    pub(super) contents_reader: bool,
    /// Topics watched by this answer; an announcement asks this row again.
    pub(super) watching: Vec<String>,
    /// A stand-in waiting for its source to become ready.
    pub(super) stale: bool,
    /// No answer yet because the source is not ready: the resource reads pending.
    pub(super) awaiting: bool,
    pub(super) member: Option<each::Member>,
    pub(super) inspection: crate::perf::ResourceWork,
}

impl<D: DataSource> Runner<D> {
    /// Runtime rows include list members; declaration walks use the plan.
    pub(super) fn resource_row(&self, i: usize) -> &exact_plan::ResourcesRow {
        if let Some(member) = &self.resources[i].member {
            &member.row
        } else {
            &self.plan.resources[i]
        }
    }

    pub(super) fn resource_rows(&self) -> impl Iterator<Item = &exact_plan::ResourcesRow> {
        (0..self.resources.len()).map(|i| self.resource_row(i))
    }

    /// Whether a change to the store asks resource `i` again: it read the
    /// store, and not only by drawing randomness, since a draw asks nothing
    /// again (LLP 1069.005 D2). A reader the plan or a carry marked, not yet
    /// asked in this run, counts as one that read.
    pub(super) fn asks_on_store_change(&self, i: usize) -> bool {
        let row = &self.resources[i];
        row.store_reader && (row.contents_reader || !row.entropy_reader)
    }

    /// What an ask of resource `i` read, from the store's counts before it:
    /// any read makes it the device's, and one beyond its draws of
    /// randomness a reader of what the store holds.
    pub(super) fn note_reads(&mut self, i: usize, reads_before: usize, draws_before: usize) {
        let reads = self.store.reads() - reads_before;
        let draws = self.store.entropy_draws() - draws_before;
        let row = &mut self.resources[i];
        row.store_reader |= reads > 0;
        row.entropy_reader |= draws > 0;
        row.contents_reader |= reads > draws;
    }
}
