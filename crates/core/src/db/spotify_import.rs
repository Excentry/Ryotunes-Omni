use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::{now_secs, Db};
use crate::spotify_import::{ImportError, ImportPlan, ImportReceipt, ImportResult, ImportTarget};

#[cfg(test)]
mod tests;

pub(super) fn initialize(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS local_playlist_imports (
            playlist_id TEXT PRIMARY KEY,
            account_key TEXT NOT NULL,
            source_playlist_id TEXT NOT NULL,
            snapshot_id TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(typeof(revision) = 'integer' AND revision > 0),
            UNIQUE(account_key, source_playlist_id)
        );
        CREATE TABLE IF NOT EXISTS local_playlist_import_entries (
            playlist_id TEXT NOT NULL,
            entry_id TEXT NOT NULL,
            source_ordinal INTEGER NOT NULL CHECK(source_ordinal >= 0),
            position INTEGER NOT NULL CHECK(position >= 0),
            video_id TEXT NOT NULL,
            song_json TEXT NOT NULL,
            PRIMARY KEY(playlist_id, entry_id),
            UNIQUE(playlist_id, position)
        );
        CREATE INDEX IF NOT EXISTS local_playlist_import_video
            ON local_playlist_import_entries(video_id);
        CREATE TABLE IF NOT EXISTS local_playlist_import_receipts (
            job_id TEXT PRIMARY KEY,
            account_key TEXT NOT NULL,
            source_playlist_id TEXT NOT NULL,
            digest TEXT NOT NULL,
            receipt_json TEXT NOT NULL
        );",
    )?;
    transaction.commit()
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPlaylistEntry {
    pub entry_id: String,
    pub source_ordinal: usize,
    pub position: usize,
    pub video_id: String,
    pub song_json: String,
}

fn unsigned_column<T: TryFrom<i64>>(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<T> {
    let value: i64 = row.get(index)?;
    T::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn write_positions(
    transaction: &rusqlite::Transaction<'_>,
    playlist_id: &str,
    order: &[String],
) -> ImportResult<()> {
    let maximum: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(position), -1) FROM local_playlist_import_entries WHERE playlist_id = ?1",
        [playlist_id], |row| row.get(0),
    )?;
    let offset = maximum.checked_add(1).ok_or(ImportError::Conflict)?;
    maximum.checked_add(offset).ok_or(ImportError::Conflict)?;
    for entry_id in order {
        transaction.execute(
            "UPDATE local_playlist_import_entries SET position = position + ?3 WHERE playlist_id = ?1 AND entry_id = ?2",
            params![playlist_id, entry_id, offset],
        )?;
    }
    for (position, entry_id) in order.iter().enumerate() {
        transaction.execute(
            "UPDATE local_playlist_import_entries SET position = ?3 WHERE playlist_id = ?1 AND entry_id = ?2",
            params![playlist_id, entry_id, position as i64],
        )?;
    }
    Ok(())
}

impl Db {
    pub fn spotify_import_target(
        &self,
        account_key: &str,
        source_id: &str,
    ) -> ImportResult<ImportTarget> {
        let connection = self.0.lock().map_err(|_| ImportError::Unavailable)?;
        let result = connection
            .query_row(
                "SELECT i.playlist_id, i.revision FROM local_playlist_imports i
             JOIN local_playlists p ON p.id = i.playlist_id
             WHERE i.account_key = ?1 AND i.source_playlist_id = ?2",
                params![account_key, source_id],
                |row| {
                    Ok(ImportTarget {
                        playlist_id: Some(row.get(0)?),
                        revision: Some(unsigned_column(row, 1)?),
                    })
                },
            )
            .optional()?;
        Ok(result.unwrap_or(ImportTarget { playlist_id: None, revision: None }))
    }

    pub fn spotify_import_receipt(
        &self,
        account_key: &str,
        job_id: &str,
    ) -> ImportResult<Option<ImportReceipt>> {
        let connection = self.0.lock().map_err(|_| ImportError::Unavailable)?;
        let json: Option<String> = connection.query_row(
            "SELECT receipt_json FROM local_playlist_import_receipts WHERE account_key = ?1 AND job_id = ?2",
            params![account_key, job_id], |row| row.get(0),
        ).optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(ImportError::from)).transpose()
    }

    pub(crate) fn commit_spotify_import(&self, plan: &ImportPlan) -> ImportResult<ImportReceipt> {
        let mut connection = self.0.lock().map_err(|_| ImportError::Unavailable)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<(String, String, String)> = transaction.query_row(
            "SELECT account_key, digest, receipt_json FROM local_playlist_import_receipts WHERE job_id = ?1",
            [&plan.job_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional()?;
        if let Some((account, digest, json)) = previous {
            if account != plan.source.account_key || digest != plan.digest {
                return Err(ImportError::Conflict);
            }
            return Ok(serde_json::from_str(&json)?);
        }
        let current: Option<(String, u64)> = transaction
            .query_row(
                "SELECT i.playlist_id, i.revision FROM local_playlist_imports i
             JOIN local_playlists p ON p.id = i.playlist_id
             WHERE i.account_key = ?1 AND i.source_playlist_id = ?2",
                params![plan.source.account_key, plan.source.playlist_id],
                |row| Ok((row.get(0)?, unsigned_column(row, 1)?)),
            )
            .optional()?;
        let now = now_secs();
        let (playlist_id, revision) = match (
            &plan.target.playlist_id,
            plan.target.revision,
            current,
        ) {
            (None, None, None) => {
                let id = format!("RYOTUNES_LOCAL_PLAYLIST:import-{:032x}", rand::random::<u128>());
                transaction.execute(
                    "INSERT INTO local_playlists(id, title, created_at, updated_at) VALUES(?1, ?2, ?3, ?3)",
                    params![id, plan.title, now],
                )?;
                transaction.execute(
                    "INSERT INTO local_playlist_imports(playlist_id, account_key, source_playlist_id, snapshot_id, revision)
                     VALUES(?1, ?2, ?3, ?4, 1)",
                    params![id, plan.source.account_key, plan.source.playlist_id, plan.source.snapshot_id],
                )?;
                (id, 1)
            }
            (Some(expected_id), Some(expected_revision), Some((id, revision)))
                if *expected_id == id && expected_revision == revision =>
            {
                let next = revision
                    .checked_add(1)
                    .filter(|value| *value <= i64::MAX as u64)
                    .ok_or(ImportError::Conflict)?;
                transaction.execute(
                    "UPDATE local_playlist_imports SET snapshot_id = ?2, revision = ?3 WHERE playlist_id = ?1",
                    params![id, plan.source.snapshot_id, next as i64],
                )?;
                transaction.execute(
                    "UPDATE local_playlists SET updated_at = ?2 WHERE id = ?1",
                    params![id, now],
                )?;
                transaction.execute(
                    "DELETE FROM local_playlist_import_entries WHERE playlist_id = ?1",
                    [&id],
                )?;
                (id, next)
            }
            _ => return Err(ImportError::Conflict),
        };
        for (position, entry) in plan.entries.iter().enumerate() {
            transaction.execute(
                "INSERT INTO local_playlist_import_entries(playlist_id, entry_id, source_ordinal, position, video_id, song_json)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![playlist_id, entry.entry_id, entry.source_ordinal as i64, position as i64, entry.video_id, entry.song_json],
            )?;
        }
        let receipt = ImportReceipt {
            job_id: plan.job_id.clone(),
            playlist_id,
            playlist_revision: revision,
            total: plan.total,
            accepted: plan.entries.len(),
            skipped: plan.total - plan.entries.len(),
        };
        transaction.execute(
            "INSERT INTO local_playlist_import_receipts(job_id, account_key, source_playlist_id, digest, receipt_json)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![plan.job_id, plan.source.account_key, plan.source.playlist_id, plan.digest, serde_json::to_string(&receipt)?],
        )?;
        transaction.commit()?;
        Ok(receipt)
    }

    pub fn imported_playlist_entries(
        &self,
        playlist_id: &str,
    ) -> ImportResult<Vec<ImportedPlaylistEntry>> {
        let connection = self.0.lock().map_err(|_| ImportError::Unavailable)?;
        let mut statement = connection.prepare(
            "SELECT entry_id, source_ordinal, position, video_id, song_json
             FROM local_playlist_import_entries WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let entries = statement
            .query_map([playlist_id], |row| {
                Ok(ImportedPlaylistEntry {
                    entry_id: row.get(0)?,
                    source_ordinal: unsigned_column(row, 1)?,
                    position: unsigned_column(row, 2)?,
                    video_id: row.get(3)?,
                    song_json: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn remove_imported_playlist_entry(
        &self,
        playlist_id: &str,
        entry_id: &str,
        expected_revision: u64,
    ) -> ImportResult<u64> {
        let mut connection = self.0.lock().map_err(|_| ImportError::Unavailable)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision: Option<u64> = transaction
            .query_row(
                "SELECT revision FROM local_playlist_imports WHERE playlist_id = ?1",
                [playlist_id],
                |row| unsigned_column(row, 0),
            )
            .optional()?;
        if revision != Some(expected_revision) {
            return Err(ImportError::Conflict);
        }
        let next = expected_revision
            .checked_add(1)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or(ImportError::Conflict)?;
        if transaction.execute(
            "DELETE FROM local_playlist_import_entries WHERE playlist_id = ?1 AND entry_id = ?2",
            params![playlist_id, entry_id],
        )? != 1
        {
            return Err(ImportError::NotFound);
        }
        let order = transaction.prepare(
            "SELECT entry_id FROM local_playlist_import_entries WHERE playlist_id = ?1 ORDER BY position",
        )?.query_map([playlist_id], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        write_positions(&transaction, playlist_id, &order)?;
        transaction.execute(
            "UPDATE local_playlist_imports SET revision = ?2 WHERE playlist_id = ?1",
            params![playlist_id, next as i64],
        )?;
        transaction.execute(
            "UPDATE local_playlists SET updated_at = ?2 WHERE id = ?1",
            params![playlist_id, now_secs()],
        )?;
        transaction.commit()?;
        Ok(next)
    }

    pub fn reorder_imported_playlist_entries(
        &self,
        playlist_id: &str,
        order: &[String],
        expected_revision: u64,
    ) -> ImportResult<u64> {
        let mut connection = self.0.lock().map_err(|_| ImportError::Unavailable)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision: Option<u64> = transaction
            .query_row(
                "SELECT revision FROM local_playlist_imports WHERE playlist_id = ?1",
                [playlist_id],
                |row| unsigned_column(row, 0),
            )
            .optional()?;
        if revision != Some(expected_revision) {
            return Err(ImportError::Conflict);
        }
        let current = transaction
            .prepare("SELECT entry_id FROM local_playlist_import_entries WHERE playlist_id = ?1")?
            .query_map([playlist_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let requested: std::collections::HashSet<_> = order.iter().collect();
        if current.len() != order.len()
            || requested.len() != order.len()
            || current.iter().any(|id| !requested.contains(id))
        {
            return Err(ImportError::Invalid);
        }
        let next = expected_revision
            .checked_add(1)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or(ImportError::Conflict)?;
        write_positions(&transaction, playlist_id, order)?;
        transaction.execute(
            "UPDATE local_playlist_imports SET revision = ?2 WHERE playlist_id = ?1",
            params![playlist_id, next as i64],
        )?;
        transaction.execute(
            "UPDATE local_playlists SET updated_at = ?2 WHERE id = ?1",
            params![playlist_id, now_secs()],
        )?;
        transaction.commit()?;
        Ok(next)
    }
}
