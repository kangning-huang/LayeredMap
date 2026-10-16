//! The undo core (WP2.10): a stack of committed transactions. One user action is one step.
//! Gesture merging and the depth limit come in Phase 3 (WP3.8).

use crate::document::{ApplyError, Document, Transaction};

/// Undo and redo stacks of transactions; the last element is the most recent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UndoStack {
    pub undo: Vec<Transaction>,
    pub redo: Vec<Transaction>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `tx` to the document and records it. Clears redo. A transaction with no ops
    /// changes nothing and is not recorded. On error the document and stacks are unchanged.
    pub fn commit(&mut self, doc: &mut Document, tx: Transaction) -> Result<(), ApplyError> {
        if tx.ops.is_empty() {
            return Ok(());
        }
        doc.apply(&tx)?;
        self.undo.push(tx);
        self.redo.clear();
        Ok(())
    }

    /// Reverses the most recent transaction; returns its label. None when there is nothing to
    /// undo, or when the document no longer accepts the reversal (stacks left unchanged).
    pub fn undo(&mut self, doc: &mut Document) -> Option<String> {
        let tx = self.undo.pop()?;
        if doc.apply(&tx.inverse()).is_err() {
            self.undo.push(tx);
            return None;
        }
        let label = tx.label.clone();
        self.redo.push(tx);
        Some(label)
    }

    /// Re-applies the most recently undone transaction; returns its label.
    pub fn redo(&mut self, doc: &mut Document) -> Option<String> {
        let tx = self.redo.pop()?;
        if doc.apply(&tx).is_err() {
            self.redo.push(tx);
            return None;
        }
        let label = tx.label.clone();
        self.undo.push(tx);
        Some(label)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The label of the step Undo would reverse ("Rename layer"), for the tooltip.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|t| t.label.as_str())
    }

    /// The label of the step Redo would re-apply.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|t| t.label.as_str())
    }

    /// The labels of the last `n` steps, most recent first (the Undo caret menu shows 10).
    pub fn recent(&self, n: usize) -> Vec<&str> {
        self.undo.iter().rev().take(n).map(|t| t.label.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Op;
    use crate::document::tests::{profile, sample};
    use crate::model::{LayerId, LayerKind};

    #[test]
    fn undo_and_redo_round_trip() {
        let mut d = sample();
        let start = d.clone();
        let mut u = UndoStack::new();
        let id = d.layers[0].id;
        let tx = d.rename_tx(id, "Elevation").unwrap();
        u.commit(&mut d, tx).unwrap();
        let renamed = d.clone();
        let tx = d.set_visible_tx(id, false).unwrap();
        u.commit(&mut d, tx).unwrap();
        let hidden = d.clone();
        assert_eq!(u.undo_label(), Some("Hide layer"));
        assert_eq!(u.recent(10), ["Hide layer", "Rename layer"]);

        assert_eq!(u.undo(&mut d).as_deref(), Some("Hide layer"));
        assert_eq!(d, renamed);
        assert_eq!(u.redo_label(), Some("Hide layer"));
        assert_eq!(u.undo(&mut d).as_deref(), Some("Rename layer"));
        assert_eq!(d, start);
        assert_eq!(u.undo(&mut d), None);
        assert!(!u.can_undo());

        assert_eq!(u.redo(&mut d).as_deref(), Some("Rename layer"));
        assert_eq!(u.redo(&mut d).as_deref(), Some("Hide layer"));
        assert_eq!(d, hidden);
        assert_eq!(u.redo(&mut d), None);
    }

    #[test]
    fn multi_op_transactions_undo_atomically() {
        let mut d = sample();
        let start = d.clone();
        let mut u = UndoStack::new();
        let a = d.layers[0].id;
        let b = d.layers[1].id;
        let tx = Transaction::new(
            "Tidy layers",
            vec![
                Op::Rename { id: a, from: "dem".into(), to: "Elevation".into() },
                Op::SetVisible { id: b, from: true, to: false },
                Op::SetUserOrdered { from: false, to: true },
                Op::Move { from_index: 0, to_index: 2 },
            ],
        );
        u.commit(&mut d, tx).unwrap();
        assert_eq!(d.layers[2].display_name, "Elevation");
        assert_eq!(u.undo(&mut d).as_deref(), Some("Tidy layers"));
        assert_eq!(d, start, "one undo reverses every op of the transaction");
    }

    #[test]
    fn a_commit_clears_redo() {
        let mut d = sample();
        let mut u = UndoStack::new();
        let id = d.layers[0].id;
        let tx = d.rename_tx(id, "A").unwrap();
        u.commit(&mut d, tx).unwrap();
        u.undo(&mut d);
        assert!(u.can_redo());
        let tx = d.rename_tx(id, "B").unwrap();
        u.commit(&mut d, tx).unwrap();
        assert!(!u.can_redo());
    }

    #[test]
    fn failed_and_empty_commits_are_not_recorded() {
        let mut d = sample();
        let before = d.clone();
        let mut u = UndoStack::new();
        let bad = Transaction::new("Bad", vec![Op::SetVisible { id: LayerId(99), from: true, to: false }]);
        assert!(u.commit(&mut d, bad).is_err());
        u.commit(&mut d, Transaction::new("Nothing", vec![])).unwrap();
        assert!(!u.can_undo());
        assert_eq!(d, before);
    }

    #[test]
    fn recent_lists_at_most_n_newest_first() {
        let mut d = sample();
        let mut u = UndoStack::new();
        for i in 0..12 {
            let tx = d.add_layer_tx(profile(&format!("p{i}.csv"), LayerKind::Points));
            u.commit(&mut d, Transaction::new(format!("Add {i}"), tx.ops)).unwrap();
        }
        let r = u.recent(10);
        assert_eq!(r.len(), 10);
        assert_eq!(r[0], "Add 11");
        assert_eq!(r[9], "Add 2");
        for _ in 0..12 {
            u.undo(&mut d);
        }
        assert_eq!(d, sample_with_ids_used(&d));
    }

    /// `sample()` plus the id counter advanced past the 12 undone layers (ids are never reused).
    fn sample_with_ids_used(d: &Document) -> Document {
        let mut s = sample();
        s.next_layer_id = d.next_layer_id;
        s
    }
}
