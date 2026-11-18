use crate::Segment;
use regex::Regex;
use std::sync::LazyLock;

static PUNCTUATION_AND_SPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\s\p{P}]").expect("valid normalization regex"));

fn normalize_text(text: &str) -> String {
    PUNCTUATION_AND_SPACE.replace_all(text, "").into_owned()
}

pub fn collapse_repeats(segments: &[Segment]) -> Vec<Segment> {
    let normalized: Vec<_> = segments
        .iter()
        .map(|segment| normalize_text(&segment.text))
        .collect();
    let mut kept = Vec::new();
    let mut position = 0;

    while position < segments.len() {
        let mut repeat_period = 0;
        let mut repeat_comparisons = 0;
        for period in 1..=3 {
            if repeat_period != 0 {
                break;
            }
            let mut comparisons = 0;
            while position + comparisons + period < segments.len()
                && normalized[position + comparisons] == normalized[position + comparisons + period]
            {
                comparisons += 1;
            }
            if comparisons >= 4 * period {
                repeat_period = period;
                repeat_comparisons = comparisons;
            }
        }

        if repeat_period != 0 {
            kept.extend_from_slice(&segments[position..position + repeat_period]);
            position += repeat_period + repeat_comparisons;
        } else {
            kept.push(segments[position].clone());
            position += 1;
        }
    }

    renumber(kept)
}

fn renumber(mut segments: Vec<Segment>) -> Vec<Segment> {
    for (index, segment) in segments.iter_mut().enumerate() {
        segment.index = index + 1;
    }
    segments
}

fn is_cjk(character: char) -> bool {
    matches!(character as u32,
        0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff)
}

fn is_cjk_punctuation(character: char) -> bool {
    matches!(character, '、' | '。' | '，' | '！' | '？')
}

fn wrap_text(text: &str) -> Vec<String> {
    if text.chars().any(is_cjk) {
        let characters: Vec<_> = text.chars().filter(|character| !character.is_whitespace()).collect();
        let mut lines = Vec::new();
        let mut offset = 0;
        while offset < characters.len() {
            let mut length = 42.min(characters.len() - offset);
            if characters.len() - offset > 42 {
                for (index, character) in characters[offset..offset + 42].iter().enumerate() {
                    if is_cjk_punctuation(*character) {
                        length = index + 1;
                    }
                }
            }
            lines.push(characters[offset..offset + length].iter().collect());
            offset += length;
        }
        return lines;
    }

    let mut lines: Vec<String> = Vec::new();
    for word in text
        .split_whitespace()
        .flat_map(|word| word.chars().collect::<Vec<_>>().chunks(42).map(<[char]>::to_vec).collect::<Vec<_>>())
    {
        let word: String = word.into_iter().collect();
        if let Some(last) = lines.last_mut() {
            if last.chars().count() + 1 + word.chars().count() <= 42 {
                last.push(' ');
                last.push_str(&word);
                continue;
            }
        }
        lines.push(word);
    }
    lines
}

fn timestamp(milliseconds: i64) -> String {
    let value = milliseconds.max(0);
    let hours = value / 3_600_000;
    let minutes = (value % 3_600_000) / 60_000;
    let seconds = (value % 60_000) / 1_000;
    let milliseconds = value % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{milliseconds:03}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cue {
    start_ms: i64,
    end_ms: i64,
    text: String,
    split: bool,
}

pub fn generate_srt(segments: &[Segment]) -> String {
    let mut cues = Vec::new();
    for segment in segments.iter().filter(|segment| !segment.text.trim().is_empty()) {
        let lines = wrap_text(&segment.text);
        let parts: Vec<String> = lines.chunks(2).map(|pair| pair.join("\n")).collect();
        let weights: Vec<usize> = parts
            .iter()
            .map(|part| part.chars().filter(|character| *character != '\n').count().max(1))
            .collect();
        let total: usize = weights.iter().sum();
        let duration = segment.end_ms - segment.start_ms;
        let mut allocated = 0;
        for (index, text) in parts.iter().enumerate() {
            let start_ms = segment.start_ms + (duration as f64 * allocated as f64 / total as f64).round() as i64;
            allocated += weights[index];
            let end_ms = if index == parts.len() - 1 {
                segment.end_ms
            } else {
                segment.start_ms + (duration as f64 * allocated as f64 / total as f64).round() as i64
            };
            cues.push(Cue { start_ms, end_ms, text: text.clone(), split: parts.len() > 1 });
        }
    }

    cues.iter()
        .enumerate()
        .map(|(index, cue)| {
            let mut end_ms = cue.end_ms;
            if end_ms - cue.start_ms < 1_000 && !cue.split {
                end_ms = cue.start_ms + 1_000;
                if let Some(next) = cues.get(index + 1) {
                    end_ms = end_ms.min(next.start_ms).max(cue.start_ms);
                }
            }
            format!(
                "{}\n{} --> {}\n{}",
                index + 1,
                timestamp(cue.start_ms),
                timestamp(end_ms),
                cue.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::{collapse_repeats, generate_srt};
    use crate::Segment;

    fn segments(texts: &[&str]) -> Vec<Segment> {
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| Segment {
                index: index + 1,
                start_ms: index as i64 * 100,
                end_ms: index as i64 * 100 + 50,
                text: (*text).to_owned(),
            })
            .collect()
    }

    #[test]
    fn collapses_repeated_segments_without_losing_timestamps() {
        let input = segments(&["Intro", "A", "A", "A", "A", "A", "A", "A", "A", "A", "A"]);
        let result = collapse_repeats(&input);
        assert_eq!(result.iter().map(|segment| segment.text.as_str()).collect::<Vec<_>>(), ["Intro", "A"]);
        assert_eq!(result[1].start_ms, 100);
        assert_eq!(result[1].index, 2);
    }

    #[test]
    fn wraps_english_at_42_characters_and_keeps_two_lines_per_cue() {
        let srt = generate_srt(&[Segment {
            index: 1,
            start_ms: 0,
            end_ms: 2_000,
            text: "one two three four five six seven eight nine ten eleven twelve".into(),
        }]);
        assert!(srt.split('\n').skip(2).all(|line| line.chars().count() <= 42));
    }

    #[test]
    fn wraps_cjk_at_punctuation_when_possible() {
        let text = format!("{}、{}", "あ".repeat(41), "い".repeat(4));
        let srt = generate_srt(&[Segment { index: 1, start_ms: 0, end_ms: 2_000, text }]);
        let lines: Vec<_> = srt.lines().skip(2).collect();
        assert_eq!(lines, [format!("{}、", "あ".repeat(41)), "い".repeat(4)]);
    }

    #[test]
    fn splits_long_segments_into_sequential_cues_with_proportional_timestamps() {
        let srt = generate_srt(&[Segment {
            index: 1,
            start_ms: 5_000,
            end_ms: 15_000,
            text: "word ".repeat(20),
        }]);
        let cues: Vec<_> = srt.split("\n\n").collect();
        assert_eq!(cues.len(), 2);
        assert!(cues[0].contains("00:00:05,000 --> 00:00:13,041"));
        assert!(cues[1].starts_with("2\n00:00:13,041 --> 00:00:15,000\n"));
    }

    #[test]
    fn preserves_start_times_and_skips_empty_text() {
        let srt = generate_srt(&[
            Segment { index: 1, start_ms: 1_000, end_ms: 2_000, text: " ".into() },
            Segment { index: 2, start_ms: 5_000, end_ms: 5_600, text: "Speech begins".into() },
        ]);
        assert_eq!(srt, "1\n00:00:05,000 --> 00:00:06,000\nSpeech begins");
    }
}