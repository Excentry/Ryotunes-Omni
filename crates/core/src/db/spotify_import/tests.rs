use std::sync::{Arc, Barrier};

use super::*;
use crate::spotify_import::*;

fn owner() -> ImportOwner {
    ImportOwner { window_key: "main-session".into(), account_key: "spotify-account".into() }
}

fn database() -> Arc<Db> {
    Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap())
}

fn review(jobs: &SpotifyImportJobs, count: usize) -> (ImportReview, ImportCommitRequest) {
    let source = ImportSource {
        account_key: "untrusted-account".into(),
        playlist_id: "3zlBonH1l7BOTkoKITprOt".into(),
        snapshot_id: "snapshot-a".into(),
        total: count,
    };
    let started = jobs.start(owner(), source, "Source title".into()).unwrap();
    jobs.begin_matching(&owner(), &started.job_id).unwrap();
    let entries = (0..count)
        .map(|ordinal| ImportOccurrence {
            occurrence_id: format!("occurrence-{ordinal}"),
            ordinal,
            outcome: ImportOutcome::Matched,
            candidates: vec![ImportCandidate {
                candidate_id: format!("candidate-{ordinal}"),
                video_id: "aaaaaaaaaaa".into(),
                title: "Song".into(),
                artists: vec!["Artist".into()],
                duration_ms: Some(180_000),
            }],
        })
        .collect();
    let review = jobs.offer_review(&owner(), &started.job_id, entries).unwrap();
    let request = ImportCommitRequest {
        job_id: review.job_id.clone(),
        expected_review_revision: review.revision,
        expected_playlist_revision: review.target.revision,
        decisions: review
            .occurrences
            .iter()
            .map(|entry| ImportDecision::Accept {
                occurrence_id: entry.occurrence_id.clone(),
                candidate_id: entry.candidates[0].candidate_id.clone(),
            })
            .collect(),
    };
    (review, request)
}

#[test]
fn lists_only_review_jobs_for_the_current_owner() {
    let jobs = SpotifyImportJobs::new(database());
    let (active, request) = review(&jobs, 1);
    assert_eq!(jobs.review_job_ids(&owner()).unwrap(), vec![active.job_id]);
    assert!(jobs
        .review_job_ids(&ImportOwner {
            window_key: "other-window".into(),
            account_key: owner().account_key,
        })
        .unwrap()
        .is_empty());
    jobs.commit(&owner(), &request).unwrap();
    assert!(jobs.review_job_ids(&owner()).unwrap().is_empty());
}

#[test]
fn preserves_1251_repeated_occurrences_and_set_like_memberships() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 1251);
    let receipt = jobs.commit(&owner(), &request).unwrap();
    let entries = db.imported_playlist_entries(&receipt.playlist_id).unwrap();
    assert_eq!(entries.len(), 1251);
    assert_eq!(entries[1250].position, 1250);
    assert_eq!(entries[1250].source_ordinal, 1250);
    assert_ne!(entries[0].entry_id, entries[1].entry_id);
    assert_eq!(db.local_playlist(&receipt.playlist_id).unwrap().track_count, 1251);
    assert_eq!(db.local_playlist_track_json(&receipt.playlist_id).len(), 1251);
    assert_eq!(db.playlist_memberships()["aaaaaaaaaaa"], vec![receipt.playlist_id]);
}

#[test]
fn removes_only_one_occurrence_and_reorders_with_dense_positions() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 4);
    let receipt = jobs.commit(&owner(), &request).unwrap();
    let before = db.imported_playlist_entries(&receipt.playlist_id).unwrap();
    let revision =
        db.remove_imported_playlist_entry(&receipt.playlist_id, &before[1].entry_id, 1).unwrap();
    let order =
        vec![before[3].entry_id.clone(), before[0].entry_id.clone(), before[2].entry_id.clone()];
    assert_eq!(
        db.reorder_imported_playlist_entries(&receipt.playlist_id, &order, revision).unwrap(),
        3
    );
    let after = db.imported_playlist_entries(&receipt.playlist_id).unwrap();
    assert_eq!(after.iter().map(|entry| entry.entry_id.clone()).collect::<Vec<_>>(), order);
    assert_eq!(after.iter().map(|entry| entry.position).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(after.iter().map(|entry| entry.source_ordinal).collect::<Vec<_>>(), vec![3, 0, 2]);
    assert!(matches!(
        db.remove_imported_playlist_entry(&receipt.playlist_id, &before[0].entry_id, 1),
        Err(ImportError::Conflict)
    ));
}

#[test]
fn reorder_rejects_duplicates_without_changing_revision_or_contents() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 2);
    let receipt = jobs.commit(&owner(), &request).unwrap();
    let before = db.imported_playlist_entries(&receipt.playlist_id).unwrap();
    let order = vec![before[0].entry_id.clone(); 2];
    assert!(matches!(
        db.reorder_imported_playlist_entries(&receipt.playlist_id, &order, 1),
        Err(ImportError::Invalid)
    ));
    assert_eq!(
        db.spotify_import_target(&owner().account_key, "3zlBonH1l7BOTkoKITprOt").unwrap().revision,
        Some(1)
    );
    assert_eq!(
        db.imported_playlist_entries(&receipt.playlist_id).unwrap()[1].entry_id,
        before[1].entry_id
    );
}

#[test]
fn commit_replay_is_order_independent_and_does_not_resurrect_deleted_playlist() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, mut request) = review(&jobs, 3);
    let receipt = jobs.commit(&owner(), &request).unwrap();
    request.decisions.reverse();
    assert_eq!(jobs.commit(&owner(), &request).unwrap(), receipt);
    request.decisions[0] = ImportDecision::Skip { occurrence_id: "occurrence-2".into() };
    assert!(matches!(jobs.commit(&owner(), &request), Err(ImportError::Conflict)));
    request.decisions[0] = ImportDecision::Accept {
        occurrence_id: "occurrence-2".into(),
        candidate_id: "candidate-2".into(),
    };
    db.delete_local_playlist(&receipt.playlist_id).unwrap();
    assert_eq!(jobs.commit(&owner(), &request).unwrap(), receipt);
    assert!(db.local_playlist(&receipt.playlist_id).is_none());
    assert_eq!(
        db.spotify_import_receipt(&owner().account_key, &request.job_id).unwrap(),
        Some(receipt)
    );
    assert!(db.spotify_import_receipt("other-account", &request.job_id).unwrap().is_none());
}

#[test]
fn reimport_preserves_destination_and_local_title_but_conflicts_with_edits() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, first) = review(&jobs, 2);
    let receipt = jobs.commit(&owner(), &first).unwrap();
    db.rename_local_playlist(&receipt.playlist_id, "My title").unwrap();
    let (_, stale) = review(&jobs, 3);
    let (_, fresh) = review(&jobs, 4);
    let replacement = jobs.commit(&owner(), &fresh).unwrap();
    assert_eq!(replacement.playlist_id, receipt.playlist_id);
    assert_eq!(replacement.playlist_revision, 3);
    assert_eq!(db.local_playlist(&receipt.playlist_id).unwrap().title, "My title");
    assert!(matches!(jobs.commit(&owner(), &stale), Err(ImportError::Conflict)));
    let (_, deleted) = review(&jobs, 1);
    db.delete_local_playlist(&receipt.playlist_id).unwrap();
    assert!(matches!(jobs.commit(&owner(), &deleted), Err(ImportError::Conflict)));
    assert!(db.local_playlists().is_empty());
}

#[test]
fn concurrent_new_source_commits_have_exactly_one_winner() {
    let db = database();
    let jobs = Arc::new(SpotifyImportJobs::new(db.clone()));
    let (_, first) = review(&jobs, 2);
    let (_, second) = review(&jobs, 3);
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = [first, second]
        .into_iter()
        .map(|request| {
            let jobs = jobs.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                jobs.commit(&owner(), &request)
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles.into_iter().map(|handle| handle.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results.iter().filter(|result| matches!(result, Err(ImportError::Conflict))).count(),
        1
    );
    assert_eq!(db.local_playlists().len(), 1);
}

#[test]
fn cancellation_and_invalidation_leave_destination_unchanged() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (snapshot, request) = review(&jobs, 2);
    let foreign = ImportOwner { window_key: "other-window".into(), ..owner() };
    assert!(matches!(jobs.snapshot(&foreign, &snapshot.job_id), Err(ImportError::NotFound)));
    assert!(matches!(jobs.commit(&foreign, &request), Err(ImportError::NotFound)));
    jobs.cancel(&owner(), &snapshot.job_id).unwrap();
    assert!(matches!(jobs.commit(&owner(), &request), Err(ImportError::Conflict)));
    jobs.invalidate(&owner()).unwrap();
    assert!(matches!(jobs.snapshot(&owner(), &snapshot.job_id), Err(ImportError::NotFound)));
    assert!(db.local_playlists().is_empty());
}

#[test]
fn updated_candidates_invalidate_prior_review_and_scope_candidate_ids() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (snapshot, mut request) = review(&jobs, 2);
    request.decisions[0] = ImportDecision::Accept {
        occurrence_id: "occurrence-0".into(),
        candidate_id: "candidate-1".into(),
    };
    assert!(matches!(jobs.commit(&owner(), &request), Err(ImportError::Invalid)));
    let updated = jobs
        .offer_candidates(
            &owner(),
            &snapshot.job_id,
            "occurrence-0",
            snapshot.revision,
            snapshot.occurrences[0].candidates.clone(),
        )
        .unwrap();
    assert!(updated.revision > snapshot.revision);
    assert!(matches!(jobs.commit(&owner(), &request), Err(ImportError::Conflict)));
    assert!(db.local_playlists().is_empty());
}

#[test]
fn rollback_initial_import_after_partial_insert_and_allow_retry() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (snapshot, request) = review(&jobs, 3);
    db.0.lock()
        .unwrap()
        .execute_batch(
            "CREATE TEMP TRIGGER fail_import BEFORE INSERT ON local_playlist_import_entries
         WHEN NEW.position = 1 BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();
    assert!(matches!(jobs.commit(&owner(), &request), Err(ImportError::Storage(_))));
    assert!(db.local_playlists().is_empty());
    assert_eq!(jobs.snapshot(&owner(), &snapshot.job_id).unwrap().phase, ImportPhase::Review);
    assert!(db.spotify_import_receipt(&owner().account_key, &request.job_id).unwrap().is_none());
    db.0.lock().unwrap().execute_batch("DROP TRIGGER fail_import").unwrap();
    assert_eq!(jobs.commit(&owner(), &request).unwrap().accepted, 3);
}

#[test]
fn rollback_replacement_when_receipt_insert_fails() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 2);
    let original = jobs.commit(&owner(), &request).unwrap();
    let before = db.local_playlist_track_json(&original.playlist_id);
    let (_, replacement) = review(&jobs, 4);
    db.0.lock()
        .unwrap()
        .execute_batch(
            "CREATE TEMP TRIGGER fail_receipt BEFORE INSERT ON local_playlist_import_receipts
         BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END;",
        )
        .unwrap();
    assert!(matches!(jobs.commit(&owner(), &replacement), Err(ImportError::Storage(_))));
    assert_eq!(db.local_playlist_track_json(&original.playlist_id), before);
    assert_eq!(
        db.spotify_import_target(&owner().account_key, "3zlBonH1l7BOTkoKITprOt").unwrap().revision,
        Some(1)
    );
    assert!(db
        .spotify_import_receipt(&owner().account_key, &replacement.job_id)
        .unwrap()
        .is_none());
}

#[test]
fn legacy_playlists_keep_deduplication_and_imports_reject_video_based_edits() {
    let db = database();
    db.create_local_playlist("legacy", "Existing").unwrap();
    assert!(db
        .add_local_playlist_track(
            "legacy",
            "aaaaaaaaaaa",
            r#"{"video_id":"aaaaaaaaaaa","title":"Existing","artists":"Artist"}"#
        )
        .unwrap());
    assert!(!db.add_local_playlist_track("legacy", "aaaaaaaaaaa", "{}").unwrap());
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 2);
    let receipt = jobs.commit(&owner(), &request).unwrap();
    assert!(db.add_local_playlist_track(&receipt.playlist_id, "aaaaaaaaaaa", "{}").is_err());
    assert!(db.remove_local_playlist_track(&receipt.playlist_id, "aaaaaaaaaaa").is_err());
    assert_eq!(db.local_playlist("legacy").unwrap().track_count, 1);
    assert_eq!(db.imported_playlist_entries(&receipt.playlist_id).unwrap().len(), 2);
    db.remove_local_playlist_track("legacy", "aaaaaaaaaaa").unwrap();
    assert_eq!(db.local_playlist("legacy").unwrap().track_count, 0);
}

#[test]
fn additive_schema_preserves_legacy_rows_on_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("import-test.sqlite");
    {
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(
            "CREATE TABLE local_playlists(id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE local_playlist_tracks(playlist_id TEXT NOT NULL, video_id TEXT NOT NULL, position INTEGER NOT NULL, song_json TEXT NOT NULL, PRIMARY KEY(playlist_id, video_id));
             INSERT INTO local_playlists VALUES('legacy', 'Preserved', 1, 1);
             INSERT INTO local_playlist_tracks VALUES('legacy', 'aaaaaaaaaaa', 0, '{}');"
        ).unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(db.local_playlist("legacy").unwrap().title, "Preserved");
    assert_eq!(db.local_playlist_track_json("legacy"), vec!["{}"]);
    drop(db);
    let reopened = Db::open(&path).unwrap();
    assert_eq!(reopened.local_playlist("legacy").unwrap().track_count, 1);
}

#[test]
fn manually_offered_candidates_can_resolve_unsupported_and_source_error_occurrences() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let source = ImportSource {
        account_key: owner().account_key,
        playlist_id: "3zlBonH1l7BOTkoKITprOt".into(),
        snapshot_id: "snapshot".into(),
        total: 3,
    };
    let started = jobs.start(owner(), source, "Import".into()).unwrap();
    jobs.begin_matching(&owner(), &started.job_id).unwrap();
    let initial = jobs
        .offer_review(
            &owner(),
            &started.job_id,
            vec![
                ImportOccurrence {
                    occurrence_id: "source-error".into(),
                    ordinal: 0,
                    outcome: ImportOutcome::SourceError,
                    candidates: Vec::new(),
                },
                ImportOccurrence {
                    occurrence_id: "unsupported".into(),
                    ordinal: 1,
                    outcome: ImportOutcome::Unsupported,
                    candidates: Vec::new(),
                },
                ImportOccurrence {
                    occurrence_id: "matched".into(),
                    ordinal: 2,
                    outcome: ImportOutcome::Matched,
                    candidates: vec![ImportCandidate {
                        candidate_id: "candidate-matched".into(),
                        video_id: "aaaaaaaaaaa".into(),
                        title: "Song".into(),
                        artists: vec!["Artist".into()],
                        duration_ms: Some(180_000),
                    }],
                },
            ],
        )
        .unwrap();
    let source_error = jobs
        .offer_candidates(
            &owner(),
            &started.job_id,
            "source-error",
            initial.revision,
            vec![ImportCandidate {
                candidate_id: "candidate-source-error".into(),
                video_id: "bbbbbbbbbbb".into(),
                title: "Recovered source song".into(),
                artists: vec!["Artist".into()],
                duration_ms: None,
            }],
        )
        .unwrap();
    let unsupported = jobs
        .offer_candidates(
            &owner(),
            &started.job_id,
            "unsupported",
            source_error.revision,
            vec![ImportCandidate {
                candidate_id: "candidate-unsupported".into(),
                video_id: "ccccccccccc".into(),
                title: "Manual replacement".into(),
                artists: vec!["Artist".into()],
                duration_ms: None,
            }],
        )
        .unwrap();
    let request = ImportCommitRequest {
        job_id: unsupported.job_id,
        expected_review_revision: unsupported.revision,
        expected_playlist_revision: None,
        decisions: vec![
            ImportDecision::Accept {
                occurrence_id: "source-error".into(),
                candidate_id: "candidate-source-error".into(),
            },
            ImportDecision::Accept {
                occurrence_id: "unsupported".into(),
                candidate_id: "candidate-unsupported".into(),
            },
            ImportDecision::Accept {
                occurrence_id: "matched".into(),
                candidate_id: "candidate-matched".into(),
            },
        ],
    };
    let receipt = jobs.commit(&owner(), &request).unwrap();
    assert_eq!((receipt.total, receipt.accepted, receipt.skipped), (3, 3, 0));
    assert_eq!(
        db.imported_playlist_entries(&receipt.playlist_id)
            .unwrap()
            .iter()
            .map(|entry| entry.source_ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn empty_or_duplicate_decisions_cannot_replace_destination() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 2);
    let original = jobs.commit(&owner(), &request).unwrap();
    let (_, mut replacement) = review(&jobs, 2);
    replacement.decisions = vec![replacement.decisions[0].clone(); 2];
    assert!(matches!(jobs.commit(&owner(), &replacement), Err(ImportError::Invalid)));
    replacement.decisions = (0..2)
        .map(|index| ImportDecision::Skip { occurrence_id: format!("occurrence-{index}") })
        .collect();
    assert!(matches!(jobs.commit(&owner(), &replacement), Err(ImportError::Invalid)));
    assert_eq!(db.local_playlist(&original.playlist_id).unwrap().track_count, 2);
}

#[test]
fn cancelled_reimport_preserves_existing_playlist() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 2);
    let original = jobs.commit(&owner(), &request).unwrap();
    let (_, replacement) = review(&jobs, 3);
    jobs.cancel(&owner(), &replacement.job_id).unwrap();
    assert!(matches!(jobs.commit(&owner(), &replacement), Err(ImportError::Conflict)));
    assert_eq!(db.local_playlist(&original.playlist_id).unwrap().track_count, 2);
    assert_eq!(
        db.spotify_import_target(&owner().account_key, "3zlBonH1l7BOTkoKITprOt").unwrap().revision,
        Some(1)
    );
}

#[test]
fn cancel_racing_commit_never_claims_rollback_after_commit() {
    let db = database();
    let jobs = Arc::new(SpotifyImportJobs::new(db.clone()));
    let (_, request) = review(&jobs, 2);
    let barrier = Arc::new(Barrier::new(2));
    let cancel_jobs = jobs.clone();
    let cancel_barrier = barrier.clone();
    let id = request.job_id.clone();
    let handle = std::thread::spawn(move || {
        cancel_barrier.wait();
        cancel_jobs.cancel(&owner(), &id).unwrap()
    });
    barrier.wait();
    let commit = jobs.commit(&owner(), &request);
    let cancellation = handle.join().unwrap();
    match commit {
        Ok(receipt) => {
            assert_eq!(cancellation.phase, ImportPhase::Committed);
            assert_eq!(db.local_playlist(&receipt.playlist_id).unwrap().track_count, 2);
        }
        Err(ImportError::Conflict) => {
            assert_eq!(cancellation.phase, ImportPhase::Cancelled);
            assert!(db.local_playlists().is_empty());
        }
        other => panic!("Unexpected commit result: {other:?}"),
    }
}

#[test]
fn remove_rolls_back_when_position_update_fails() {
    let db = database();
    let jobs = SpotifyImportJobs::new(db.clone());
    let (_, request) = review(&jobs, 3);
    let receipt = jobs.commit(&owner(), &request).unwrap();
    let before = db.imported_playlist_entries(&receipt.playlist_id).unwrap();
    db.0.lock().unwrap().execute_batch(
        "CREATE TEMP TRIGGER fail_positions BEFORE UPDATE OF position ON local_playlist_import_entries
         BEGIN SELECT RAISE(ABORT, 'injected position failure'); END;"
    ).unwrap();
    assert!(matches!(
        db.remove_imported_playlist_entry(&receipt.playlist_id, &before[1].entry_id, 1),
        Err(ImportError::Storage(_))
    ));
    let after = db.imported_playlist_entries(&receipt.playlist_id).unwrap();
    assert_eq!(after.len(), 3);
    assert_eq!(after[1].entry_id, before[1].entry_id);
    assert_eq!(
        db.spotify_import_target(&owner().account_key, "3zlBonH1l7BOTkoKITprOt").unwrap().revision,
        Some(1)
    );
}

#[test]
fn durable_receipt_replays_after_reopen_without_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("durable-import.sqlite");
    let db = Db::open(&path).unwrap();
    let mut plan = ImportPlan {
        job_id: "durable-job".into(),
        source: ImportSource {
            account_key: owner().account_key,
            playlist_id: "3zlBonH1l7BOTkoKITprOt".into(),
            snapshot_id: "snapshot".into(),
            total: 1,
        },
        target: ImportTarget { playlist_id: None, revision: None },
        title: "Import".into(),
        digest: "host-decision-digest".into(),
        total: 1,
        entries: vec![ImportEntry {
            entry_id: "entry".into(),
            source_ordinal: 0,
            video_id: "aaaaaaaaaaa".into(),
            song_json: "{}".into(),
        }],
    };
    let receipt = db.commit_spotify_import(&plan).unwrap();
    drop(db);
    let reopened = Db::open(&path).unwrap();
    reopened.0.lock().unwrap().execute_batch(
        "CREATE TEMP TRIGGER reject_duplicate_write BEFORE INSERT ON local_playlist_import_entries
         BEGIN SELECT RAISE(ABORT, 'unexpected replay write'); END;"
    ).unwrap();
    assert_eq!(reopened.commit_spotify_import(&plan).unwrap(), receipt);
    assert_eq!(reopened.local_playlist(&receipt.playlist_id).unwrap().track_count, 1);
    plan.digest = "changed-digest".into();
    assert!(matches!(reopened.commit_spotify_import(&plan), Err(ImportError::Conflict)));
}

#[test]
fn separate_database_connections_conflict_on_same_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("concurrent-import.sqlite");
    let first_db = Arc::new(Db::open(&path).unwrap());
    let second_db = Arc::new(Db::open(&path).unwrap());
    let first_jobs = SpotifyImportJobs::new(first_db.clone());
    let second_jobs = SpotifyImportJobs::new(second_db);
    let (_, first) = review(&first_jobs, 2);
    let (_, second) = review(&second_jobs, 3);
    first_jobs.commit(&owner(), &first).unwrap();
    assert!(matches!(second_jobs.commit(&owner(), &second), Err(ImportError::Conflict)));
    assert_eq!(first_db.local_playlists().len(), 1);
}
