use std::sync::LazyLock;
use std::time::Duration;

use lyrics_crypto::decrypter::qrc::decrypter::decrypt_lyrics;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use strsim::normalized_levenshtein;

use crate::models::PlaybackSnapshot;

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    data: Option<SearchData>,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    song: Option<SongData>,
}

#[derive(Debug, Deserialize)]
struct SongData {
    #[serde(default)]
    list: Vec<QqSong>,
}

#[derive(Debug, Deserialize)]
struct QqSong {
    songid: u64,
    songmid: String,
    songname: String,
    interval: Option<u64>,
    #[serde(default)]
    singer: Vec<QqSinger>,
}

#[derive(Debug, Deserialize)]
struct QqSinger {
    name: String,
}

#[derive(Debug, Deserialize)]
struct SimpleLyrics {
    lyric: Option<String>,
    trans: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RawLyrics {
    pub source: String,
    pub original: String,
    pub translation: Option<String>,
    pub romanization: Option<String>,
}

pub struct QqMusicProvider {
    client: Client,
}

impl QqMusicProvider {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 QQMusic-LAN-Sync/0.1")
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }

    pub async fn search(&self, playback: &PlaybackSnapshot) -> Result<RawLyrics, String> {
        let title = playback
            .title
            .as_deref()
            .ok_or_else(|| "当前歌曲缺少歌名".to_string())?;
        let artist = playback.artist.as_deref().unwrap_or_default();
        let mut url = reqwest::Url::parse("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
            .map_err(|error| error.to_string())?;
        url.query_pairs_mut()
            .append_pair("w", &format!("{title} {artist}"))
            .append_pair("p", "1")
            .append_pair("n", "12")
            .append_pair("format", "json");
        let envelope = self
            .client
            .get(url)
            .header("Referer", "https://y.qq.com/")
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<SearchEnvelope>()
            .await
            .map_err(|error| error.to_string())?;
        let mut songs = envelope
            .data
            .and_then(|data| data.song)
            .map(|song| song.list)
            .unwrap_or_default();
        songs.sort_by(|left, right| score(playback, right).total_cmp(&score(playback, left)));

        for song in songs.into_iter().take(5) {
            if let Ok(lyrics) = self.fetch_legacy(song.songid).await {
                if !lyrics.original.trim().is_empty() {
                    return Ok(lyrics);
                }
            }
            if let Ok(lyrics) = self.fetch_simple(&song.songmid).await {
                if !lyrics.original.trim().is_empty() {
                    return Ok(lyrics);
                }
            }
        }
        Err("QQ 音乐没有返回可用的同步歌词".into())
    }

    async fn fetch_simple(&self, song_mid: &str) -> Result<RawLyrics, String> {
        let response = self
            .client
            .get("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
            .header("Referer", "https://y.qq.com/")
            .query(&[("songmid", song_mid), ("format", "json"), ("nobase64", "1")])
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<SimpleLyrics>()
            .await
            .map_err(|error| error.to_string())?;
        Ok(RawLyrics {
            source: "QQMusic LRC".into(),
            original: response.lyric.unwrap_or_default(),
            translation: response.trans.filter(|value| !value.trim().is_empty()),
            romanization: None,
        })
    }

    async fn fetch_legacy(&self, song_id: u64) -> Result<RawLyrics, String> {
        let song_id = song_id.to_string();
        let response = self
            .client
            .post("https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg")
            .header("Referer", "https://c.y.qq.com/")
            .form(&[
                ("version", "15"),
                ("miniversion", "82"),
                ("lrctype", "4"),
                ("musicid", song_id.as_str()),
            ])
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .text()
            .await
            .map_err(|error| error.to_string())?;
        let response = response.replace("<!--", "").replace("-->", "");
        let original = extract_and_decrypt(&response, "content")?
            .ok_or_else(|| "QQ 音乐 QRC 原文解密失败".to_string())?;
        Ok(RawLyrics {
            source: "QQMusic QRC".into(),
            original,
            translation: extract_and_decrypt(&response, "contentts")?,
            romanization: extract_and_decrypt(&response, "contentroma")?,
        })
    }
}

fn score(playback: &PlaybackSnapshot, song: &QqSong) -> f64 {
    let song_artist = song
        .singer
        .iter()
        .map(|singer| singer.name.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let title = normalized_levenshtein(
        &normalize(playback.title.as_deref().unwrap_or_default()),
        &normalize(&song.songname),
    );
    let artist = normalized_levenshtein(
        &normalize(playback.artist.as_deref().unwrap_or_default()),
        &normalize(&song_artist),
    );
    let duration = match (playback.duration_ms, song.interval) {
        (Some(expected), Some(actual)) => {
            (1.0 - expected.abs_diff(actual * 1000) as f64 / 15_000.0).clamp(0.0, 1.0)
        }
        _ => 0.5,
    };
    title * 0.62 + artist * 0.28 + duration * 0.10
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn extract_and_decrypt(xml: &str, tag: &str) -> Result<Option<String>, String> {
    let pattern = Regex::new(&format!(r"(?s)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>"))
        .map_err(|error| error.to_string())?;
    let Some(value) = pattern
        .captures(xml)
        .and_then(|capture| capture.get(1))
        .map(|value| unwrap_cdata(value.as_str().trim()))
    else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    let decrypted = if is_hex(value) {
        decrypt_qrc(value)?
    } else {
        value.to_owned()
    };
    Ok(Some(extract_lyric_content(&decrypted)))
}

fn unwrap_cdata(value: &str) -> &str {
    value
        .strip_prefix("<![CDATA[")
        .and_then(|value| value.strip_suffix("]]>"))
        .map(str::trim)
        .unwrap_or(value)
}

fn is_hex(value: &str) -> bool {
    value.len() % 2 == 0 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn decrypt_qrc(value: &str) -> Result<String, String> {
    decrypt_lyrics(value).ok_or_else(|| "QQ 音乐 QRC 解密或解压失败".into())
}

static LYRIC_CONTENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"LyricContent="([^"]*)""#).expect("valid LyricContent regex"));

fn extract_lyric_content(value: &str) -> String {
    LYRIC_CONTENT
        .captures(value)
        .and_then(|capture| capture.get(1))
        .map(|content| html_escape::decode_html_entities(content.as_str()).into_owned())
        .unwrap_or_else(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{is_hex, unwrap_cdata, QqMusicProvider};

    #[test]
    fn unwraps_hex_lyrics_from_cdata() {
        let value = unwrap_cdata("<![CDATA[  9F0C07715EAB4E78  ]]>");
        assert_eq!(value, "9F0C07715EAB4E78");
        assert!(is_hex(value));
    }

    #[tokio::test]
    #[ignore = "calls the live QQ Music lyrics endpoint"]
    async fn fetches_live_qrc() {
        let lyrics = QqMusicProvider::new()
            .expect("provider")
            .fetch_legacy(9_086_138)
            .await
            .expect("QRC lyrics");
        assert!(lyrics.original.contains("其实我不快乐"));
    }
}
