use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::models::{LyricsDocument, LyricsLine, LyricsTrack, LyricsWord};

static LRC_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(\d{1,3}):(\d{1,2})(?:[\.:](\d{1,3}))?\]").expect("valid LRC timestamp regex")
});
static QRC_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[(\d+),(\d+)\](.*)$").expect("valid QRC line regex"));
static QRC_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(.*?)\((\d+),(\d+)\)").expect("valid QRC word regex"));

pub fn parse_document(
    track_key: &str,
    source: &str,
    original: &str,
    translation: Option<&str>,
    romanization: Option<&str>,
) -> Result<LyricsDocument, String> {
    let original_track = parse_track(original)?;
    if original_track.lines.is_empty() {
        return Err("歌词内容中没有可用的时间轴".into());
    }
    Ok(LyricsDocument {
        track_key: track_key.into(),
        source: source.into(),
        offset_ms: embedded_offset(original),
        original: original_track,
        translation: translation.and_then(parse_optional_track),
        romanization: romanization.and_then(parse_optional_track),
    })
}

fn parse_optional_track(raw: &str) -> Option<LyricsTrack> {
    parse_track(raw)
        .ok()
        .filter(|track| !track.lines.is_empty())
}

fn parse_track(raw: &str) -> Result<LyricsTrack, String> {
    let qrc = parse_qrc(raw);
    if !qrc.is_empty() {
        return Ok(LyricsTrack { lines: finish(qrc) });
    }
    let mut grouped = BTreeMap::<u64, Vec<String>>::new();
    for line in raw.lines() {
        let timestamps = LRC_TIMESTAMP
            .captures_iter(line)
            .filter_map(|capture| timestamp_ms(&capture))
            .collect::<Vec<_>>();
        if timestamps.is_empty() {
            continue;
        }
        let text = LRC_TIMESTAMP.replace_all(line, "").trim().to_owned();
        if text.is_empty() {
            continue;
        }
        for timestamp in timestamps {
            grouped.entry(timestamp).or_default().push(text.clone());
        }
    }
    let lines = grouped
        .into_iter()
        .map(|(start_ms, values)| LyricsLine {
            text: values.first().cloned().unwrap_or_default(),
            start_ms,
            end_ms: None,
            words: None,
        })
        .collect::<Vec<_>>();
    Ok(LyricsTrack {
        lines: finish(lines),
    })
}

fn parse_qrc(raw: &str) -> Vec<LyricsLine> {
    raw.lines()
        .filter_map(|line| {
            let capture = QRC_LINE.captures(line.trim())?;
            let start_ms = capture.get(1)?.as_str().parse::<u64>().ok()?;
            let duration_ms = capture.get(2)?.as_str().parse::<u64>().ok()?;
            let content = capture.get(3)?.as_str();
            let words = QRC_WORD
                .captures_iter(content)
                .filter_map(|word| {
                    let text = word.get(1)?.as_str().to_owned();
                    let start_ms = word.get(2)?.as_str().parse::<u64>().ok()?;
                    let duration = word.get(3)?.as_str().parse::<u64>().ok()?;
                    (!text.is_empty()).then_some(LyricsWord {
                        text,
                        start_ms,
                        end_ms: start_ms.saturating_add(duration),
                    })
                })
                .collect::<Vec<_>>();
            let text = if words.is_empty() {
                content.trim().to_owned()
            } else {
                words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<String>()
            };
            (!text.is_empty()).then_some(LyricsLine {
                text,
                start_ms,
                end_ms: Some(start_ms.saturating_add(duration_ms)),
                words: (!words.is_empty()).then_some(words),
            })
        })
        .collect()
}

fn finish(mut lines: Vec<LyricsLine>) -> Vec<LyricsLine> {
    lines.sort_by_key(|line| line.start_ms);
    for index in 0..lines.len().saturating_sub(1) {
        if lines[index].end_ms.is_none() {
            lines[index].end_ms = Some(lines[index + 1].start_ms);
        }
    }
    lines
}

fn timestamp_ms(capture: &regex::Captures<'_>) -> Option<u64> {
    let minutes = capture.get(1)?.as_str().parse::<u64>().ok()?;
    let seconds = capture.get(2)?.as_str().parse::<u64>().ok()?;
    let fraction = capture.get(3).map(|value| value.as_str()).unwrap_or("0");
    let millis = match fraction.len() {
        1 => fraction.parse::<u64>().ok()?.saturating_mul(100),
        2 => fraction.parse::<u64>().ok()?.saturating_mul(10),
        _ => fraction.get(..3).unwrap_or(fraction).parse::<u64>().ok()?,
    };
    Some((minutes * 60 + seconds) * 1000 + millis)
}

fn embedded_offset(raw: &str) -> i64 {
    raw.lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("[offset:")?
                .strip_suffix(']')?
                .parse()
                .ok()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_and_qrc() {
        let lrc = parse_document(
            "track",
            "test",
            "[00:01.20]Hello\n[00:03.00]World",
            None,
            None,
        )
        .unwrap();
        assert_eq!(lrc.original.lines[0].start_ms, 1200);
        let qrc = parse_document(
            "track",
            "test",
            "[1000,2000]Hel(1000,800)lo(1800,1200)",
            None,
            None,
        )
        .unwrap();
        assert_eq!(qrc.original.lines[0].words.as_ref().unwrap().len(), 2);
    }
}
