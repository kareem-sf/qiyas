//! Undo and redo for the take-off, as whole snapshots: small enough at take-off sizes. Each step
//! remembers who made it, so the AI can undo only its own last change.

use crate::{Source, Takeoff};

/// Most undo steps kept; the oldest is dropped first.
const MAX_STEPS: usize = 200;

#[derive(Debug, Default)]
pub(crate) struct History {
    /// The take-off before each change, and who made the change.
    undo: Vec<(Takeoff, Source)>,
    redo: Vec<(Takeoff, Source)>,
}

impl History {
    /// Records the take-off as it was before a command by `source`. Clears what could be redone.
    pub(crate) fn record(&mut self, before: Takeoff, source: Source) {
        self.redo.clear();
        push_capped(&mut self.undo, (before, source));
    }

    /// Who made the change that `undo` would take back.
    pub(crate) fn top_source(&self) -> Option<Source> {
        self.undo.last().map(|(_, source)| *source)
    }

    /// Puts `current` back to its state before the last command. False when there is none.
    pub(crate) fn undo(&mut self, current: &mut Takeoff) -> bool {
        let Some((before, source)) = self.undo.pop() else {
            return false;
        };
        self.redo.push((std::mem::replace(current, before), source));
        true
    }

    /// Redoes the last undone command. False when there is none.
    pub(crate) fn redo(&mut self, current: &mut Takeoff) -> bool {
        let Some((after, source)) = self.redo.pop() else {
            return false;
        };
        push_capped(&mut self.undo, (std::mem::replace(current, after), source));
        true
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn push_capped(stack: &mut Vec<(Takeoff, Source)>, step: (Takeoff, Source)) {
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
        let mut current = Takeoff::default();
        for _ in 0..MAX_STEPS + 5 {
            history.record(Takeoff::default(), Source::User);
        }
        let undone = std::iter::from_fn(|| history.undo(&mut current).then_some(())).count();
        assert_eq!(undone, MAX_STEPS);
    }
}
