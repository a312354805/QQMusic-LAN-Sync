# QQMusic LAN Sync

Windows 局域网 QQ 音乐状态、歌词与播放控制同步客户端。一个安装包支持主机和客户端两种角色。

## Development

```powershell
nvm use 22.23.1
pnpm install
pnpm dev
```

Tauri 构建还需要 Rust stable 和 Windows 10/11 SDK：

```powershell
pnpm tauri dev
```

## Ports

- UDP `17635`: 主机自动发现
- TCP `17636`: WebSocket 状态同步

## Source references

- Lyrics Plus: MIT licensed UI and playback synchronization reference
- 163MusicLyrics: Apache-2.0 licensed QQ Music lyrics API and parsing reference
