use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use voya_platform::elevation::{SudoPasswordError, SudoPasswordStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SudoCollectionStatus {
    Ready,
    Required { request_id: u64 },
}

#[derive(Debug)]
pub struct SudoPasswordCollector {
    store: Arc<SudoPasswordStore>,
    pending_request_id: Mutex<Option<u64>>,
    next_request_id: AtomicU64,
}

impl SudoPasswordCollector {
    #[must_use]
    pub fn default_store() -> Self {
        Self::new(Arc::new(SudoPasswordStore::new()))
    }

    #[must_use]
    pub fn new(store: Arc<SudoPasswordStore>) -> Self {
        Self {
            store,
            pending_request_id: Mutex::new(None),
            next_request_id: AtomicU64::new(initial_request_id()),
        }
    }

    #[must_use]
    pub fn store(&self) -> Arc<SudoPasswordStore> {
        Arc::clone(&self.store)
    }

    pub fn begin_collection(&self) -> Result<SudoCollectionStatus, SudoCollectionError> {
        if self.store.has_password()? {
            return Ok(SudoCollectionStatus::Ready);
        }

        let mut pending = self
            .pending_request_id
            .lock()
            .map_err(|_| SudoCollectionError::LockPoisoned)?;
        let request_id = match *pending {
            Some(request_id) => request_id,
            None => {
                let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
                *pending = Some(request_id);
                request_id
            }
        };

        Ok(SudoCollectionStatus::Required { request_id })
    }

    pub fn submit_password(
        &self,
        request_id: u64,
        password: String,
    ) -> Result<SudoCollectionStatus, SudoCollectionError> {
        if password.is_empty() {
            return Err(SudoCollectionError::EmptyPassword);
        }

        let mut pending = self
            .pending_request_id
            .lock()
            .map_err(|_| SudoCollectionError::LockPoisoned)?;

        match *pending {
            Some(pending_request_id) if pending_request_id == request_id => {
                self.store.set_password(password)?;
                *pending = None;
                Ok(SudoCollectionStatus::Ready)
            }
            Some(_) => Err(SudoCollectionError::RequestMismatch),
            None if self.store.has_password()? => Ok(SudoCollectionStatus::Ready),
            None => Err(SudoCollectionError::NoPendingRequest),
        }
    }

    pub fn clear_password(&self) -> Result<(), SudoCollectionError> {
        self.store.clear()?;
        let mut pending = self
            .pending_request_id
            .lock()
            .map_err(|_| SudoCollectionError::LockPoisoned)?;
        *pending = None;
        Ok(())
    }
}

fn initial_request_id() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |duration| duration.as_secs().max(1))
}

#[derive(Debug, Error)]
pub enum SudoCollectionError {
    #[error("sudo collection lock is poisoned")]
    LockPoisoned,
    #[error("sudo password cannot be empty")]
    EmptyPassword,
    #[error("sudo password request id does not match the pending request")]
    RequestMismatch,
    #[error("no sudo password request is pending")]
    NoPendingRequest,
    #[error(transparent)]
    Store(#[from] SudoPasswordError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_sudo_password_is_collected_once_when_tun_is_enabled() {
        let collector = SudoPasswordCollector::new(Arc::new(SudoPasswordStore::new()));

        let first = collector.begin_collection().expect("first request");
        let second = collector.begin_collection().expect("second request");
        assert_eq!(first, second);

        let SudoCollectionStatus::Required { request_id } = first else {
            panic!("expected sudo password request");
        };
        assert_eq!(
            collector
                .submit_password(request_id, "pw".to_string())
                .expect("submit password"),
            SudoCollectionStatus::Ready
        );
        assert_eq!(
            collector.begin_collection().expect("ready after password"),
            SudoCollectionStatus::Ready
        );
    }
}
