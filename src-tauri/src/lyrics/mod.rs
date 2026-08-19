mod parser;
mod qqmusic;

use std::sync::Arc;

use crate::models::{LyricsDocument, PlaybackSnapshot};
use crate::storage::Storage;

pub struct LyricsService {
    provider: qqmusic::QqMusicProvider,
    storage: Arc<Storage>,
}

impl LyricsService {
    pub fn new(storage: Arc<Storage>) -> Result<Self, String> {
        Ok(Self {
            provider: qqmusic::QqMusicProvider::new()?,
            storage,
        })
    }

    pub async fn get_or_search(
        &self,
        playback: &PlaybackSnapshot,
    ) -> Result<LyricsDocument, String> {
        let track_key = playback
            .track_key
            .as_deref()
            .ok_or_else(|| "当前歌曲缺少稳定标识".to_string())?;
        if let Some(document) = self.storage.load_lyrics(track_key)? {
            return Ok(document);
        }
        let raw = self.provider.search(playback).await?;
        let document = parser::parse_document(
            track_key,
            &raw.source,
            &raw.original,
            raw.translation.as_deref(),
            raw.romanization.as_deref(),
        )?;
        self.storage.save_lyrics(&document)?;
        Ok(document)
    }
}
