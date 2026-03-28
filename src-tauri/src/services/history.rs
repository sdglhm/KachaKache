use crate::types::{TranscriptEntry, TranscriptRetention};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
    max_entries: usize,
}

impl HistoryStore {
    pub fn new(path: PathBuf, max_entries: usize) -> Self {
        Self { path, max_entries }
    }

    pub fn load(&self) -> anyhow::Result<Vec<TranscriptEntry>> {
        if !self.path.exists() {
            self.save(&[])?;
            return Ok(vec![]);
        }

        let raw = fs::read_to_string(&self.path).context("failed to read history")?;
        if raw.trim().is_empty() {
            return Ok(vec![]);
        }

        let mut entries = serde_json::from_str::<Vec<TranscriptEntry>>(&raw).unwrap_or_default();
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(entries)
    }

    pub fn load_pruned(
        &self,
        retention: &TranscriptRetention,
    ) -> anyhow::Result<Vec<TranscriptEntry>> {
        let entries = self.load()?;
        self.prune_entries(entries, retention)
    }

    pub fn save(&self, entries: &[TranscriptEntry]) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(entries).context("failed to serialize history")?;
        fs::write(&self.path, json).context("failed to write history")?;
        Ok(())
    }

    pub fn append(
        &self,
        entry: TranscriptEntry,
        retention: &TranscriptRetention,
    ) -> anyhow::Result<Vec<TranscriptEntry>> {
        let mut entries = self.load()?;
        entries.insert(0, entry);
        let entries = self.prune_entries(entries, retention)?;
        self.save(&entries)?;
        Ok(entries)
    }

    pub fn delete_entry(&self, id: &str) -> anyhow::Result<Vec<TranscriptEntry>> {
        let mut entries = self.load()?;
        entries.retain(|entry| entry.id != id);
        self.save(&entries)?;
        Ok(entries)
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        self.save(&[])
    }

    fn prune_entries(
        &self,
        mut entries: Vec<TranscriptEntry>,
        retention: &TranscriptRetention,
    ) -> anyhow::Result<Vec<TranscriptEntry>> {
        let original_len = entries.len();
        if let Some(cutoff) = retention_cutoff(retention) {
            entries.retain(|entry| {
                parse_created_at(&entry.created_at)
                    .map(|created_at| created_at >= cutoff)
                    .unwrap_or(true)
            });
        }

        if entries.len() > self.max_entries {
            entries.truncate(self.max_entries);
        }

        if entries.len() != original_len {
            self.save(&entries)?;
        } else if !self.path.exists() {
            self.save(&entries)?;
        }

        Ok(entries)
    }
}

fn retention_cutoff(retention: &TranscriptRetention) -> Option<DateTime<Utc>> {
    let now = Utc::now();
    match retention {
        TranscriptRetention::Indefinite => None,
        TranscriptRetention::NinetyDays => Some(now - Duration::days(90)),
        TranscriptRetention::ThirtyDays => Some(now - Duration::days(30)),
        TranscriptRetention::FourteenDays => Some(now - Duration::days(14)),
        TranscriptRetention::SevenDays => Some(now - Duration::days(7)),
    }
}

fn parse_created_at(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InsertionStrategy;
    use uuid::Uuid;

    fn temp_store() -> HistoryStore {
        let path = std::env::temp_dir().join(format!("kachakache-history-{}.json", Uuid::new_v4()));
        HistoryStore::new(path, 100)
    }

    fn entry(id: &str, created_at: DateTime<Utc>) -> TranscriptEntry {
        TranscriptEntry {
            id: id.to_string(),
            text: format!("entry {id}"),
            created_at: created_at.to_rfc3339(),
            model_id: "base.en".to_string(),
            duration_ms: 1000,
            inserted: true,
            insertion_strategy: InsertionStrategy::Typed,
        }
    }

    #[test]
    fn delete_entry_removes_only_requested_item() {
        let store = temp_store();
        let now = Utc::now();
        store
            .save(&[entry("one", now), entry("two", now - Duration::days(1))])
            .unwrap();

        let remaining = store.delete_entry("one").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "two");

        let _ = std::fs::remove_file(store.path);
    }

    #[test]
    fn load_pruned_respects_retention_window() {
        let store = temp_store();
        let now = Utc::now();
        store
            .save(&[
                entry("recent", now - Duration::days(5)),
                entry("old", now - Duration::days(40)),
            ])
            .unwrap();

        let remaining = store
            .load_pruned(&TranscriptRetention::ThirtyDays)
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "recent");

        let _ = std::fs::remove_file(store.path);
    }

    #[test]
    fn append_persists_entries_without_pruning() {
        let store = temp_store();
        let now = Utc::now();

        let updated = store
            .append(entry("one", now), &TranscriptRetention::Indefinite)
            .unwrap();
        assert_eq!(updated.len(), 1);

        let reloaded = store.load().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].id, "one");

        let _ = std::fs::remove_file(store.path);
    }
}
