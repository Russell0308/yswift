//! FFI-boundary behaviour: failures that used to abort or deadlock the host
//! app are now errors, plus the doc-level update observer.
use crate::doc::{YrsDoc, YrsDocUpdateDelegate, YrsOrigin};
use crate::error::{guard, YrsError};
use std::sync::{Arc, Mutex};

#[test]
fn root_lookup_inside_transaction_is_busy_not_a_deadlock() {
    let doc = YrsDoc::new();
    let txn = doc.transact(None).unwrap();
    assert!(matches!(doc.get_text("content".into()), Err(YrsError::TransactionBusy)));
    assert!(matches!(doc.get_map("notes".into()), Err(YrsError::TransactionBusy)));
    assert!(matches!(doc.transact(None), Err(YrsError::TransactionBusy)));
    txn.free().unwrap();
    assert!(doc.get_text("content".into()).is_ok());
}

#[test]
fn bad_json_is_an_error() {
    let doc = YrsDoc::new();
    let map = doc.get_map("notes".into()).unwrap();
    let text = doc.get_text("content".into()).unwrap();
    let txn = doc.transact(None).unwrap();
    assert!(matches!(map.insert(&txn, "k".into(), "{not json".into()), Err(YrsError::InvalidJson { .. })));
    assert!(matches!(text.insert_embed(&txn, 0, "{".into()), Err(YrsError::InvalidJson { .. })));
    assert!(matches!(text.format(&txn, 0, 0, "nope".into()), Err(YrsError::InvalidJson { .. })));
    assert_eq!(map.length(&txn).unwrap(), 0);
}

#[test]
fn missing_key_and_freed_transaction_are_errors() {
    let doc = YrsDoc::new();
    let map = doc.get_map("notes".into()).unwrap();
    let txn = doc.transact(None).unwrap();
    assert!(matches!(map.get(&txn, "absent".into()), Err(YrsError::NotFound)));
    txn.free().unwrap();
    txn.free().unwrap(); // idempotent
    assert!(matches!(map.length(&txn), Err(YrsError::TransactionClosed)));
}

#[test]
fn bad_update_is_a_decoding_error() {
    let doc = YrsDoc::new();
    let txn = doc.transact(None).unwrap();
    assert!(matches!(txn.transaction_apply_update(vec![0xff, 0xff, 0xff]), Err(YrsError::DecodingError)));
    assert!(matches!(txn.transaction_encode_state_as_update_from_sv(vec![0xff]), Err(YrsError::DecodingError)));
}

#[test]
fn panics_become_errors() {
    let result: Result<(), YrsError> = guard(|| panic!("boom"));
    match result {
        Err(YrsError::Panic { message }) => assert_eq!(message, "boom"),
        _ => panic!("expected YrsError::Panic"),
    }
}

#[test]
fn utf16_offsets_match_yjs() {
    let doc = YrsDoc::new();
    let text = doc.get_text("content".into()).unwrap();
    let txn = doc.transact(None).unwrap();
    text.insert(&txn, 0, "héllo 👋 world".into()).unwrap();
    assert_eq!(text.length(&txn).unwrap(), 14); // JS "héllo 👋 world".length
    text.remove_range(&txn, 6, 2).unwrap(); // the emoji is 2 UTF-16 units
    assert_eq!(text.get_string(&txn).unwrap(), "héllo  world");
}

#[derive(Debug, Default)]
struct Updates(Mutex<Vec<(Vec<u8>, Option<Vec<u8>>)>>);
#[derive(Debug)]
struct Collect(Arc<Updates>);
impl YrsDocUpdateDelegate for Collect {
    fn call(&self, update: Vec<u8>, origin: Option<YrsOrigin>) {
        let origin = origin.map(|o| yrs::Origin::from(o).as_ref().to_vec());
        self.0 .0.lock().unwrap().push((update, origin));
    }
}

#[test]
fn update_observer_sees_local_and_remote_updates_until_dropped() {
    let (a, b) = (YrsDoc::new(), YrsDoc::new());
    let (text_a, text_b) = (a.get_text("content".into()).unwrap(), b.get_text("content".into()).unwrap());
    let seen = Arc::new(Updates::default());
    let subscription = b.observe_update_v1(Box::new(Collect(seen.clone()))).unwrap();

    // local edit on b, with an origin
    let txn = b.transact(Some(YrsOrigin::from(yrs::Origin::from("local")))).unwrap();
    text_b.insert(&txn, 0, "b".into()).unwrap();
    let local = txn.transaction_encode_update().unwrap();
    txn.free().unwrap();
    // remote edit from a, applied to b
    let txn = a.transact(None).unwrap();
    text_a.insert(&txn, 0, "a".into()).unwrap();
    let remote = txn.transaction_encode_update().unwrap();
    txn.free().unwrap();
    let txn = b.transact(None).unwrap();
    txn.transaction_apply_update(remote.clone()).unwrap();
    txn.free().unwrap();

    {
        let seen = seen.0.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], (local, Some(b"local".to_vec())));
        assert_eq!(seen[1].0, remote);
    }
    drop(subscription);
    let txn = b.transact(None).unwrap();
    text_b.append(&txn, "!".into()).unwrap();
    txn.free().unwrap();
    assert_eq!(seen.0.lock().unwrap().len(), 2, "no callbacks after the subscription is dropped");
}

#[test]
fn yrs_panic_out_of_range_is_an_error_and_the_transaction_survives() {
    let doc = YrsDoc::new();
    let array = doc.get_array("items".into()).unwrap();
    let txn = doc.transact(None).unwrap();
    array.push_back(&txn, "1".into()).unwrap();
    let out_of_range = array.remove_range(&txn, 5, 2); // yrs panics: index out of bounds
    assert!(matches!(out_of_range, Err(YrsError::Panic { .. })), "got {:?}", out_of_range.err());
    array.push_back(&txn, "2".into()).unwrap();
    assert_eq!(array.to_a(&txn).unwrap(), vec!["1".to_string(), "2".to_string()]);
}
