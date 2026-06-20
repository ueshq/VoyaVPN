use std::sync::{Arc, Mutex};

use thiserror::Error;
use uuid::Uuid;
use voya_platform::elevation::{SudoPasswordError, SudoPasswordStore};
use zeroize::Zeroizing;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SudoCollectionStatus {
    Ready,
    Required { request_id: String },
}

#[derive(Debug)]
pub struct SudoPasswordCollector {
    store: Arc<SudoPasswordStore>,
    pending_request_id: Mutex<Option<String>>,
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
        let request_id = match pending.as_ref() {
            Some(request_id) => request_id.clone(),
            None => {
                let request_id = new_request_id();
                *pending = Some(request_id.clone());
                request_id
            }
        };

        Ok(SudoCollectionStatus::Required { request_id })
    }

    pub fn submit_password(
        &self,
        request_id: &str,
        password: Zeroizing<String>,
    ) -> Result<SudoCollectionStatus, SudoCollectionError> {
        if password.is_empty() {
            return Err(SudoCollectionError::EmptyPassword);
        }

        let mut pending = self
            .pending_request_id
            .lock()
            .map_err(|_| SudoCollectionError::LockPoisoned)?;

        if pending
            .as_ref()
            .is_some_and(|pending_request_id| pending_request_id == request_id)
        {
            self.store.set_password_secret(password)?;
            *pending = None;
            Ok(SudoCollectionStatus::Ready)
        } else if pending.is_some() {
            Err(SudoCollectionError::RequestMismatch)
        } else if self.store.has_password()? {
            Ok(SudoCollectionStatus::Ready)
        } else {
            Err(SudoCollectionError::NoPendingRequest)
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

fn new_request_id() -> String {
    Uuid::new_v4().to_string()
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
                .submit_password(&request_id, Zeroizing::new("pw".to_string()))
                .expect("submit password"),
            SudoCollectionStatus::Ready
        );
        assert_eq!(
            collector.begin_collection().expect("ready after password"),
            SudoCollectionStatus::Ready
        );
    }

    #[test]
    fn sudo_collection_request_ids_are_random_v4_uuids() {
        let collector = SudoPasswordCollector::new(Arc::new(SudoPasswordStore::new()));

        let first = required_request_id(collector.begin_collection().expect("first request"));
        let first_uuid = Uuid::parse_str(&first).expect("first request id is uuid");
        assert_eq!(first_uuid.get_version(), Some(uuid::Version::Random));
        assert!(first.parse::<u64>().is_err());

        collector.clear_password().expect("clear pending request");
        let second = required_request_id(collector.begin_collection().expect("second request"));
        let second_uuid = Uuid::parse_str(&second).expect("second request id is uuid");
        assert_eq!(second_uuid.get_version(), Some(uuid::Version::Random));

        assert_ne!(first, second);
    }

    fn required_request_id(status: SudoCollectionStatus) -> String {
        let SudoCollectionStatus::Required { request_id } = status else {
            panic!("expected sudo password request");
        };
        request_id
    }
}
