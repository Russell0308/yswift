use crate::error::out_to_json;
use yrs::types::Change;

pub enum YrsChange {
    Added { elements: Vec<String> },
    Removed { range: u32 },
    Retained { range: u32 },
}

// Added shared-type elements are skipped.
impl From<&Change> for YrsChange {
    fn from(item: &Change) -> Self {
        match item {
            Change::Added(added) => YrsChange::Added { elements: added.iter().filter_map(out_to_json).collect() },
            Change::Removed(range) => YrsChange::Removed { range: *range },
            Change::Retain(range) => YrsChange::Retained { range: *range },
        }
    }
}
