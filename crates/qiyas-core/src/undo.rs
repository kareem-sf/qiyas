//! Undo and redo for the take-off and the BOQ, as whole snapshots: small enough at take-off
//! sizes. A step keeps only the part its command changed, so editing the take-off never copies
//! a long BOQ. Each step remembers who made it, so the AI can undo only its own last change.

use crate::{Boq, Source, Takeoff};

/// Most undo steps kept; the oldest is dropped first.
const MAX_STEPS: usize = 200;

/// The part of the session a command changed, as it was before (or after, on the redo stack).
#[derive(Debug)]
pub(crate) enum Snapshot {
    Takeoff(Takeoff),
    Boq(Option<Boq>),
}

impl Snapshot {
    /// Puts this snapshot in place and returns what it replaced.
    fn swap(self, takeoff: &mut Takeoff, boq: &mut Option<Boq>) -> Snapshot {
        match self {
            Snapshot::Takeoff(t) => Snapshot::Takeoff(std::mem::replace(takeoff, t)),
            Snapshot::Boq(b) => Snapshot::Boq(std::mem::replace(boq, b)),
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

    /// Puts the take-off or BOQ back to its state before the last command. False when there is
    /// none.
    pub(crate) fn undo(&mut self, takeoff: &mut Takeoff, boq: &mut Option<Boq>) -> bool {
        let Some((before, source)) = self.undo.pop() else {
            return false;
        };
        self.redo.push((before.swap(takeoff, boq), source));
        true
    }

    /// Redoes the last undone command. False when there is none.
    pub(crate) fn redo(&mut self, takeoff: &mut Takeoff, boq: &mut Option<Boq>) -> bool {
        let Some((after, source)) = self.redo.pop() else {
            return false;
        };
        push_capped(&mut self.undo, (after.swap(takeoff, boq), source));
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
        let mut takeoff = Takeoff::default();
        let mut boq = None;
        for _ in 0..MAX_STEPS + 5 {
            history.record(Snapshot::Takeoff(Takeoff::default()), Source::User);
        }
        let undone =
            std::iter::from_fn(|| history.undo(&mut takeoff, &mut boq).then_some(())).count();
        assert_eq!(undone, MAX_STEPS);
    }
}
