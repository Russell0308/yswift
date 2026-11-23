use crate::array::YrsArray;
use crate::doc::YrsOrigin;
use crate::error::{guard, YrsError};
use crate::map::YrsMap;
use crate::text::YrsText;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use yrs::{
    updates::decoder::Decode, updates::encoder::Encode, ReadTxn, StateVector, TransactionMut,
    Update,
};

/// A read-write transaction. Must be `free()`d before the next one on the same doc.
pub(crate) struct YrsTransaction(Mutex<Option<TransactionMut<'static>>>);

unsafe impl Send for YrsTransaction {}
unsafe impl Sync for YrsTransaction {}

impl<'doc> From<TransactionMut<'doc>> for YrsTransaction {
    fn from(txn: TransactionMut<'doc>) -> Self {
        // The transaction keeps its own clone of the Doc (and so the store) alive.
        let txn: TransactionMut<'static> = unsafe { std::mem::transmute(txn) };
        YrsTransaction(Mutex::new(Some(txn)))
    }
}

impl YrsTransaction {
    /// The transaction slot; busy if another call (e.g. an observer) holds it.
    /// A panic in an earlier call poisons the mutex but leaves the slot usable.
    fn lock(&self) -> Result<MutexGuard<'_, Option<TransactionMut<'static>>>, YrsError> {
        match self.0.try_lock() {
            Ok(slot) => Ok(slot),
            Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => Err(YrsError::TransactionBusy),
        }
    }

    /// Runs `body` on the live transaction, inside the panic guard.
    pub(crate) fn with<T>(
        &self,
        body: impl FnOnce(&mut TransactionMut<'static>) -> Result<T, YrsError>,
    ) -> Result<T, YrsError> {
        guard(|| {
            let mut slot = self.lock()?;
            let txn = slot.as_mut().ok_or(YrsError::TransactionClosed)?;
            body(txn)
        })
    }

    pub(crate) fn origin(&self) -> Result<Option<YrsOrigin>, YrsError> {
        self.with(|txn| Ok(txn.origin().cloned().map(YrsOrigin::from)))
    }

    pub(crate) fn transaction_encode_update(&self) -> Result<Vec<u8>, YrsError> {
        self.with(|txn| Ok(txn.encode_update_v1()))
    }

    pub(crate) fn transaction_encode_state_as_update_from_sv(
        &self,
        state_vector: Vec<u8>,
    ) -> Result<Vec<u8>, YrsError> {
        self.with(|txn| {
            let sv = StateVector::decode_v1(&state_vector).map_err(|_| YrsError::DecodingError)?;
            Ok(txn.encode_state_as_update_v1(&sv))
        })
    }

    pub(crate) fn transaction_encode_state_as_update(&self) -> Result<Vec<u8>, YrsError> {
        self.with(|txn| Ok(txn.encode_state_as_update_v1(&StateVector::default())))
    }

    pub(crate) fn transaction_state_vector(&self) -> Result<Vec<u8>, YrsError> {
        self.with(|txn| Ok(txn.state_vector().encode_v1()))
    }

    pub(crate) fn transaction_apply_update(&self, update: Vec<u8>) -> Result<(), YrsError> {
        self.with(|txn| {
            let update = Update::decode_v1(&update).map_err(|_| YrsError::DecodingError)?;
            txn.apply_update(update).map_err(|_| YrsError::DecodingError)
        })
    }

    pub(crate) fn transaction_get_text(&self, name: String) -> Result<Option<Arc<YrsText>>, YrsError> {
        self.with(|txn| Ok(txn.get_text(name.as_str()).map(|t| Arc::new(YrsText::from(t)))))
    }

    pub(crate) fn transaction_get_array(&self, name: String) -> Result<Option<Arc<YrsArray>>, YrsError> {
        self.with(|txn| Ok(txn.get_array(name.as_str()).map(|a| Arc::new(YrsArray::from(a)))))
    }

    pub(crate) fn transaction_get_map(&self, name: String) -> Result<Option<Arc<YrsMap>>, YrsError> {
        self.with(|txn| Ok(txn.get_map(name.as_str()).map(|m| Arc::new(YrsMap::from(m)))))
    }

    /// Commits the transaction (observers run here) and releases the doc.
    /// Freeing twice is a no-op.
    pub(crate) fn free(&self) -> Result<(), YrsError> {
        guard(|| {
            let txn = self.lock()?.take();
            drop(txn);
            Ok(())
        })
    }
}
