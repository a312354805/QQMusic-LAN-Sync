#[cfg(target_os = "windows")]
mod windows_smtc;

use crate::models::{PlaybackSnapshot, PlayerCommand};

#[derive(Default)]
pub struct PlayerService {
    #[cfg(target_os = "windows")]
    windows: windows_smtc::WindowsPlayerService,
}

impl PlayerService {
    pub fn snapshot(&self) -> PlaybackSnapshot {
        #[cfg(target_os = "windows")]
        {
            self.windows
                .snapshot()
                .unwrap_or_else(|error| PlaybackSnapshot {
                    error: Some(error),
                    ..PlaybackSnapshot::default()
                })
        }
        #[cfg(not(target_os = "windows"))]
        {
            PlaybackSnapshot {
                error: Some("播放器监听当前仅支持 Windows".into()),
                ..PlaybackSnapshot::default()
            }
        }
    }

    pub fn execute(&self, command: PlayerCommand) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            self.windows.execute(command)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = command;
            Err("播放器控制当前仅支持 Windows".into())
        }
    }
}
