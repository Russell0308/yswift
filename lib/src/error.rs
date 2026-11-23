use std::any::Any as StdAny;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Every failure that crosses the FFI boundary. Swift sees these as thrown
/// errors; nothing in this crate is allowed to abort the host app.
#[derive(Debug, thiserror::Error)]
pub enum YrsError {
    #[error("could not encode the value")]
    EncodingError,
    #[error("could not decode the update or state vector")]
    DecodingError,
    #[error("invalid JSON: {message}")]
    InvalidJson { message: String },
    #[error("another transaction is active on this document")]
    TransactionBusy,
    #[error("the transaction was already freed")]
    TransactionClosed,
    #[error("no value at the given key or index")]
    NotFound,
    #[error("Rust panic: {message}")]
    Panic { message: String },
}

/// Runs `body`, turning a Rust panic into `YrsError::Panic` so it reaches
/// Swift as an error instead of aborting (needs `panic = "unwind"`).
pub(crate) fn guard<T>(body: impl FnOnce() -> Result<T, YrsError>) -> Result<T, YrsError> {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(result) => result,
        Err(payload) => Err(YrsError::Panic { message: panic_message(payload.as_ref()) }),
    }
}

fn panic_message(payload: &(dyn StdAny + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// Parses JSON into a yrs value without panicking on bad input.
pub(crate) fn parse_json(json: &str) -> Result<yrs::Any, YrsError> {
    yrs::Any::from_json(json).map_err(|e| YrsError::InvalidJson { message: e.to_string() })
}

/// JSON for a yrs value; shared types (text, maps, arrays) are not plain
/// values and yield `None`.
pub(crate) fn out_to_json(value: &yrs::Out) -> Option<String> {
    match value {
        yrs::Out::Any(any) => {
            let mut buf = String::new();
            any.to_json(&mut buf);
            Some(buf)
        }
        _ => None,
    }
}
