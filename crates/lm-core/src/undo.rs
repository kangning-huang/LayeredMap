//! The undo core (WP2.10, WP3.8): a stack of committed transactions. One user action is one
//! step, a slider gesture is one step, and the stack keeps the last [`UNDO_DEPTH`] steps.
//! Selection and camera moves are never steps (they are not document changes).

use crate::document::{ApplyError, Document, Op, Transaction};

/// How many steps Undo can go back (WP3.8).
pub const UNDO_DEPTH: usize = 100;

/// Undo and redo stacks of transactions; the last element is the most recent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UndoStack {
    pub undo: Vec<Transaction>,
    pub redo: Vec<Transaction>,
    /// The gesture the last step belongs to while it is still going (a slider being dragged):
    /// further commits with the same key fold into that step.
    gesture: Option<String>,
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
        self.gesture = None;
        self.push(tx);
        Ok(())
    }

    fn push(&mut self, tx: Transaction) {
        self.undo.push(tx);
        self.redo.clear();
        if self.undo.len() > UNDO_DEPTH {
            let extra = self.undo.len() - UNDO_DEPTH;
            self.undo.drain(..extra);
        }
    }

    /// Commits one moment of a continuous gesture (dragging the opacity slider). The first
    /// commit with a `key` makes a step; later ones with the same key, until
    /// [`UndoStack::end_gesture`] or any other commit, fold into it, so the whole drag undoes
    /// in one step back to where it began. Ops on the same subject merge (first `from`, last
    /// `to`), so a long drag keeps one op, not thousands.
    pub fn commit_gesture(&mut self, doc: &mut Document, tx: Transaction, key: &str) -> Result<(), ApplyError> {
        if tx.ops.is_empty() {
            return Ok(());
        }
        if self.gesture.as_deref() != Some(key) || self.undo.is_empty() {
            doc.apply(&tx)?;
            self.push(tx);
            self.gesture = Some(key.to_string());
            return Ok(());
        }
        doc.apply(&tx)?;
        if let Some(last) = self.undo.last_mut() {
            for op in tx.ops {
                merge_op(&mut last.ops, op);
            }
        }
        Ok(())
    }

    /// Ends the current gesture: the next commit makes a new step.
    pub fn end_gesture(&mut self) {
        self.gesture = None;
    }

    /// Every transaction held (undo and redo), for keeping the data they refer to.
    pub fn transactions(&self) -> impl Iterator<Item = &Transaction> {
        self.undo.iter().chain(self.redo.iter())
    }

    /// Reverses the most recent transaction; returns its label. None when there is nothing to
    /// undo, or when the document no longer accepts the reversal (stacks left unchanged).
    pub fn undo(&mut self, doc: &mut Document) -> Option<String> {
        self.gesture = None;
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
        self.gesture = None;
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

/// Folds `op` into a gesture's ops: a change to the same subject keeps the first `from` and
/// takes the new `to`; anything else is appended.
fn merge_op(ops: &mut Vec<Op>, op: Op) {
    for existing in ops.iter_mut() {
        match (existing, &op) {
            (Op::SetStyle { id: a, to, .. }, Op::SetStyle { id: b, to: new, .. }) if a == b => {
                *to = new.clone();
                return;
            }
            (Op::SetFilter { id: a, to, .. }, Op::SetFilter { id: b, to: new, .. }) if a == b => {
                *to = new.clone();
                return;
            }
            (Op::SetTime { id: a, to, .. }, Op::SetTime { id: b, to: new, .. }) if a == b => {
                *to = *new;
                return;
            }
            _ => {}
        }
    }
    ops.push(op);
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

    #[test]
    fn a_slider_drag_is_one_step() {
        let mut d = sample();
        let start = d.clone();
        let mut u = UndoStack::new();
        let id = d.layers[0].id;
        for i in 0..1800 {
            let mut s = d.layers[0].style.clone();
            s.opacity = 1.0 - (i as f32) / 2000.0;
            let tx = d.set_style_tx(id, s).unwrap();
            u.commit_gesture(&mut d, tx, "opacity").unwrap();
        }
        u.end_gesture();
        assert_eq!(u.undo.len(), 1);
        assert_eq!(u.undo[0].ops.len(), 1, "merged into one op");
        let end = d.clone();
        u.undo(&mut d);
        assert_eq!(d, start);
        u.redo(&mut d);
        assert_eq!(d, end);
    }

    #[test]
    fn a_new_gesture_or_commit_starts_a_new_step() {
        let mut d = sample();
        let mut u = UndoStack::new();
        let id = d.layers[0].id;
        let mut s = d.layers[0].style.clone();
        s.opacity = 0.5;
        let tx = d.set_style_tx(id, s.clone()).unwrap();
        u.commit_gesture(&mut d, tx, "opacity").unwrap();
        let tx = d.rename_tx(id, "x").unwrap();
        u.commit(&mut d, tx).unwrap();
        s.opacity = 0.4;
        let tx = d.set_style_tx(id, s).unwrap();
        u.commit_gesture(&mut d, tx, "opacity").unwrap();
        assert_eq!(u.undo.len(), 3);
    }

    #[test]
    fn depth_is_bounded() {
        let mut d = sample();
        let mut u = UndoStack::new();
        let id = d.layers[0].id;
        for i in 0..1000 {
            let tx = d.rename_tx(id, format!("n{i}")).unwrap();
            u.commit(&mut d, tx).unwrap();
        }
        assert_eq!(u.undo.len(), UNDO_DEPTH);
        for _ in 0..UNDO_DEPTH {
            assert!(u.undo(&mut d).is_some());
        }
        assert_eq!(d.layers[0].display_name, "n899");
        assert!(u.undo(&mut d).is_none());
    }

    #[test]
    fn filters_roles_and_stars_round_trip() {
        use crate::filter::{Condition, Filter, Test};
        use crate::model::{Confidence, DataType, FieldProfile, FieldRole, Known};
        let mut d = sample();
        let id = d.layers[0].id;
        for n in ["a", "b", "c"] {
            d.layers[0].profile.fields.push(FieldProfile {
                name: n.into(),
                dtype: DataType::Float,
                role: Known::likely(FieldRole::Measure(crate::model::MeasureKind::Unknown), ""),
                unit: Known::unknown(None, ""),
                stats: None,
            });
        }
        let start = d.clone();
        let mut u = UndoStack::new();
        let f = Filter { conditions: vec![Condition { field: "a".into(), test: Test::Above(1.0) }] };
        let tx = d.set_filter_tx(id, Some(f)).unwrap();
        u.commit(&mut d, tx).unwrap();
        let tx = d.set_role_tx(id, "a", Some(FieldRole::Identifier)).unwrap();
        u.commit(&mut d, tx).unwrap();
        for n in ["a", "b", "c"] {
            let tx = d.set_starred_tx(id, n, true).unwrap();
            u.commit(&mut d, tx).unwrap();
        }
        let tx = d.set_starred_tx(id, "a", false).unwrap();
        u.commit(&mut d, tx).unwrap();
        assert_eq!(d.layers[0].starred, ["b", "c"]);
        assert_eq!(d.layers[0].role(&d.layers[0].profile.fields[0]).confidence, Confidence::Certain);
        let end = d.clone();
        u.undo(&mut d);
        assert_eq!(d.layers[0].starred, ["a", "b", "c"], "unstar undone puts it back in place");
        while u.undo(&mut d).is_some() {}
        assert_eq!(d, start);
        while u.redo(&mut d).is_some() {}
        assert_eq!(d, end);
    }

    /// `sample()` plus the id counter advanced past the 12 undone layers (ids are never reused).
    fn sample_with_ids_used(d: &Document) -> Document {
        let mut s = sample();
        s.next_layer_id = d.next_layer_id;
        s
    }
}
