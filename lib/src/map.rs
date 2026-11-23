use crate::doc::YrsCollectionPtr;
use crate::error::{guard, out_to_json, parse_json, YrsError};
use crate::mapchange::{YrsEntryChange, YrsMapChange};
use crate::subscription::{next_key, SendBox, YSubscription};
use crate::transaction::YrsTransaction;
use std::fmt::Debug;
use std::sync::Arc;
use yrs::branch::Branch;
use yrs::{Map, MapRef, Observable};

/// A `Y.Map`. Values cross the FFI as JSON strings.
pub(crate) struct YrsMap(MapRef);

unsafe impl Send for YrsMap {}
unsafe impl Sync for YrsMap {}

impl AsRef<Branch> for YrsMap {
    fn as_ref(&self) -> &Branch {
        self.0.as_ref()
    }
}

impl From<MapRef> for YrsMap {
    fn from(value: MapRef) -> Self {
        YrsMap(value)
    }
}

pub(crate) trait YrsMapIteratorDelegate: Send + Sync + Debug {
    fn call(&self, value: String);
}

pub(crate) trait YrsMapKVIteratorDelegate: Send + Sync + Debug {
    fn call(&self, key: String, value: String);
}

pub(crate) trait YrsMapObservationDelegate: Send + Sync + Debug {
    fn call(&self, value: Vec<YrsMapChange>);
}

impl YrsMap {
    pub(crate) fn raw_ptr(&self) -> YrsCollectionPtr {
        YrsCollectionPtr::from(self.0.as_ref())
    }

    /// Inserts a JSON value; bad JSON is `InvalidJson`, not a crash.
    pub(crate) fn insert(&self, tx: &YrsTransaction, key: String, value: String) -> Result<(), YrsError> {
        let value = parse_json(&value)?;
        tx.with(|txn| {
            self.0.insert(txn, key, value);
            Ok(())
        })
    }

    pub(crate) fn length(&self, tx: &YrsTransaction) -> Result<u32, YrsError> {
        tx.with(|txn| Ok(self.0.len(txn)))
    }

    pub(crate) fn contains_key(&self, tx: &YrsTransaction, key: String) -> Result<bool, YrsError> {
        tx.with(|txn| Ok(self.0.contains_key(txn, key.as_str())))
    }

    /// The JSON value at `key`; `NotFound` if absent, `EncodingError` if it's a shared type.
    pub(crate) fn get(&self, tx: &YrsTransaction, key: String) -> Result<String, YrsError> {
        tx.with(|txn| {
            let value = self.0.get(txn, key.as_str()).ok_or(YrsError::NotFound)?;
            out_to_json(&value).ok_or(YrsError::EncodingError)
        })
    }

    pub(crate) fn remove(&self, tx: &YrsTransaction, key: String) -> Result<Option<String>, YrsError> {
        tx.with(|txn| match self.0.remove(txn, key.as_str()) {
            Some(value) => out_to_json(&value).map(Some).ok_or(YrsError::EncodingError),
            None => Ok(None),
        })
    }

    pub(crate) fn clear(&self, tx: &YrsTransaction) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.clear(txn);
            Ok(())
        })
    }

    pub(crate) fn keys(&self, tx: &YrsTransaction, delegate: Box<dyn YrsMapIteratorDelegate>) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.keys(txn).for_each(|key| delegate.call(key.to_string()));
            Ok(())
        })
    }

    /// Shared-type values are passed as an empty string.
    pub(crate) fn values(&self, tx: &YrsTransaction, delegate: Box<dyn YrsMapIteratorDelegate>) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0
                .values(txn)
                .flatten()
                .for_each(|value| delegate.call(out_to_json(&value).unwrap_or_default()));
            Ok(())
        })
    }

    /// Shared-type values are passed as an empty string.
    pub(crate) fn each(&self, tx: &YrsTransaction, delegate: Box<dyn YrsMapKVIteratorDelegate>) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0
                .iter(txn)
                .for_each(|(key, value)| delegate.call(key.to_string(), out_to_json(&value).unwrap_or_default()));
            Ok(())
        })
    }

    pub(crate) fn observe(&self, delegate: Box<dyn YrsMapObservationDelegate>) -> Result<Arc<YSubscription>, YrsError> {
        guard(|| {
            let key = next_key();
            self.0.observe(key.clone(), move |txn, event| {
                let changes = event
                    .keys(txn)
                    .iter()
                    .map(|(key, change)| YrsMapChange { key: key.to_string(), change: YrsEntryChange::from(change) })
                    .collect();
                delegate.call(changes)
            });
            let map = SendBox(self.0.clone());
            Ok(Arc::new(YSubscription::new(move || {
                map.0.unobserve(key);
            })))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::doc::YrsDoc;

    #[test]
    fn verify_new_map_has_zero_count() {
        let doc = YrsDoc::new();
        let map = doc.get_map("example_map".to_string()).unwrap();
        let txn = doc.transact(None).unwrap();
        assert_eq!(map.length(&txn).unwrap(), 0);
    }

    #[test]
    fn map_insert_and_count() {
        let doc = YrsDoc::new();
        let map = doc.get_map("example_map".to_string()).unwrap();
        let key = "AB123".to_string();
        let txn = doc.transact(None).unwrap();
        assert!(!map.contains_key(&txn, key.clone()).unwrap());
        map.insert(&txn, key.clone(), "\"Hello\"".to_string()).unwrap();
        assert_eq!(map.length(&txn).unwrap(), 1);
        assert!(map.contains_key(&txn, key).unwrap());
    }

    #[test]
    fn map_insert_and_get() {
        let doc = YrsDoc::new();
        let map = doc.get_map("example_map".to_string()).unwrap();
        let key = "AB123".to_string();
        let value = "\"Hello\"".to_string();
        let txn = doc.transact(None).unwrap();
        map.insert(&txn, key.clone(), value.clone()).unwrap();
        assert_eq!(map.get(&txn, key).unwrap(), value);
    }

    #[test]
    fn map_remove() {
        let doc = YrsDoc::new();
        let map = doc.get_map("example_map".to_string()).unwrap();
        let key = "AB123".to_string();
        let value = "\"Hello\"".to_string();
        let txn = doc.transact(None).unwrap();
        map.insert(&txn, key.clone(), value.clone()).unwrap();
        assert_eq!(map.remove(&txn, key).unwrap(), Some(value));
        assert_eq!(map.length(&txn).unwrap(), 0);
    }

    #[test]
    fn map_clear() {
        let doc = YrsDoc::new();
        let map = doc.get_map("example_map".to_string()).unwrap();
        let txn = doc.transact(None).unwrap();
        map.insert(&txn, "AB123".to_string(), "\"Hello\"".to_string()).unwrap();
        assert_eq!(map.length(&txn).unwrap(), 1);
        map.clear(&txn).unwrap();
        assert_eq!(map.length(&txn).unwrap(), 0);
    }
}
