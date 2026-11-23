use crate::array::YrsArray;
use crate::error::{guard, YrsError};
use crate::map::YrsMap;
use crate::subscription::{next_key, SendBox, YSubscription};
use crate::text::YrsText;
use crate::transaction::YrsTransaction;
use crate::undo::YrsUndoManager;
use std::fmt::Debug;
use std::sync::Arc;
use yrs::branch::Branch;
use yrs::{
    updates::decoder::Decode, Doc, OffsetKind, Options, Origin, StateVector, Transact,
    TransactionMut, WriteTxn, ReadTxn,
};

pub(crate) struct YrsDoc(Doc);

unsafe impl Send for YrsDoc {}
unsafe impl Sync for YrsDoc {}

/// Receives every committed update (v1) of the document, with the
/// transaction's origin. Runs during commit: don't open a transaction on the
/// same document from inside it.
pub(crate) trait YrsDocUpdateDelegate: Send + Sync + Debug {
    fn call(&self, update: Vec<u8>, origin: Option<YrsOrigin>);
}

impl YrsDoc {
    pub(crate) fn new() -> Self {
        let mut options = Options::default();
        options.offset_kind = OffsetKind::Utf16;
        Self(Doc::with_options(options))
    }

    /// An exclusive transaction for a one-off call, or `TransactionBusy`
    /// (yrs 0.28 would block forever instead).
    fn exclusive(&self) -> Result<TransactionMut<'_>, YrsError> {
        self.0.try_transact_mut().map_err(|_| YrsError::TransactionBusy)
    }

    pub(crate) fn encode_diff_v1(
        &self,
        transaction: &YrsTransaction,
        state_vector: Vec<u8>,
    ) -> Result<Vec<u8>, YrsError> {
        transaction.with(|txn| {
            let sv = StateVector::decode_v1(&state_vector).map_err(|_| YrsError::DecodingError)?;
            Ok(txn.encode_diff_v1(&sv))
        })
    }

    pub(crate) fn get_text(&self, name: String) -> Result<Arc<YrsText>, YrsError> {
        guard(|| Ok(Arc::new(YrsText::from(self.exclusive()?.get_or_insert_text(name.as_str())))))
    }

    pub(crate) fn get_array(&self, name: String) -> Result<Arc<YrsArray>, YrsError> {
        guard(|| Ok(Arc::new(YrsArray::from(self.exclusive()?.get_or_insert_array(name.as_str())))))
    }

    pub(crate) fn get_map(&self, name: String) -> Result<Arc<YrsMap>, YrsError> {
        guard(|| Ok(Arc::new(YrsMap::from(self.exclusive()?.get_or_insert_map(name.as_str())))))
    }

    pub(crate) fn transact(&self, origin: Option<YrsOrigin>) -> Result<Arc<YrsTransaction>, YrsError> {
        guard(|| {
            let txn = match origin {
                Some(origin) => self.0.try_transact_mut_with(origin),
                None => self.0.try_transact_mut(),
            }
            .map_err(|_| YrsError::TransactionBusy)?;
            Ok(Arc::new(YrsTransaction::from(txn)))
        })
    }

    pub(crate) fn undo_manager(
        &self,
        tracked_refs: Vec<YrsCollectionPtr>,
    ) -> Result<Arc<YrsUndoManager>, YrsError> {
        guard(|| {
            let mut manager = yrs::undo::UndoManager::with_options(Default::default());
            for tracked in tracked_refs {
                manager.expand_scope(&self.0, &tracked);
            }
            Ok(Arc::new(YrsUndoManager::new(self.0.clone(), manager)))
        })
    }

    /// Calls `delegate` with each committed update (local or applied remote).
    pub(crate) fn observe_update_v1(
        &self,
        delegate: Box<dyn YrsDocUpdateDelegate>,
    ) -> Result<Arc<YSubscription>, YrsError> {
        guard(|| {
            let key = next_key();
            self.0
                .observe_update_v1(key.clone(), move |txn, event| {
                    delegate.call(event.update.clone(), txn.origin().cloned().map(YrsOrigin::from))
                })
                .map_err(|_| YrsError::TransactionBusy)?;
            let doc = SendBox(self.0.clone());
            Ok(Arc::new(YSubscription::new(move || {
                let _ = doc.0.unobserve_update_v1(key);
            })))
        })
    }
}

#[derive(Clone)]
pub(crate) struct YrsOrigin(Arc<[u8]>);

impl From<Origin> for YrsOrigin {
    fn from(value: Origin) -> Self {
        YrsOrigin(Arc::from(value.as_ref()))
    }
}

impl From<YrsOrigin> for Origin {
    fn from(value: YrsOrigin) -> Origin {
        Origin::from(value.0.as_ref())
    }
}

uniffi::custom_type!(YrsOrigin, Vec<u8>, {
    lower: |origin| origin.0.to_vec(),
    try_lift: |bytes| Ok(YrsOrigin(bytes.into())),
});

/// A shared type's branch address, used to scope undo managers.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct YrsCollectionPtr(*const Branch);

unsafe impl Send for YrsCollectionPtr {}
unsafe impl Sync for YrsCollectionPtr {}

impl AsRef<Branch> for YrsCollectionPtr {
    #[inline]
    fn as_ref(&self) -> &Branch {
        unsafe { self.0.as_ref() }.unwrap()
    }
}

impl<'a> From<&'a Branch> for YrsCollectionPtr {
    #[inline]
    fn from(value: &'a Branch) -> Self {
        YrsCollectionPtr(value as *const Branch)
    }
}

uniffi::custom_type!(YrsCollectionPtr, u64, {
    lower: |ptr| ptr.0 as usize as u64,
    try_lift: |raw| Ok(YrsCollectionPtr(raw as usize as *const Branch)),
});
