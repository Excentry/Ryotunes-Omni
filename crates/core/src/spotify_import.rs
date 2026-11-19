use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::Db;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Import state is unavailable.")]
    Unavailable,
    #[error("Import job was not found for this session.")]
    NotFound,
    #[error("Import job changed or is not open for this operation.")]
    Conflict,
    #[error("Invalid import source, candidates or decisions.")]
    Invalid,
    #[error("Import storage operation failed.")]
    Storage(#[from] rusqlite::Error),
    #[error("Import metadata could not be encoded.")]
    Encoding(#[from] serde_json::Error),
}

pub type ImportResult<T> = Result<T, ImportError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportOwner {
    pub window_key: String,
    pub account_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSource {
    pub account_key: String,
    pub playlist_id: String,
    pub snapshot_id: String,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTarget {
    pub playlist_id: Option<String>,
    pub revision: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportOutcome {
    Matched,
    LowConfidence,
    NotFound,
    Unsupported,
    SourceError,
    SearchError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportPhase {
    Loading,
    Matching,
    Review,
    Committing,
    Committed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub candidate_id: String,
    pub video_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOccurrence {
    pub occurrence_id: String,
    pub ordinal: usize,
    pub outcome: ImportOutcome,
    pub candidates: Vec<ImportCandidate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReview {
    pub job_id: String,
    pub revision: u64,
    pub phase: ImportPhase,
    pub source: ImportSource,
    pub target: ImportTarget,
    pub occurrences: Vec<ImportOccurrence>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ImportDecision {
    Skip { occurrence_id: String },
    Accept { occurrence_id: String, candidate_id: String },
}

impl ImportDecision {
    fn occurrence_id(&self) -> &str {
        match self {
            Self::Skip { occurrence_id } | Self::Accept { occurrence_id, .. } => occurrence_id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitRequest {
    pub job_id: String,
    pub expected_review_revision: u64,
    pub expected_playlist_revision: Option<u64>,
    pub decisions: Vec<ImportDecision>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReceipt {
    pub job_id: String,
    pub playlist_id: String,
    pub playlist_revision: u64,
    pub total: usize,
    pub accepted: usize,
    pub skipped: usize,
}

pub(crate) struct ImportEntry {
    pub entry_id: String,
    pub source_ordinal: usize,
    pub video_id: String,
    pub song_json: String,
}

pub(crate) struct ImportPlan {
    pub job_id: String,
    pub source: ImportSource,
    pub target: ImportTarget,
    pub title: String,
    pub digest: String,
    pub total: usize,
    pub entries: Vec<ImportEntry>,
}

#[derive(Clone)]
struct Job {
    owner: ImportOwner,
    title: String,
    review: ImportReview,
    committed: Option<(u64, String, ImportReceipt)>,
}

pub struct SpotifyImportJobs {
    db: Arc<Db>,
    jobs: Mutex<HashMap<String, Job>>,
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024
}

fn video_id(value: &str) -> bool {
    value.len() == 11 && value.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

fn validate_occurrences(source: &ImportSource, entries: &[ImportOccurrence]) -> ImportResult<()> {
    if entries.len() != source.total {
        return Err(ImportError::Invalid);
    }
    let mut ids = HashSet::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.ordinal != index
            || !text(&entry.occurrence_id)
            || !ids.insert(&entry.occurrence_id)
            || entry.candidates.len() > 100
        {
            return Err(ImportError::Invalid);
        }
        let mut candidates = HashSet::new();
        for candidate in &entry.candidates {
            if !text(&candidate.candidate_id)
                || !candidates.insert(&candidate.candidate_id)
                || !video_id(&candidate.video_id)
                || !text(&candidate.title)
                || candidate.artists.is_empty()
                || candidate.artists.len() > 30
                || !candidate.artists.iter().all(|artist| text(artist))
                || candidate
                    .duration_ms
                    .is_some_and(|duration| duration == 0 || duration > 86_400_000)
            {
                return Err(ImportError::Invalid);
            }
        }
    }
    Ok(())
}

fn plan(job: &Job, request: &ImportCommitRequest) -> ImportResult<ImportPlan> {
    if request.expected_review_revision != job.review.revision
        || request.expected_playlist_revision != job.review.target.revision
        || request.decisions.len() != job.review.source.total
    {
        return Err(ImportError::Conflict);
    }
    validate_occurrences(&job.review.source, &job.review.occurrences)?;
    let mut choices = HashMap::new();
    for choice in &request.decisions {
        if choices.insert(choice.occurrence_id(), choice).is_some() {
            return Err(ImportError::Invalid);
        }
    }
    let mut canonical = Vec::new();
    let mut entries = Vec::new();
    for occurrence in &job.review.occurrences {
        let choice = choices.get(occurrence.occurrence_id.as_str()).ok_or(ImportError::Invalid)?;
        canonical.push(*choice);
        if let ImportDecision::Accept { candidate_id, .. } = choice {
            let candidate = occurrence
                .candidates
                .iter()
                .find(|candidate| candidate.candidate_id == *candidate_id)
                .ok_or(ImportError::Invalid)?;
            let item = innertube::SongItem {
                video_id: candidate.video_id.clone(),
                title: candidate.title.clone(),
                artists: candidate.artists.join(" & "),
                duration: candidate.duration_ms.map(|ms| {
                    let seconds = ms / 1000;
                    format!("{}:{:02}", seconds / 60, seconds % 60)
                }),
                ..Default::default()
            };
            entries.push(ImportEntry {
                entry_id: format!("{}:{}", job.review.job_id, occurrence.ordinal),
                source_ordinal: occurrence.ordinal,
                video_id: candidate.video_id.clone(),
                song_json: serde_json::to_string(&item)?,
            });
        }
    }
    if entries.is_empty() {
        return Err(ImportError::Invalid);
    }
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            &job.review.job_id,
            job.review.revision,
            &job.review.source.snapshot_id,
            &job.review.target,
            canonical,
        ))?)
    );
    Ok(ImportPlan {
        job_id: job.review.job_id.clone(),
        source: job.review.source.clone(),
        target: job.review.target.clone(),
        title: job.title.clone(),
        digest,
        total: job.review.source.total,
        entries,
    })
}

impl SpotifyImportJobs {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db, jobs: Mutex::new(HashMap::new()) }
    }

    pub fn start(
        &self,
        owner: ImportOwner,
        mut source: ImportSource,
        title: String,
    ) -> ImportResult<ImportReview> {
        if !text(&owner.window_key)
            || !text(&owner.account_key)
            || !text(&source.snapshot_id)
            || source.playlist_id.len() != 22
            || !source.playlist_id.bytes().all(|c| c.is_ascii_alphanumeric())
            || source.total > 10_000
            || !text(&title)
            || title.chars().count() > 150
        {
            return Err(ImportError::Invalid);
        }
        source.account_key = owner.account_key.clone();
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        if jobs.len() >= 32 {
            return Err(ImportError::Unavailable);
        }
        let target = self.db.spotify_import_target(&source.account_key, &source.playlist_id)?;
        let job_id = format!("spotify-import-{:032x}", rand::random::<u128>());
        let review = ImportReview {
            job_id: job_id.clone(),
            revision: 0,
            phase: ImportPhase::Loading,
            source,
            target,
            occurrences: Vec::new(),
        };
        jobs.insert(job_id, Job { owner, title, review: review.clone(), committed: None });
        Ok(review)
    }

    pub fn snapshot(&self, owner: &ImportOwner, id: &str) -> ImportResult<ImportReview> {
        let jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let job = jobs.get(id).filter(|job| job.owner == *owner).ok_or(ImportError::NotFound)?;
        Ok(job.review.clone())
    }

    pub fn review_job_ids(&self, owner: &ImportOwner) -> ImportResult<Vec<String>> {
        let jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let mut ids: Vec<_> = jobs
            .iter()
            .filter(|(_, job)| job.owner == *owner && job.review.phase == ImportPhase::Review)
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        Ok(ids)
    }

    pub fn begin_matching(&self, owner: &ImportOwner, id: &str) -> ImportResult<ImportReview> {
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let job =
            jobs.get_mut(id).filter(|job| job.owner == *owner).ok_or(ImportError::NotFound)?;
        if job.review.phase != ImportPhase::Loading {
            return Err(ImportError::Conflict);
        }
        job.review.phase = ImportPhase::Matching;
        job.review.revision += 1;
        Ok(job.review.clone())
    }

    pub fn offer_review(
        &self,
        owner: &ImportOwner,
        id: &str,
        entries: Vec<ImportOccurrence>,
    ) -> ImportResult<ImportReview> {
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let job =
            jobs.get_mut(id).filter(|job| job.owner == *owner).ok_or(ImportError::NotFound)?;
        if job.review.phase != ImportPhase::Matching {
            return Err(ImportError::Conflict);
        }
        validate_occurrences(&job.review.source, &entries)?;
        job.review.occurrences = entries;
        job.review.phase = ImportPhase::Review;
        job.review.revision += 1;
        Ok(job.review.clone())
    }

    pub fn offer_candidates(
        &self,
        owner: &ImportOwner,
        id: &str,
        occurrence_id: &str,
        expected_revision: u64,
        candidates: Vec<ImportCandidate>,
    ) -> ImportResult<ImportReview> {
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let job =
            jobs.get_mut(id).filter(|job| job.owner == *owner).ok_or(ImportError::NotFound)?;
        if job.review.phase != ImportPhase::Review || job.review.revision != expected_revision {
            return Err(ImportError::Conflict);
        }
        let mut entries = job.review.occurrences.clone();
        let entry = entries
            .iter_mut()
            .find(|entry| entry.occurrence_id == occurrence_id)
            .ok_or(ImportError::Invalid)?;
        entry.candidates = candidates;
        validate_occurrences(&job.review.source, &entries)?;
        job.review.occurrences = entries;
        job.review.revision += 1;
        Ok(job.review.clone())
    }

    pub fn cancel(&self, owner: &ImportOwner, id: &str) -> ImportResult<ImportReview> {
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let job =
            jobs.get_mut(id).filter(|job| job.owner == *owner).ok_or(ImportError::NotFound)?;
        if !matches!(
            job.review.phase,
            ImportPhase::Committed | ImportPhase::Cancelled | ImportPhase::Failed
        ) {
            job.review.phase = ImportPhase::Cancelled;
            job.review.revision += 1;
            job.review.occurrences.clear();
        }
        Ok(job.review.clone())
    }

    pub fn invalidate(&self, owner: &ImportOwner) -> ImportResult<()> {
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        jobs.retain(|_, job| job.owner != *owner);
        Ok(())
    }

    pub fn release(&self, owner: &ImportOwner, id: &str) -> ImportResult<()> {
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let job = jobs.get(id).filter(|job| job.owner == *owner).ok_or(ImportError::NotFound)?;
        if !matches!(
            job.review.phase,
            ImportPhase::Committed | ImportPhase::Cancelled | ImportPhase::Failed
        ) {
            return Err(ImportError::Conflict);
        }
        jobs.remove(id);
        Ok(())
    }

    pub fn commit(
        &self,
        owner: &ImportOwner,
        request: &ImportCommitRequest,
    ) -> ImportResult<ImportReceipt> {
        let mut jobs = self.jobs.lock().map_err(|_| ImportError::Unavailable)?;
        let job = jobs
            .get_mut(&request.job_id)
            .filter(|job| job.owner == *owner)
            .ok_or(ImportError::NotFound)?;
        if !matches!(job.review.phase, ImportPhase::Review | ImportPhase::Committed) {
            return Err(ImportError::Conflict);
        }
        if let Some((revision, digest, receipt)) = &job.committed {
            let mut original = job.clone();
            original.review.revision = *revision;
            if plan(&original, request)?.digest != *digest {
                return Err(ImportError::Conflict);
            }
            return Ok(receipt.clone());
        }
        let planned = plan(job, request)?;
        let digest = planned.digest.clone();
        let review_revision = job.review.revision;
        let previous_phase = job.review.phase;
        job.review.phase = ImportPhase::Committing;
        match self.db.commit_spotify_import(&planned) {
            Ok(receipt) => {
                job.review.phase = ImportPhase::Committed;
                job.review.revision += 1;
                job.committed = Some((review_revision, digest, receipt.clone()));
                Ok(receipt)
            }
            Err(error) => {
                job.review.phase = previous_phase;
                Err(error)
            }
        }
    }
}
