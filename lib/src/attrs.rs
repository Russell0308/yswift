use crate::error::{parse_json, YrsError};
use std::collections::HashMap;
use std::sync::Arc;
use yrs::types::Attrs;
use yrs::Any;

/// Text formatting attributes; JSON objects across the FFI.
pub(crate) struct YrsAttrs(pub(crate) Attrs);

impl From<Attrs> for YrsAttrs {
    fn from(value: Attrs) -> Self {
        YrsAttrs(value)
    }
}

impl YrsAttrs {
    /// A JSON object; any other JSON value means no attributes.
    pub(crate) fn parse(json: &str) -> Result<YrsAttrs, YrsError> {
        Ok(match parse_json(json)? {
            Any::Map(map) => YrsAttrs(map.iter().map(|(k, v)| (Arc::from(k.as_str()), v.clone())).collect()),
            _ => YrsAttrs(Attrs::new()),
        })
    }
}

impl From<YrsAttrs> for String {
    fn from(value: YrsAttrs) -> String {
        let map: HashMap<String, Any> = value.0.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let mut buf = String::new();
        Any::from(map).to_json(&mut buf);
        buf
    }
}
