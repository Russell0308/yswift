use crate::change::YrsChange;
use crate::doc::YrsCollectionPtr;
use crate::error::{guard, out_to_json, parse_json, YrsError};
use crate::subscription::{next_key, SendBox, YSubscription};
use crate::transaction::YrsTransaction;
use std::fmt::Debug;
use std::sync::Arc;
use yrs::branch::Branch;
use yrs::{Any, Array, ArrayRef, Observable};

/// A `Y.Array`. Values cross the FFI as JSON strings.
pub(crate) struct YrsArray(ArrayRef);

unsafe impl Send for YrsArray {}
unsafe impl Sync for YrsArray {}

impl AsRef<Branch> for YrsArray {
    fn as_ref(&self) -> &Branch {
        self.0.as_ref()
    }
}

impl From<ArrayRef> for YrsArray {
    fn from(value: ArrayRef) -> Self {
        YrsArray(value)
    }
}

pub(crate) trait YrsArrayEachDelegate: Send + Sync + Debug {
    fn call(&self, value: String);
}

pub(crate) trait YrsArrayObservationDelegate: Send + Sync + Debug {
    fn call(&self, value: Vec<YrsChange>);
}

impl YrsArray {
    pub(crate) fn raw_ptr(&self) -> YrsCollectionPtr {
        YrsCollectionPtr::from(self.0.as_ref())
    }

    /// Shared-type elements are passed as an empty string.
    pub(crate) fn each(&self, tx: &YrsTransaction, delegate: Box<dyn YrsArrayEachDelegate>) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.iter(txn).for_each(|value| delegate.call(out_to_json(&value).unwrap_or_default()));
            Ok(())
        })
    }

    pub(crate) fn get(&self, tx: &YrsTransaction, index: u32) -> Result<String, YrsError> {
        tx.with(|txn| {
            let value = self.0.get(txn, index).ok_or(YrsError::NotFound)?;
            out_to_json(&value).ok_or(YrsError::EncodingError)
        })
    }

    pub(crate) fn insert(&self, tx: &YrsTransaction, index: u32, value: String) -> Result<(), YrsError> {
        let value = parse_json(&value)?;
        tx.with(|txn| {
            self.0.insert(txn, index, value);
            Ok(())
        })
    }

    pub(crate) fn insert_range(&self, tx: &YrsTransaction, index: u32, values: Vec<String>) -> Result<(), YrsError> {
        let values = values.iter().map(|v| parse_json(v)).collect::<Result<Vec<Any>, _>>()?;
        tx.with(|txn| {
            self.0.insert_range(txn, index, values);
            Ok(())
        })
    }

    pub(crate) fn length(&self, tx: &YrsTransaction) -> Result<u32, YrsError> {
        tx.with(|txn| Ok(self.0.len(txn)))
    }

    pub(crate) fn push_back(&self, tx: &YrsTransaction, value: String) -> Result<(), YrsError> {
        let value = parse_json(&value)?;
        tx.with(|txn| {
            self.0.push_back(txn, value);
            Ok(())
        })
    }

    pub(crate) fn push_front(&self, tx: &YrsTransaction, value: String) -> Result<(), YrsError> {
        let value = parse_json(&value)?;
        tx.with(|txn| {
            self.0.push_front(txn, value);
            Ok(())
        })
    }

    pub(crate) fn remove(&self, tx: &YrsTransaction, index: u32) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.remove(txn, index);
            Ok(())
        })
    }

    pub(crate) fn remove_range(&self, tx: &YrsTransaction, index: u32, len: u32) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.remove_range(txn, index, len);
            Ok(())
        })
    }

    pub(crate) fn observe(&self, delegate: Box<dyn YrsArrayObservationDelegate>) -> Result<Arc<YSubscription>, YrsError> {
        guard(|| {
            let key = next_key();
            self.0.observe(key.clone(), move |txn, event| {
                delegate.call(event.delta(txn).iter().map(YrsChange::from).collect())
            });
            let array = SendBox(self.0.clone());
            Ok(Arc::new(YSubscription::new(move || {
                array.0.unobserve(key);
            })))
        })
    }

    /// Plain values only; shared-type elements are skipped.
    pub(crate) fn to_a(&self, tx: &YrsTransaction) -> Result<Vec<String>, YrsError> {
        tx.with(|txn| Ok(self.0.iter(txn).filter_map(|v| out_to_json(&v)).collect()))
    }
}
