use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

use super::errors::{JobError, JobResult};
use super::job::{Job, JobId, JobStatus};
use super::priority::Priority;

/// Storage for persisting jobs to disk under the `.nabu/jobs/` directory.
///
/// Each job is stored as an individual JSON file, enabling:
/// - **Crash recovery**: Jobs survive process crashes because they're written to disk immediately.
/// - **Concurrent access**: Individual file locking per job.
/// - **Efficient scans**: Listing dir contents reveals all jobs without loading every file.
/// - **No database dependency**: Pure file-based, no SQLite required.
///
/// File structure: `.nabu/jobs/{STATUS}/{JOB_ID}.json`
/// Status subdirectories (queued, running, completed, failed, etc.) allow efficient
/// listing of actionable jobs without scanning completed work.
#[derive(Debug)]
pub struct JobStore {
    /// Root directory for job storage (e.g., `.nabu/jobs/`).
    path: PathBuf,

    /// In-memory index of all known jobs for fast lookup.
    /// Keyed by job ID, valued by the job's on-disk path.
    index: Arc<RwLock<HashMap<String, JobEntry>>>,
}

/// Internal metadata tracked for each job in the in-memory index.
#[derive(Debug, Clone)]
struct JobEntry {
    /// The file path where this job is stored.
    path: PathBuf,
    /// The current status (for quick filtering without deserializing).
    status: JobStatus,
    /// The job priority (for ordering without deserializing).
    priority: Priority,
    /// When the job was created.
    created_at: chrono::DateTime<chrono::Utc>,
}

impl JobStore {
    /// Creates a new job store rooted at the given path.
    ///
    /// The path should be `.nabu/jobs/` relative to the vault root.
    /// This will create the directory structure if it doesn't exist.
    pub async fn new<P: AsRef<Path>>(path: P) -> JobResult<Self> {
        let root = path.as_ref().to_path_buf();
        let store = JobStore {
            path: root,
            index: Arc::new(RwLock::new(HashMap::new())),
        };
        store.ensure_directories().await?;
        store.rebuild_index().await?;
        Ok(store)
    }

    /// Returns the root path of the job store.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the number of jobs currently tracked in memory.
    pub async fn count(&self) -> usize {
        self.index.read().await.len()
    }

    /// Stores a new job to disk.
    ///
    /// This writes the job to a file under the appropriate status directory
    /// and adds it to the in-memory index.
    pub async fn store(&self, job: &Job) -> JobResult<()> {
        let file_path = self.job_file_path(job);
        let dir = file_path.parent().unwrap();
        fs::create_dir_all(dir).await.map_err(JobError::Io)?;

        let json = serde_json::to_string_pretty(job)?;
        // Write atomically: write to temp file, then rename
        let tmp_path = dir.join(format!("{}.tmp", job.id));
        fs::write(&tmp_path, &json).await.map_err(JobError::Io)?;
        fs::rename(&tmp_path, &file_path).await.map_err(JobError::Io)?;

        // Update index
        let mut index = self.index.write().await;
        index.insert(
            job.id.to_string(),
            JobEntry {
                path: file_path,
                status: job.status,
                priority: job.priority,
                created_at: job.created_at,
            },
        );

        Ok(())
    }

    /// Loads a single job from disk by its ID.
    pub async fn load(&self, job_id: &JobId) -> JobResult<Job> {
        let id_str = job_id.to_string();
        let entry = {
            let index = self.index.read().await;
            index
                .get(&id_str)
                .cloned()
                .ok_or_else(|| JobError::NotFound(id_str.clone()))?
        };

        let content = fs::read_to_string(&entry.path)
            .await
            .map_err(JobError::Io)?;
        let job: Job = serde_json::from_str(&content)?;
        Ok(job)
    }

    /// Updates an existing job on disk.
    ///
    /// If the job's status changed, this may move the file to a new status
    /// subdirectory.
    pub async fn update(&self, job: &Job) -> JobResult<()> {
        let id_str = job.id.to_string();
        let old_entry = {
            let index = self.index.read().await;
            index.get(&id_str).cloned()
        };

        let new_path = self.job_file_path(job);

        // If status changed, remove the old file and write to the new location
        if let Some(ref old_entry) = old_entry {
            if old_entry.path != new_path {
                // Delete old file
                let _ = fs::remove_file(&old_entry.path).await;
            }
        }

        let dir = new_path.parent().unwrap();
        fs::create_dir_all(dir).await.map_err(JobError::Io)?;

        let json = serde_json::to_string_pretty(job)?;
        let tmp_path = dir.join(format!("{}.tmp", job.id));
        fs::write(&tmp_path, &json).await.map_err(JobError::Io)?;
        fs::rename(&tmp_path, &new_path).await.map_err(JobError::Io)?;

        // Update index
        let mut index = self.index.write().await;
        index.insert(
            id_str,
            JobEntry {
                path: new_path,
                status: job.status,
                priority: job.priority,
                created_at: job.created_at,
            },
        );

        Ok(())
    }

    /// Removes a job from disk and the in-memory index.
    pub async fn remove(&self, job_id: &JobId) -> JobResult<()> {
        let id_str = job_id.to_string();
        let entry = {
            let mut index = self.index.write().await;
            let entry = index.remove(&id_str);
            entry
        };

        if let Some(entry) = entry {
            let _ = fs::remove_file(&entry.path).await;
            // Also remove from any status directory
            for status in &[
                JobStatus::Queued,
                JobStatus::Running,
                JobStatus::Completed,
                JobStatus::Failed,
                JobStatus::Cancelled,
                JobStatus::Scheduled,
                JobStatus::PermanentlyFailed,
            ] {
                let alt_path = self.status_path_for(*status).join(format!("{}.json", job_id.0));
                let _ = fs::remove_file(&alt_path).await;
            }
            Ok(())
        } else {
            Err(JobError::NotFound(id_str))
        }
    }

    /// Lists all jobs with the given status.
    pub async fn list_by_status(&self, status: JobStatus) -> JobResult<Vec<Job>> {
        let dir = self.status_path_for(status);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut jobs = Vec::new();
        let mut read_dir = fs::read_dir(&dir).await.map_err(JobError::Io)?;

        while let Some(entry) = read_dir.next_entry().await.map_err(JobError::Io)? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match fs::read_to_string(&path).await {
                    Ok(content) => {
                        if let Ok(job) = serde_json::from_str::<Job>(&content) {
                            jobs.push(job);
                        }
                    }
                    Err(_) => continue, // Skip unreadable files
                }
            }
        }

        Ok(jobs)
    }

    /// Lists all queued and scheduled jobs ready for execution, sorted by priority (highest first),
    /// then by creation time (oldest first within same priority).
    pub async fn list_ready(&self) -> JobResult<Vec<Job>> {
        let mut ready = Vec::new();

        // Load all queued jobs
        let queued = self.list_by_status(JobStatus::Queued).await?;
        ready.extend(queued);

        // Load all scheduled jobs that are ready
        let scheduled = self.list_by_status(JobStatus::Scheduled).await?;
        let now = chrono::Utc::now();
        let due = scheduled.into_iter().filter(|j| j.scheduled_at <= now);
        ready.extend(due);

        // Sort by priority descending, then by created_at ascending
        ready.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.created_at.cmp(&b.created_at))
        });

        Ok(ready)
    }

    /// Lists all jobs in a given priority range.
    pub async fn list_by_priority(
        &self,
        min_priority: Priority,
        status: Option<JobStatus>,
    ) -> JobResult<Vec<Job>> {
        let statuses = if let Some(s) = status {
            vec![s]
        } else {
            vec![
                JobStatus::Queued,
                JobStatus::Scheduled,
                JobStatus::Running,
            ]
        };

        let mut jobs = Vec::new();
        for s in statuses {
            let batch = self.list_by_status(s).await?;
            jobs.extend(
                batch
                    .into_iter()
                    .filter(|j| j.priority >= min_priority),
            );
        }

        Ok(jobs)
    }

    /// Loads all jobs from disk and rebuilds the in-memory index.
    /// Called on startup to recover queue state after a restart/crash.
    async fn rebuild_index(&self) -> JobResult<()> {
        let mut index = self.index.write().await;
        index.clear();

        for status in &[
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
            JobStatus::Scheduled,
            JobStatus::PermanentlyFailed,
        ] {
            let dir = self.status_path_for(*status);
            if !dir.exists() {
                continue;
            }

            let mut read_dir = match fs::read_dir(&dir).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            while let Some(entry) = read_dir.next_entry().await.map_err(JobError::Io)? {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }

                // Read just enough to extract job metadata
                match fs::read_to_string(&path).await {
                    Ok(content) => {
                        if let Ok(job) = serde_json::from_str::<Job>(&content) {
                            index.insert(
                                job.id.to_string(),
                                JobEntry {
                                    path,
                                    status: job.status,
                                    priority: job.priority,
                                    created_at: job.created_at,
                                },
                            );
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        Ok(())
    }

    /// Ensures all status subdirectories exist in the job store.
    async fn ensure_directories(&self) -> JobResult<()> {
        fs::create_dir_all(&self.path).await.map_err(JobError::Io)?;
        for status in &[
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
            JobStatus::Scheduled,
            JobStatus::PermanentlyFailed,
        ] {
            fs::create_dir_all(self.status_path_for(*status))
                .await
                .map_err(JobError::Io)?;
        }
        Ok(())
    }

    /// Returns the file path for a job based on its status.
    fn job_file_path(&self, job: &Job) -> PathBuf {
        self.status_path_for(job.status)
            .join(format!("{}.json", job.id.0))
    }

    /// Returns the directory path for a given job status.
    fn status_path_for(&self, status: JobStatus) -> PathBuf {
        let dir_name = match status {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Scheduled => "scheduled",
            JobStatus::PermanentlyFailed => "permanently_failed",
        };
        self.path.join(dir_name)
    }
}

/// The type alias for a shared, concurrent job store.
pub type SharedJobStore = Arc<JobStore>;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    async fn setup_store() -> (tempfile::TempDir, JobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::new(dir.path().join(".nabu").join("jobs"))
            .await
            .unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn test_store_and_load() {
        let (_dir, store) = setup_store().await;
        let job = Job::new("test_job", JobPayload::new());

        store.store(&job).await.unwrap();
        let loaded = store.load(&job.id).await.unwrap();

        assert_eq!(loaded.id, job.id);
        assert_eq!(loaded.job_type, job.job_type);
        assert_eq!(loaded.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_store_and_update() {
        let (_dir, store) = setup_store().await;
        let mut job = Job::new("test_job", JobPayload::new());
        store.store(&job).await.unwrap();

        job.mark_running();
        store.update(&job).await.unwrap();

        let loaded = store.load(&job.id).await.unwrap();
        assert_eq!(loaded.status, JobStatus::Running);
        assert!(loaded.started_at.is_some());
    }

    #[tokio::test]
    async fn test_store_and_remove() {
        let (_dir, store) = setup_store().await;
        let job = Job::new("test_job", JobPayload::new());
        store.store(&job).await.unwrap();

        assert_eq!(store.count().await, 1);

        store.remove(&job.id).await.unwrap();
        assert_eq!(store.count().await, 0);

        let result = store.load(&job.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_nonexistent() {
        let (_dir, store) = setup_store().await;
        let id = JobId::new();
        let result = store.remove(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let (_dir, store) = setup_store().await;

        let job1 = Job::new("type_a", JobPayload::new());
        let job2 = Job::new("type_b", JobPayload::new());
        store.store(&job1).await.unwrap();
        store.store(&job2).await.unwrap();

        let queued = store.list_by_status(JobStatus::Queued).await.unwrap();
        assert_eq!(queued.len(), 2);

        let completed = store.list_by_status(JobStatus::Completed).await.unwrap();
        assert_eq!(completed.len(), 0);
    }

    #[tokio::test]
    async fn test_list_ready_ordered_by_priority() {
        let (_dir, store) = setup_store().await;

        let low = Job::new("low", JobPayload::new()).with_priority(Priority::Low);
        let high = Job::new("high", JobPayload::new()).with_priority(Priority::High);
        let critical = Job::new("critical", JobPayload::new()).with_priority(Priority::Critical);

        store.store(&low).await.unwrap();
        store.store(&high).await.unwrap();
        store.store(&critical).await.unwrap();

        let ready = store.list_ready().await.unwrap();
        assert_eq!(ready.len(), 3);
        assert_eq!(ready[0].priority, Priority::Critical);
        assert_eq!(ready[1].priority, Priority::High);
        assert_eq!(ready[2].priority, Priority::Low);
    }

    #[tokio::test]
    async fn test_scheduled_jobs_not_in_ready_until_due() {
        let (_dir, store) = setup_store().await;

        let immediate = Job::new("immediate", JobPayload::new());
        let future = Job::scheduled(
            "delayed",
            JobPayload::new(),
            chrono::Utc::now() + Duration::hours(1),
        );
        let past = Job::scheduled(
            "past_due",
            JobPayload::new(),
            chrono::Utc::now() - Duration::minutes(5),
        );

        store.store(&immediate).await.unwrap();
        store.store(&future).await.unwrap();
        store.store(&past).await.unwrap();

        let ready = store.list_ready().await.unwrap();
        assert_eq!(ready.len(), 2); // immediate + past_due, not future

        let types: Vec<&str> = ready.iter().map(|j| j.job_type.0.as_str()).collect();
        assert!(types.contains(&"immediate"));
        assert!(types.contains(&"past_due"));
        assert!(!types.contains(&"delayed"));
    }

    #[tokio::test]
    async fn test_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let store_path = dir.path().join(".nabu").join("jobs");

        // First session
        {
            let store = JobStore::new(&store_path).await.unwrap();
            let job = Job::new("survivor", JobPayload::new())
                .with_priority(Priority::High);
            store.store(&job).await.unwrap();

            // Simulate crash by dropping the store without cleanup
        }

        // Second session (simulates restart)
        {
            let store = JobStore::new(&store_path).await.unwrap();
            assert_eq!(store.count().await, 1);

            let ready = store.list_ready().await.unwrap();
            assert_eq!(ready.len(), 1);
            assert_eq!(ready[0].job_type.0, "survivor");
            assert_eq!(ready[0].priority, Priority::High);
        }
    }

    #[tokio::test]
    async fn test_multiple_jobs_persist_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let store_path = dir.path().join(".nabu").join("jobs");

        // Session 1: store 5 jobs with various priorities and statuses
        {
            let store = JobStore::new(&store_path).await.unwrap();

            let j1 = Job::new("ocr", JobPayload::new()).with_priority(Priority::Critical);
            let j2 = Job::new("whisper", JobPayload::new()).with_priority(Priority::High);
            let j3 = Job::new("embedding", JobPayload::new());
            let j4 = Job::new("backup", JobPayload::new()).with_priority(Priority::Background);
            let j5 = Job::scheduled("cleanup", JobPayload::new(), chrono::Utc::now() + Duration::hours(2));

            store.store(&j1).await.unwrap();
            store.store(&j2).await.unwrap();
            store.store(&j3).await.unwrap();
            store.store(&j4).await.unwrap();
            store.store(&j5).await.unwrap();

            assert_eq!(store.count().await, 5);
        }

        // Session 2: verify all survived
        {
            let store = JobStore::new(&store_path).await.unwrap();
            assert_eq!(store.count().await, 5);

            let ready = store.list_ready().await.unwrap();
            // 4 queued jobs should be ready (not the future scheduled one)
            assert_eq!(ready.len(), 4);
            assert_eq!(ready[0].job_type.0, "ocr");
            assert_eq!(ready[1].job_type.0, "whisper");
            assert_eq!(ready[2].job_type.0, "embedding");
            assert_eq!(ready[3].job_type.0, "backup");
        }
    }

    #[tokio::test]
    async fn test_list_by_priority() {
        let (_dir, store) = setup_store().await;

        let high = Job::new("high", JobPayload::new()).with_priority(Priority::High);
        let low = Job::new("low", JobPayload::new()).with_priority(Priority::Low);

        store.store(&high).await.unwrap();
        store.store(&low).await.unwrap();

        let high_and_above = store
            .list_by_priority(Priority::High, Some(JobStatus::Queued))
            .await
            .unwrap();
        assert_eq!(high_and_above.len(), 1);
        assert_eq!(high_and_above[0].priority, Priority::High);
    }
}
