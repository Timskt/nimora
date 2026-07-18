use nimora_persistence_sqlite::{
    BackupCoordinator, BackupHealth, BackupRecord, SqlitePersistenceError,
};
use std::sync::Mutex;

pub(crate) struct BackupService<'a> {
    coordinator: &'a BackupCoordinator,
    last_error: &'a Mutex<Option<String>>,
}

impl<'a> BackupService<'a> {
    pub(crate) fn new(
        coordinator: &'a BackupCoordinator,
        last_error: &'a Mutex<Option<String>>,
    ) -> Self {
        Self {
            coordinator,
            last_error,
        }
    }

    pub(crate) fn health(&self) -> Result<BackupHealth, SqlitePersistenceError> {
        let mut health = self.coordinator.health()?;
        let last_error = self
            .last_error
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        health.last_error.clone_from(&last_error);
        Ok(health)
    }

    pub(crate) fn create_now(&self) -> Result<BackupRecord, SqlitePersistenceError> {
        match self.coordinator.create_now() {
            Ok(record) => {
                self.clear_error()?;
                Ok(record)
            }
            Err(error) => {
                self.record_error(&error);
                Err(error)
            }
        }
    }

    pub(crate) fn create_if_due(&self) -> Result<Option<BackupRecord>, SqlitePersistenceError> {
        match self.coordinator.create_if_due() {
            Ok(record) => {
                self.clear_error()?;
                Ok(record)
            }
            Err(error) => {
                self.record_error(&error);
                Err(error)
            }
        }
    }

    pub(crate) fn request_restore(&self, backup_id: &str) -> Result<(), SqlitePersistenceError> {
        self.coordinator.request_restore(backup_id)
    }

    fn clear_error(&self) -> Result<(), SqlitePersistenceError> {
        *self
            .last_error
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)? = None;
        Ok(())
    }

    fn record_error(&self, error: &SqlitePersistenceError) {
        if let Ok(mut last_error) = self.last_error.lock() {
            *last_error = Some(error.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_persistence_sqlite::{BackupPolicy, SqlitePetRepository};

    fn fixture() -> (std::path::PathBuf, BackupCoordinator, Mutex<Option<String>>) {
        let root =
            std::env::temp_dir().join(format!("nimora-backup-service-{}", uuid::Uuid::now_v7()));
        let blocked_database = root.join("database.sqlite3");
        std::fs::create_dir_all(&blocked_database).expect("blocked database fixture");
        let coordinator = BackupCoordinator::new(
            blocked_database,
            root.join("backups"),
            BackupPolicy::default(),
        );
        (root, coordinator, Mutex::new(None))
    }

    #[test]
    fn health_projects_the_shared_last_error_without_paths() {
        let (root, coordinator, last_error) = fixture();
        *last_error.lock().expect("error state") = Some("backup unavailable".to_owned());

        let health = BackupService::new(&coordinator, &last_error)
            .health()
            .expect("health projection");
        assert_eq!(health.last_error.as_deref(), Some("backup unavailable"));
        assert!(health.available.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_manual_backup_is_recorded_for_every_health_consumer() {
        let (root, coordinator, last_error) = fixture();
        let service = BackupService::new(&coordinator, &last_error);

        assert!(service.create_now().is_err());
        let health = service.health().expect("health after failure");
        assert!(health.last_error.is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_scheduled_backup_uses_the_same_error_projection() {
        let (root, coordinator, last_error) = fixture();
        let service = BackupService::new(&coordinator, &last_error);

        assert!(service.create_if_due().is_err());
        assert!(
            service
                .health()
                .expect("health after failure")
                .last_error
                .is_some()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn successful_backup_clears_a_previous_failure() {
        let root =
            std::env::temp_dir().join(format!("nimora-backup-success-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&root).expect("fixture directory");
        let database = root.join("runtime.sqlite3");
        SqlitePetRepository::open(&database).expect("database fixture");
        let coordinator =
            BackupCoordinator::new(&database, root.join("backups"), BackupPolicy::default());
        let last_error = Mutex::new(Some("previous failure".to_owned()));
        let service = BackupService::new(&coordinator, &last_error);

        service.create_now().expect("successful backup");
        assert!(
            service
                .health()
                .expect("healthy backup state")
                .last_error
                .is_none()
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }
}
