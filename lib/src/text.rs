use crate::attrs::YrsAttrs;
use crate::delta::YrsDelta;
use crate::doc::YrsCollectionPtr;
use crate::error::{guard, parse_json, YrsError};
use crate::subscription::{next_key, SendBox, YSubscription};
use crate::transaction::YrsTransaction;
use std::fmt::Debug;
use std::sync::Arc;
use yrs::branch::Branch;
use yrs::{GetString, Observable, Text, TextRef};

/// A `Y.Text`. Offsets are UTF-16 code units (the doc's `OffsetKind`), as in Yjs.
pub(crate) struct YrsText(TextRef);

unsafe impl Send for YrsText {}
unsafe impl Sync for YrsText {}

impl AsRef<Branch> for YrsText {
    fn as_ref(&self) -> &Branch {
        self.0.as_ref()
    }
}

impl From<TextRef> for YrsText {
    fn from(value: TextRef) -> Self {
        YrsText(value)
    }
}

pub(crate) trait YrsTextObservationDelegate: Send + Sync + Debug {
    fn call(&self, value: Vec<YrsDelta>);
}

impl YrsText {
    pub(crate) fn raw_ptr(&self) -> YrsCollectionPtr {
        YrsCollectionPtr::from(self.0.as_ref())
    }

    pub(crate) fn format(&self, tx: &YrsTransaction, index: u32, length: u32, attrs: String) -> Result<(), YrsError> {
        let attrs = YrsAttrs::parse(&attrs)?;
        tx.with(|txn| {
            self.0.format(txn, index, length, attrs.0);
            Ok(())
        })
    }

    pub(crate) fn append(&self, tx: &YrsTransaction, text: String) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.push(txn, text.as_str());
            Ok(())
        })
    }

    pub(crate) fn insert(&self, tx: &YrsTransaction, index: u32, chunk: String) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.insert(txn, index, chunk.as_str());
            Ok(())
        })
    }

    pub(crate) fn insert_with_attributes(&self, tx: &YrsTransaction, index: u32, chunk: String, attrs: String) -> Result<(), YrsError> {
        let attrs = YrsAttrs::parse(&attrs)?;
        tx.with(|txn| {
            self.0.insert_with_attributes(txn, index, chunk.as_str(), attrs.0);
            Ok(())
        })
    }

    pub(crate) fn insert_embed(&self, tx: &YrsTransaction, index: u32, content: String) -> Result<(), YrsError> {
        let content = parse_json(&content)?;
        tx.with(|txn| {
            self.0.insert_embed(txn, index, content);
            Ok(())
        })
    }

    pub(crate) fn insert_embed_with_attributes(&self, tx: &YrsTransaction, index: u32, content: String, attrs: String) -> Result<(), YrsError> {
        let content = parse_json(&content)?;
        let attrs = YrsAttrs::parse(&attrs)?;
        tx.with(|txn| {
            self.0.insert_embed_with_attributes(txn, index, content, attrs.0);
            Ok(())
        })
    }

    pub(crate) fn get_string(&self, tx: &YrsTransaction) -> Result<String, YrsError> {
        tx.with(|txn| Ok(self.0.get_string(txn)))
    }

    pub(crate) fn remove_range(&self, tx: &YrsTransaction, start: u32, length: u32) -> Result<(), YrsError> {
        tx.with(|txn| {
            self.0.remove_range(txn, start, length);
            Ok(())
        })
    }

    pub(crate) fn length(&self, tx: &YrsTransaction) -> Result<u32, YrsError> {
        tx.with(|txn| Ok(self.0.len(txn)))
    }

    pub(crate) fn observe(&self, delegate: Box<dyn YrsTextObservationDelegate>) -> Result<Arc<YSubscription>, YrsError> {
        guard(|| {
            let key = next_key();
            self.0.observe(key.clone(), move |txn, event| {
                delegate.call(event.delta(txn).iter().map(YrsDelta::from).collect())
            });
            let text = SendBox(self.0.clone());
            Ok(Arc::new(YSubscription::new(move || {
                text.0.unobserve(key);
            })))
        })
    }
}
