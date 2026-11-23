use crate::doc::{YrsCollectionPtr, YrsOrigin};
use crate::error::{guard, YrsError};
use crate::subscription::{next_key, SendBox, YSubscription};
use std::fmt::Debug;
use std::sync::{Arc, Mutex, MutexGuard};
use yrs::undo::{EventKind, UndoManager};
use yrs::{Doc, Transact};

type Manager = UndoManager<u64>;

/// Tracks changes to a set of shared types; `undo`/`redo` revert or re-apply
/// them. Fails with `TransactionBusy` while a transaction is open on the doc
/// (yrs 0.28 would block forever instead).
pub(crate) struct YrsUndoManager {
    doc: Doc,
    manager: Arc<Mutex<Manager>>,
}

unsafe impl Send for YrsUndoManager {}
unsafe impl Sync for YrsUndoManager {}

impl YrsUndoManager {
    pub(crate) fn new(doc: Doc, manager: Manager) -> Self {
        YrsUndoManager { doc, manager: Arc::new(Mutex::new(manager)) }
    }

    fn lock(&self) -> MutexGuard<'_, Manager> {
        self.manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Fails fast instead of deadlocking if a transaction is open.
    fn ensure_idle(&self) -> Result<(), YrsError> {
        self.doc.try_transact_mut().map(drop).map_err(|_| YrsError::TransactionBusy)
    }

    pub(crate) fn add_origin(&self, origin: YrsOrigin) -> Result<(), YrsError> {
        guard(|| Ok(self.lock().include_origin(origin)))
    }

    pub(crate) fn remove_origin(&self, origin: YrsOrigin) -> Result<(), YrsError> {
        guard(|| Ok(self.lock().exclude_origin(origin)))
    }

    pub(crate) fn add_scope(&self, tracked_ref: YrsCollectionPtr) -> Result<(), YrsError> {
        guard(|| Ok(self.lock().expand_scope(&self.doc, &tracked_ref)))
    }

    pub(crate) fn undo(&self) -> Result<bool, YrsError> {
        guard(|| {
            self.ensure_idle()?;
            Ok(self.lock().undo_blocking())
        })
    }

    pub(crate) fn redo(&self) -> Result<bool, YrsError> {
        guard(|| {
            self.ensure_idle()?;
            Ok(self.lock().redo_blocking())
        })
    }

    pub(crate) fn clear(&self) -> Result<(), YrsError> {
        guard(|| Ok(self.lock().clear_all()))
    }

    pub(crate) fn wrap_changes(&self) -> Result<(), YrsError> {
        guard(|| Ok(self.lock().reset()))
    }

    pub(crate) fn observe_added(&self, delegate: Box<dyn YrsUndoManagerObservationDelegate>) -> Result<Arc<YSubscription>, YrsError> {
        self.subscribe(delegate, |m, key, f| m.observe_item_added(key, f), |m, key| { m.unobserve_item_added(key); })
    }

    pub(crate) fn observe_updated(&self, delegate: Box<dyn YrsUndoManagerObservationDelegate>) -> Result<Arc<YSubscription>, YrsError> {
        self.subscribe(delegate, |m, key, f| m.observe_item_updated(key, f), |m, key| { m.unobserve_item_updated(key); })
    }

    pub(crate) fn observe_popped(&self, delegate: Box<dyn YrsUndoManagerObservationDelegate>) -> Result<Arc<YSubscription>, YrsError> {
        self.subscribe(delegate, |m, key, f| m.observe_item_popped(key, f), |m, key| { m.unobserve_item_popped(key); })
    }

    fn subscribe(
        &self,
        delegate: Box<dyn YrsUndoManagerObservationDelegate>,
        observe: impl FnOnce(&mut Manager, yrs::Origin, Box<dyn FnMut(&yrs::TransactionMut, &mut yrs::undo::Event<u64>) + Send + Sync>),
        unobserve: impl FnOnce(&mut Manager, yrs::Origin) + Send + 'static,
    ) -> Result<Arc<YSubscription>, YrsError> {
        guard(|| {
            let key = next_key();
            observe(&mut self.lock(), key.clone(), Box::new(move |_, e| {
                let meta = *e.meta();
                *e.meta_mut() = delegate.call(YrsUndoEvent::new(e), meta);
            }));
            let manager = SendBox(self.manager.clone());
            Ok(Arc::new(YSubscription::new(move || {
                let manager = manager; // capture the whole SendBox, not just its field
                let mut m = manager.0.lock().unwrap_or_else(|p| p.into_inner());
                unobserve(&mut m, key);
            })))
        })
    }
}

pub(crate) trait YrsUndoManagerObservationDelegate: Send + Sync + Debug {
    fn call(&self, e: Arc<YrsUndoEvent>, ptr: u64) -> u64;
}

/// Snapshot of an undo event, valid after the callback returns.
pub(crate) struct YrsUndoEvent {
    origin: Option<YrsOrigin>,
    kind: EventKind,
    changed: Vec<usize>,
}

impl YrsUndoEvent {
    fn new(event: &yrs::undo::Event<u64>) -> Arc<Self> {
        Arc::new(YrsUndoEvent {
            origin: event.origin().cloned().map(YrsOrigin::from),
            kind: event.kind(),
            changed: event.changed_parent_types().iter().map(|b| AsRef::<yrs::branch::Branch>::as_ref(b) as *const _ as usize).collect(),
        })
    }

    pub(crate) fn origin(&self) -> Option<YrsOrigin> {
        self.origin.clone()
    }

    pub(crate) fn kind(&self) -> YrsUndoEventKind {
        match self.kind {
            EventKind::Undo => YrsUndoEventKind::Undo,
            EventKind::Redo => YrsUndoEventKind::Redo,
        }
    }

    pub(crate) fn has_changed(&self, shared_ref: YrsCollectionPtr) -> bool {
        let ptr = AsRef::<yrs::branch::Branch>::as_ref(&shared_ref) as *const _ as usize;
        self.changed.contains(&ptr)
    }
}

#[repr(u8)]
pub(crate) enum YrsUndoEventKind {
    Undo,
    Redo,
}
