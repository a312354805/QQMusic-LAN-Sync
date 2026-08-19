use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::models::LyricsDocument;

pub struct Storage {
    connection: Mutex<Connection>,
}

impl Storage {
    pub fn open(app_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&app_dir).map_err(|error| error.to_string())?;
        let connection = Connection::open(app_dir.join("qqmusic-lan-sync.db"))
            .map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS lyrics (
                    track_key TEXT PRIMARY KEY,
                    document_json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );",
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn load_lyrics(&self, track_key: &str) -> Result<Option<LyricsDocument>, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let raw = connection
            .query_row(
                "SELECT document_json FROM lyrics WHERE track_key = ?1",
                params![track_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        raw.map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn save_lyrics(&self, document: &LyricsDocument) -> Result<(), String> {
        let raw = serde_json::to_string(document).map_err(|error| error.to_string())?;
        self.connection
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .execute(
                "INSERT INTO lyrics (track_key, document_json, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(track_key) DO UPDATE SET
                   document_json = excluded.document_json,
                   updated_at_ms = excluded.updated_at_ms",
                params![document.track_key, raw, crate::models::now_ms()],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}
