use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

use crate::models::{now_ms, PlaybackCapabilities, PlaybackSnapshot, PlayerCommand};

const SESSION_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const METADATA_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const POSITION_RESET_TOLERANCE_MS: u64 = 2_000;

#[derive(Default)]
pub struct WindowsPlayerService {
    state: Mutex<PlayerState>,
}

#[derive(Default)]
struct PlayerState {
    manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
    session: Option<GlobalSystemMediaTransportControlsSession>,
    source_app: Option<String>,
    metadata: CachedMetadata,
    last_session_scan: Option<Instant>,
    last_metadata_refresh: Option<Instant>,
    last_position_ms: Option<u64>,
    last_duration_ms: Option<u64>,
}

#[derive(Clone, Default)]
struct CachedMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

impl WindowsPlayerService {
    pub fn snapshot(&self) -> Result<PlaybackSnapshot, String> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshot()
    }

    pub fn execute(&self, command: PlayerCommand) -> Result<(), String> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let session = state.session()?;
        let result = match command {
            PlayerCommand::TogglePlayPause => session.TryTogglePlayPauseAsync(),
            PlayerCommand::Previous => session.TrySkipPreviousAsync(),
            PlayerCommand::Next => session.TrySkipNextAsync(),
        }
        .map_err(error_string)
        .and_then(|operation| operation.get().map_err(error_string));

        match result {
            Ok(true) => {
                if matches!(command, PlayerCommand::Previous | PlayerCommand::Next) {
                    state.mark_metadata_dirty();
                }
                Ok(())
            }
            Ok(false) => Err("QQ 音乐拒绝了当前媒体控制命令".into()),
            Err(error) => {
                state.invalidate_session();
                Err(error)
            }
        }
    }
}

impl PlayerState {
    fn snapshot(&mut self) -> Result<PlaybackSnapshot, String> {
        let session = self.session()?;
        let source_app = self.source_app.clone();
        let playback = session.GetPlaybackInfo().map_err(|error| {
            self.invalidate_session();
            error_string(error)
        })?;
        let timeline = session.GetTimelineProperties().map_err(|error| {
            self.invalidate_session();
            error_string(error)
        })?;
        let controls = playback.Controls().map_err(error_string)?;

        let position_ms = ticks_to_ms(timeline.Position().map_err(error_string)?.Duration);
        let start_ticks = timeline.StartTime().map_err(error_string)?.Duration;
        let end_ticks = timeline.EndTime().map_err(error_string)?.Duration;
        let duration_ms = ticks_to_ms(end_ticks.saturating_sub(start_ticks));
        let metadata_age = self.last_metadata_refresh.map(|time| time.elapsed());
        let refresh_metadata = should_refresh_metadata(
            self.metadata.title.is_some(),
            metadata_age,
            self.last_position_ms,
            position_ms,
            self.last_duration_ms,
            duration_ms,
        );

        if refresh_metadata {
            self.refresh_metadata(&session)?;
        }

        self.last_position_ms = position_ms;
        self.last_duration_ms = duration_ms;
        let playing = playback.PlaybackStatus().map_err(error_string)?
            == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
        let track_key = self.metadata.title.as_ref().map(|title| {
            format!(
                "smtc:{}|{}|{}",
                normalize(title),
                normalize(self.metadata.artist.as_deref().unwrap_or_default()),
                duration_ms.unwrap_or_default()
            )
        });

        Ok(PlaybackSnapshot {
            sequence: 0,
            track_key,
            title: self.metadata.title.clone(),
            artist: self.metadata.artist.clone(),
            album: self.metadata.album.clone(),
            cover_url: None,
            source_app,
            duration_ms,
            position_ms,
            observed_at_ms: now_ms(),
            playing,
            capabilities: PlaybackCapabilities {
                play_pause: controls.IsPlayPauseToggleEnabled().unwrap_or(false)
                    || controls.IsPlayEnabled().unwrap_or(false)
                    || controls.IsPauseEnabled().unwrap_or(false),
                previous: controls.IsPreviousEnabled().unwrap_or(false),
                next: controls.IsNextEnabled().unwrap_or(false),
                seek: controls.IsPlaybackPositionEnabled().unwrap_or(false),
            },
            error: None,
        })
    }

    fn manager(&mut self) -> Result<GlobalSystemMediaTransportControlsSessionManager, String> {
        if let Some(manager) = &self.manager {
            return Ok(manager.clone());
        }
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(error_string)?
            .get()
            .map_err(error_string)?;
        self.manager = Some(manager.clone());
        Ok(manager)
    }

    fn session(&mut self) -> Result<GlobalSystemMediaTransportControlsSession, String> {
        if let Some(session) = &self.session {
            return Ok(session.clone());
        }
        if self
            .last_session_scan
            .is_some_and(|time| time.elapsed() < SESSION_RETRY_INTERVAL)
        {
            return Err(no_session_error());
        }

        self.last_session_scan = Some(Instant::now());
        let manager = self.manager()?;
        let sessions = manager.GetSessions().map_err(|error| {
            self.manager = None;
            error_string(error)
        })?;
        for index in 0..sessions.Size().map_err(error_string)? {
            let session = sessions.GetAt(index).map_err(error_string)?;
            let source = session
                .SourceAppUserModelId()
                .map_err(error_string)?
                .to_string();
            if is_qq_music_source(&source) {
                self.session = Some(session.clone());
                self.source_app = Some(source);
                self.metadata = CachedMetadata::default();
                self.last_metadata_refresh = None;
                self.last_position_ms = None;
                self.last_duration_ms = None;
                return Ok(session);
            }
        }
        Err(no_session_error())
    }

    fn refresh_metadata(
        &mut self,
        session: &GlobalSystemMediaTransportControlsSession,
    ) -> Result<(), String> {
        let media = session
            .TryGetMediaPropertiesAsync()
            .map_err(error_string)?
            .get()
            .map_err(error_string)?;
        self.metadata = CachedMetadata {
            title: non_empty(media.Title().map_err(error_string)?.to_string()),
            artist: non_empty(media.Artist().map_err(error_string)?.to_string()),
            album: non_empty(media.AlbumTitle().map_err(error_string)?.to_string()),
        };
        self.last_metadata_refresh = Some(Instant::now());
        Ok(())
    }

    fn mark_metadata_dirty(&mut self) {
        self.last_metadata_refresh = None;
        self.last_position_ms = None;
        self.last_duration_ms = None;
    }

    fn invalidate_session(&mut self) {
        self.session = None;
        self.source_app = None;
        self.metadata = CachedMetadata::default();
        self.last_session_scan = None;
        self.mark_metadata_dirty();
    }
}

fn should_refresh_metadata(
    has_metadata: bool,
    metadata_age: Option<Duration>,
    previous_position_ms: Option<u64>,
    position_ms: Option<u64>,
    previous_duration_ms: Option<u64>,
    duration_ms: Option<u64>,
) -> bool {
    if !has_metadata
        || metadata_age.is_none_or(|age| age >= METADATA_REFRESH_INTERVAL)
        || previous_duration_ms
            .zip(duration_ms)
            .is_some_and(|(previous, current)| previous != current)
    {
        return true;
    }

    previous_position_ms
        .zip(position_ms)
        .is_some_and(|(previous, current)| {
            current.saturating_add(POSITION_RESET_TOLERANCE_MS) < previous
        })
}

fn is_qq_music_source(source: &str) -> bool {
    let source = source.to_ascii_lowercase();
    source.contains("qqmusic") || source.contains("qq music") || source.contains("tencent.qq")
}

fn no_session_error() -> String {
    "没有检测到 QQ 音乐的 Windows 媒体会话，请确认 QQ 音乐正在运行并播放过歌曲".into()
}

fn ticks_to_ms(ticks: i64) -> Option<u64> {
    (ticks >= 0).then_some((ticks / 10_000) as u64)
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn error_string(error: windows::core::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refreshes_metadata_for_initial_and_periodic_reads() {
        assert!(should_refresh_metadata(false, None, None, None, None, None));
        assert!(should_refresh_metadata(
            true,
            Some(METADATA_REFRESH_INTERVAL),
            Some(2_000),
            Some(2_500),
            Some(180_000),
            Some(180_000),
        ));
    }

    #[test]
    fn refreshes_metadata_when_track_timeline_changes() {
        assert!(should_refresh_metadata(
            true,
            Some(Duration::from_secs(1)),
            Some(120_000),
            Some(500),
            Some(180_000),
            Some(180_000),
        ));
        assert!(should_refresh_metadata(
            true,
            Some(Duration::from_secs(1)),
            Some(30_000),
            Some(31_000),
            Some(180_000),
            Some(220_000),
        ));
    }

    #[test]
    fn keeps_cached_metadata_during_normal_playback() {
        assert!(!should_refresh_metadata(
            true,
            Some(Duration::from_secs(1)),
            Some(30_000),
            Some(31_000),
            Some(180_000),
            Some(180_000),
        ));
    }
}
