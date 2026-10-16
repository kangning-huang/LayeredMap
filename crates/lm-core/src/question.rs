//! Question budget instrumentation (Assumptions and questions): one record per answered
//! question card. Timestamp, question type, answer chosen and seconds to answer only; never
//! file names or data values.

use serde::{Deserialize, Serialize};

use crate::model::{AssumptionKind, Choice};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuestionEvent {
    /// When the card was answered, milliseconds since the Unix epoch.
    pub at_unix_ms: i64,
    pub kind: AssumptionKind,
    /// The chosen option's machine id ("wgs84", "not_sure"), without any data-derived part.
    pub answer_id: String,
    pub seconds_to_answer: f64,
}

impl QuestionEvent {
    /// Builds an event from the chosen option. Choice ids may carry data after a colon
    /// ("columns:lon,lat" names columns); only the part before the colon is kept.
    pub fn new(at_unix_ms: i64, kind: AssumptionKind, answer: &Choice, seconds_to_answer: f64) -> Self {
        let answer_id = answer.id.split(':').next().unwrap_or_default().to_string();
        let seconds_to_answer = if seconds_to_answer.is_finite() { seconds_to_answer.max(0.0) } else { 0.0 };
        QuestionEvent { at_unix_ms, kind, answer_id, seconds_to_answer }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_derived_parts_of_answer_ids_are_dropped() {
        let e = QuestionEvent::new(1, AssumptionKind::CsvCoordinateColumns, &Choice::new("columns:lon,lat", "lon/lat"), 3.5);
        assert_eq!(e.answer_id, "columns");
        let e = QuestionEvent::new(1, AssumptionKind::NoCrsLonLat, &Choice::not_sure(), f64::NAN);
        assert_eq!((e.answer_id.as_str(), e.seconds_to_answer), ("not_sure", 0.0));
    }
}
