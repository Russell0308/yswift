use crate::error::out_to_json;
use yrs::types::EntryChange;

pub struct YrsMapChange {
    pub key: String,
    pub change: YrsEntryChange,
}

pub enum YrsEntryChange {
    Inserted { value: String },
    Updated { old_value: String, new_value: String },
    Removed { value: String },
}

// Shared-type values are passed as "".
impl From<&EntryChange> for YrsEntryChange {
    fn from(item: &EntryChange) -> Self {
        let json = |v| out_to_json(v).unwrap_or_default();
        match item {
            EntryChange::Inserted(v) => YrsEntryChange::Inserted { value: json(v) },
            EntryChange::Updated(old, new) => YrsEntryChange::Updated { old_value: json(old), new_value: json(new) },
            EntryChange::Removed(v) => YrsEntryChange::Removed { value: json(v) },
        }
    }
}
