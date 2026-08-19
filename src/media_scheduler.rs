use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    sync::Arc,
};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaJobKind {
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaJobId {
    pub kind: MediaJobKind,
    pub source: String,
}

impl MediaJobId {
    pub fn image(source: impl Into<String>) -> Self {
        Self {
            kind: MediaJobKind::Image,
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediaPriority {
    FullsizeSpeculation,
    NearbyThumbnail,
    SelectedThumbnail,
    VisibleMedia,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledMediaJob {
    pub id: MediaJobId,
    pub priority: MediaPriority,
    pub generation: u64,
    attempt: u8,
}

impl ScheduledMediaJob {
    pub fn attempt(&self) -> u8 {
        self.attempt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitResult {
    Queued,
    Reprioritized,
    Deduplicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionResult {
    Accepted,
    Discarded,
    Retrying,
}

#[derive(Debug, Clone)]
struct JobRecord {
    priority: MediaPriority,
    generation: u64,
    attempt: u8,
    revision: u64,
    state: JobState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobState {
    Queued,
    InFlight,
    CancelledInFlight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueueEntry {
    id: MediaJobId,
    priority: MediaPriority,
    sequence: u64,
    revision: u64,
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct MediaScheduler {
    records: HashMap<MediaJobId, JobRecord>,
    queue: BinaryHeap<QueueEntry>,
    next_sequence: u64,
    active: usize,
    max_active: usize,
    max_attempts: u8,
    download_slots: Arc<Semaphore>,
    decode_slots: Arc<Semaphore>,
}

#[derive(Clone)]
pub struct MediaExecutionLimits {
    download_slots: Arc<Semaphore>,
    decode_slots: Arc<Semaphore>,
}

impl MediaExecutionLimits {
    pub async fn download_permit(&self) -> OwnedSemaphorePermit {
        self.download_slots
            .clone()
            .acquire_owned()
            .await
            .expect("media download semaphore remains open")
    }

    pub async fn decode_permit(&self) -> OwnedSemaphorePermit {
        self.decode_slots
            .clone()
            .acquire_owned()
            .await
            .expect("media decode semaphore remains open")
    }
}

impl Default for MediaScheduler {
    fn default() -> Self {
        Self::new(4, 2, 2)
    }
}

impl MediaScheduler {
    pub fn new(download_limit: usize, decode_limit: usize, max_attempts: u8) -> Self {
        assert!(download_limit > 0);
        assert!(decode_limit > 0);
        assert!(max_attempts > 0);
        Self {
            records: HashMap::new(),
            queue: BinaryHeap::new(),
            next_sequence: 0,
            active: 0,
            max_active: download_limit + decode_limit,
            max_attempts,
            download_slots: Arc::new(Semaphore::new(download_limit)),
            decode_slots: Arc::new(Semaphore::new(decode_limit)),
        }
    }

    pub fn submit(
        &mut self,
        id: MediaJobId,
        priority: MediaPriority,
        generation: u64,
    ) -> SubmitResult {
        if let Some(record) = self.records.get_mut(&id) {
            if record.state == JobState::CancelledInFlight {
                record.state = JobState::InFlight;
                record.priority = priority;
                record.generation = generation;
                return SubmitResult::Reprioritized;
            }
            if record.state == JobState::Queued && priority > record.priority {
                record.priority = priority;
                record.generation = generation;
                record.revision = record.revision.saturating_add(1);
                let revision = record.revision;
                self.push(id, priority, revision);
                return SubmitResult::Reprioritized;
            }
            return SubmitResult::Deduplicated;
        }

        self.records.insert(
            id.clone(),
            JobRecord {
                priority,
                generation,
                attempt: 1,
                revision: 0,
                state: JobState::Queued,
            },
        );
        self.push(id, priority, 0);
        SubmitResult::Queued
    }

    fn push(&mut self, id: MediaJobId, priority: MediaPriority, revision: u64) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.queue.push(QueueEntry {
            id,
            priority,
            sequence,
            revision,
        });
    }

    pub fn take_next(&mut self) -> Option<ScheduledMediaJob> {
        if self.active >= self.max_active {
            return None;
        }
        while let Some(entry) = self.queue.pop() {
            let Some(record) = self.records.get_mut(&entry.id) else {
                continue;
            };
            if record.state != JobState::Queued
                || record.revision != entry.revision
                || record.priority != entry.priority
            {
                continue;
            }
            record.state = JobState::InFlight;
            self.active += 1;
            return Some(ScheduledMediaJob {
                id: entry.id,
                priority: record.priority,
                generation: record.generation,
                attempt: record.attempt,
            });
        }
        None
    }

    pub fn cancel_speculation_before(&mut self, generation: u64) -> Vec<MediaJobId> {
        let mut cancelled = Vec::new();
        let mut queued_to_remove = Vec::new();
        for (id, record) in &mut self.records {
            if record.generation < generation
                && record.priority <= MediaPriority::SelectedThumbnail
                && record.state != JobState::CancelledInFlight
            {
                match record.state {
                    JobState::Queued => {
                        queued_to_remove.push(id.clone());
                        cancelled.push(id.clone());
                    }
                    JobState::InFlight => record.state = JobState::CancelledInFlight,
                    JobState::CancelledInFlight => unreachable!("filtered above"),
                }
            }
        }
        for id in queued_to_remove {
            self.records.remove(&id);
        }
        cancelled
    }

    pub fn complete(
        &mut self,
        job: &ScheduledMediaJob,
        succeeded: bool,
        retryable: bool,
    ) -> CompletionResult {
        let Some(mut record) = self.records.remove(&job.id) else {
            return CompletionResult::Discarded;
        };
        match record.state {
            JobState::CancelledInFlight => {
                self.active = self.active.saturating_sub(1);
                return CompletionResult::Discarded;
            }
            JobState::InFlight => self.active = self.active.saturating_sub(1),
            JobState::Queued => return CompletionResult::Discarded,
        }
        if !succeeded && retryable && record.attempt < self.max_attempts {
            record.attempt += 1;
            record.state = JobState::Queued;
            record.revision = record.revision.saturating_add(1);
            let revision = record.revision;
            let priority = record.priority;
            self.records.insert(job.id.clone(), record);
            self.push(job.id.clone(), priority, revision);
            CompletionResult::Retrying
        } else {
            CompletionResult::Accepted
        }
    }

    pub fn execution_limits(&self) -> MediaExecutionLimits {
        MediaExecutionLimits {
            download_slots: self.download_slots.clone(),
            decode_slots: self.decode_slots.clone(),
        }
    }

    pub fn queued(&self) -> usize {
        self.records
            .values()
            .filter(|record| record.state == JobState::Queued)
            .count()
    }

    pub fn active(&self) -> usize {
        self.active
    }
}

pub fn retryable_media_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "timeout",
        "timed out",
        "connect",
        "request failed",
        "503",
        "502",
        "429",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_work_overtakes_prefetch_and_reprioritizes_without_duplication() {
        let mut scheduler = MediaScheduler::new(1, 1, 2);
        scheduler.submit(
            MediaJobId::image("near-1"),
            MediaPriority::NearbyThumbnail,
            1,
        );
        scheduler.submit(
            MediaJobId::image("near-2"),
            MediaPriority::NearbyThumbnail,
            1,
        );
        assert_eq!(
            scheduler.submit(MediaJobId::image("near-2"), MediaPriority::VisibleMedia, 1),
            SubmitResult::Reprioritized
        );
        assert_eq!(scheduler.queued(), 2);
        assert_eq!(scheduler.take_next().unwrap().id.source, "near-2");
    }

    #[test]
    fn active_work_is_bounded_and_old_speculation_is_cancelled() {
        let mut scheduler = MediaScheduler::new(1, 1, 2);
        for index in 0..8 {
            scheduler.submit(
                MediaJobId::image(format!("image-{index}")),
                MediaPriority::NearbyThumbnail,
                1,
            );
        }
        let first = scheduler.take_next().unwrap();
        let second = scheduler.take_next().unwrap();
        assert!(scheduler.take_next().is_none());
        assert_eq!(scheduler.active(), 2);
        assert_eq!(scheduler.cancel_speculation_before(2).len(), 6);
        // Physical work keeps its slot until completion, so cancellation can
        // never oversubscribe the independent execution semaphores.
        assert_eq!(scheduler.active(), 2);
        assert_eq!(scheduler.queued(), 0);
        assert_eq!(
            scheduler.complete(&first, true, false),
            CompletionResult::Discarded
        );
        assert_eq!(scheduler.active(), 1);
        assert_eq!(
            scheduler.submit(second.id.clone(), MediaPriority::VisibleMedia, 2),
            SubmitResult::Reprioritized
        );
        assert_eq!(
            scheduler.complete(&second, true, false),
            CompletionResult::Accepted
        );
    }

    #[test]
    fn transient_failure_retries_once_but_permanent_failure_does_not() {
        let mut scheduler = MediaScheduler::new(1, 1, 2);
        let id = MediaJobId::image("image");
        scheduler.submit(id.clone(), MediaPriority::VisibleMedia, 1);
        let first = scheduler.take_next().unwrap();
        assert_eq!(
            scheduler.complete(&first, false, true),
            CompletionResult::Retrying
        );
        let second = scheduler.take_next().unwrap();
        assert_eq!(second.attempt(), 2);
        assert_eq!(
            scheduler.complete(&second, false, true),
            CompletionResult::Accepted
        );
        assert_eq!(scheduler.queued(), 0);

        scheduler.submit(id, MediaPriority::VisibleMedia, 1);
        let permanent = scheduler.take_next().unwrap();
        assert_eq!(
            scheduler.complete(&permanent, false, false),
            CompletionResult::Accepted
        );
    }

    #[test]
    fn failure_classifier_is_conservative() {
        assert!(retryable_media_error("request timed out"));
        assert!(retryable_media_error("image request failed: 503"));
        assert!(!retryable_media_error("could not decode image bytes"));
        assert!(!retryable_media_error("404 not found"));
    }
}
