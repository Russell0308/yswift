use crate::attrs::YrsAttrs;
use crate::error::out_to_json;
use yrs::types::Delta;

pub enum YrsDelta {
    Inserted { value: String, attrs: String },
    Deleted { index: u32 },
    Retained { index: u32, attrs: String },
}

// Inserted values that are shared types (e.g. embedded XML) are passed as "".
impl From<&Delta> for YrsDelta {
    fn from(item: &Delta) -> Self {
        let attrs = |a: &Option<Box<yrs::types::Attrs>>| String::from(YrsAttrs::from(a.as_deref().cloned().unwrap_or_default()));
        match item {
            Delta::Inserted(value, a) => YrsDelta::Inserted { value: out_to_json(value).unwrap_or_default(), attrs: attrs(a) },
            Delta::Retain(index, a) => YrsDelta::Retained { index: *index, attrs: attrs(a) },
            Delta::Deleted(index) => YrsDelta::Deleted { index: *index },
        }
    }
}
