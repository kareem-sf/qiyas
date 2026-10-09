//! Undo and redo for the take-off, the BOQ, the conflicts and the drawing itself, as whole
//! snapshots: small enough at take-off sizes. A step keeps only the part its command changed, so
//! editing the take-off never copies a long BOQ. A drawing snapshot shares the entities with the
//! drawing (they are reference-counted), so it costs the tables and indexes, not the geometry.
//! Each step remembers who made it, so the AI can undo only its own last change.

use opencadcodec::CadDocument;

use crate::{Boq, Conflict, Schedule, Source, Takeoff};

/// Most undo steps kept; the oldest is dropped first.
const MAX_STEPS: usize = 200;

/// The part of the session a command changed, as it was before (or after, on the redo stack).
#[derive(Debug)]
pub(crate) enum Snapshot {
    Takeoff(Takeoff),
    Boq(Option<Boq>),
    Conflicts(Vec<Conflict>, Vec<Schedule>),
    /// The drawing (PDF import, underlays, calibration).
    Document(Box<CadDocument>),
}

/// The parts of the session undo and redo put back.
pub(crate) struct Parts<'a> {
    pub takeoff: &'a mut Takeoff,
    pub boq: &'a mut Option<Boq>,
    pub conflicts: &'a mut Vec<Conflict>,
    pub schedules: &'a mut Vec<Schedule>,
    pub doc: &'a mut CadDocument,
    /// Set when the drawing was put back, so the session redraws it.
    pub doc_changed: &'a mut bool,
}

impl Snapshot {
    /// Puts this snapshot in place and returns what it replaced.
    fn swap(self, parts: Parts) -> Snapshot {
        match self {
            Snapshot::Takeoff(t) => Snapshot::Takeoff(std::mem::replace(parts.takeoff, t)),
            Snapshot::Boq(b) => Snapshot::Boq(std::mem::replace(parts.boq, b)),
            Snapshot::Conflicts(c, s) => Snapshot::Conflicts(
                std::mem::replace(parts.conflicts, c),
                std::mem::replace(parts.schedules, s),
            ),
            Snapshot::Document(d) => {
                *parts.doc_changed = true;
                Snapshot::Document(Box::new(std::mem::replace(parts.doc, *d)))
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct History {
    /// The state before each change, and who made the change.
    undo: Vec<(Snapshot, Source)>,
    redo: Vec<(Snapshot, Source)>,
}

impl History {
    /// Records the state as it was before a command by `source`. Clears what could be redone.
    pub(crate) fn record(&mut self, before: Snapshot, source: Source) {
        self.redo.clear();
        push_capped(&mut self.undo, (before, source));
    }

    /// Who made the change that `undo` would take back.
    pub(crate) fn top_source(&self) -> Option<Source> {
        self.undo.last().map(|(_, source)| *source)
    }

    /// Puts the take-off, BOQ or conflicts back to their state before the last command. False
    /// when there is none.
    pub(crate) fn undo(&mut self, parts: Parts) -> bool {
        let Some((before, source)) = self.undo.pop() else {
            return false;
        };
        self.redo.push((before.swap(parts), source));
        true
    }

    /// Redoes the last undone command. False when there is none.
    pub(crate) fn redo(&mut self, parts: Parts) -> bool {
        let Some((after, source)) = self.redo.pop() else {
            return false;
        };
        push_capped(&mut self.undo, (after.swap(parts), source));
        true
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn push_capped(stack: &mut Vec<(Snapshot, Source)>, step: (Snapshot, Source)) {
    if stack.len() == MAX_STEPS {
        stack.remove(0);
    }
    stack.push(step);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_at_most_200_steps() {
        let mut history = History::default();
        let (mut takeoff, mut boq, mut conflicts, mut schedules) =
            (Takeoff::default(), None, Vec::new(), Vec::new());
        let (mut doc, mut doc_changed) = (CadDocument::new(), false);
        for _ in 0..MAX_STEPS + 5 {
            history.record(Snapshot::Takeoff(Takeoff::default()), Source::User);
        }
        let undone = std::iter::from_fn(|| {
            let parts = Parts {
                takeoff: &mut takeoff,
                boq: &mut boq,
                conflicts: &mut conflicts,
                schedules: &mut schedules,
                doc: &mut doc,
                doc_changed: &mut doc_changed,
            };
            history.undo(parts).then_some(())
        })
        .count();
        assert_eq!(undone, MAX_STEPS);
    }
}
