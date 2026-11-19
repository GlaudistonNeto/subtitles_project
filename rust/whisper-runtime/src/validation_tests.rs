#[cfg(test)]
mod validation_tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::collections::HashMap;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Utterance {
        start: f64,
        end: f64,
        text: String,
    }

    #[derive(Debug, Deserialize)]
    struct ReferenceFile {
        utterances: Vec<Utterance>,
    }

    #[derive(Debug)]
    struct Cue {
        start: f64,
        end: f64,
        text: String,
    }

    // Helper to convert "HH:MM:SS.mmm" or "MM:SS.mmm" to seconds
    fn to_seconds(timestamp: &str) -> f64 {
        let parts: Vec<&str> = timestamp.replace(',', ".").split(':').collect();
        let mut seconds = 0.0;
        let mut current = parts.iter().rev();
        
        let mut count = 0;
        while let Some(part) = current.next() {
            match count {
                0 => seconds += part.parse::<f64>().unwrap_or(0.0),
                1 => seconds += part.parse::<f64>().unwrap_or(0.0) * 60.0,
                2 => seconds += part.parse::<f64>().unwrap_or(0.0) * 3600.0,
                _ => break,
            }
            count += 1;
        }
        seconds
    }

    fn parse_vtt(contents: &str) -> Vec<Cue> {
        let blocks: Vec<&str> = contents.replace("\uFEFF", "").split("\r\n\r\n").collect();
        let mut cues = Vec::new();
        for block in blocks {
            let lines: Vec<&str> = block.split("\r\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if let Some(time_index) = lines.iter().position(|l| l.contains("-->")) {
                let time_line = lines[time_index];
                let parts: Vec<&str> = time_line.split("-->").collect();
                if parts.len() == 2 {
                    let start_str = parts[0].trim().split_whitespace().next().unwrap_or("");
                    let end_str = parts[1].trim().split_whitespace().next().unwrap_or("");
                    let text = lines[time_index + 1..].join(" ").replace("<[^>]*>", "").trim().to_string();
                    if !text.is_empty() {
                        cues.push(Cue {
                            start: to_seconds(start_str),
                            end: to_seconds(end_str),
                            text,
                        });
                    }
                }
            }
        }
        cues
    }

    fn normalize(text: &str) -> Vec<String> {
        let cleaned = text.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'')
            .collect::<String>();
        cleaned.split_whitespace().map(|s| s.to_string()).collect()
    }

    fn word_error_rate(actual: &[String], expected: &[String]) -> f64 {
        if expected.is_empty() {
            return if actual.is_empty() { 0.0 } else { 1.0 };
        }
        let mut prev = (0..=expected.len()).collect::<Vec<usize>>();
        for (i, &act_word) in actual.iter().enumerate() {
            let mut curr = vec![i + 1];
            for j in 1..=expected.len() {
                curr.push(std::cmp::min(
                    curr[j - 1] + 1,
                    prev[j] + 1,
                    prev[j - 1] + if act_word == &expected[j - 1] { 0 } else { 1 },
                ));
            }
            prev = curr;
        }
        prev[expected.len()] as f64 / expected.len() as f64
    }

    #[test]
    fn test_subtitle_logic() {
        // Logic to replicate the checks in legenda-video.spec.ts
        // Since this is a test, we'll assume paths are provided via env or a known config
        let vtt_path = std::env::var("VTT_PATH").expect("VTT_PATH not set");
        let ref_json_path = std::env::var("REFERENCE_JSON_PATH").expect("REFERENCE_JSON_PATH not set");

        let vtt_content = fs::read_to_string(vtt_path).expect("Failed to read VTT");
        let cues = parse_vtt(&vtt_content);
        
        let ref_content = fs::read_to_string(ref_json_path).expect("Failed to read reference JSON");
        let reference: ReferenceFile = serde_json::from_str(&ref_content).expect("Failed to parse reference JSON");
        
        let mut utterances = reference.utterances;
        utterances.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

        let mut issues = Vec::new();
        let min_start_to_caption_end = 1.0;
        let max_start_to_caption_end = 3.0;
        let max_text_error_rate = 0.25;

        for (index, cue) in cues.iter().enumerate() {
            // Find matching utterances
            let nearby: Vec<&Utterance> = utterances.iter()
                .filter(|item| item.end >= cue.start - max_start_to_caption_end && 
                               item.start <= cue.end + max_start_to_caption_end)
                .collect();

            let matches = if !nearby.is_empty() {
                nearby
            } else {
                let nearest = utterances.iter()
                    .min_by(|a, b| {
                        let dist_a = (a.start - cue.start).abs();
                        let dist_b = (b.start - cue.start).abs();
                        dist_a.partial_cmp(&dist_b).unwrap()
                    });
                match nearest {
                    Some(u) => vec![u],
                    None => {
                        issues.push(format!("Legenda {}: não foi encontrada fala correspondente na referência.", index + 1));
                        continue;
                    }
                }
            };

            let speech_start = matches.iter().map(|m| m.start).fold(f64::INFINITY, f64::min);
            let elapsed_to_caption_end = cue.end - speech_start;

            if elapsed_to_caption_end < min_start_to_caption_end || elapsed_to_caption_end > max_start_to_caption_end {
                issues.push(format!(
                    "Legenda {} ({:.2}–{:.2}s): {:.2}s entre o início da fala e o fim da legenda; esperado de {} a {}s.",
                    index + 1, cue.start, cue.end, elapsed_to_caption_end, min_start_to_caption_end, max_start_to_caption_end
                ));
            }

            let reference_text = matches.iter().map(|m| m.text.as_str()).collect::<Vec<_>>().join(" ");
            let caption_words = normalize(&cue.text);
            let reference_words = normalize(&reference_text);

            let mut reference_counts = HashMap::new();
            for word in &reference_words {
                *reference_counts.entry(word).or_insert(0) += 1;
            }

            let mut hallucinated = Vec::new();
            for word in &caption_words {
                let count = *reference_counts.get(word).unwrap_or(&0);
                if count == 0 {
                    hallucinated.push(word);
                } else {
                    reference_counts.insert(word, count - 1);
                }
            }

            if !hallucinated.is_empty() {
                issues.push(format!(
                    "Legenda {}: palavras sem correspondência na fala de referência (possível alucinação): {}.",
                    index + 1, hallucinated.join(", ")
                ));
            }

            let error_rate = word_error_rate(&caption_words, &reference_words);
            if error_rate > max_text_error_rate {
                issues.push(format!(
                    "Legenda {}: texto diferente da fala de referência (taxa de erro {:.2}; limite {}). Legenda: \"{}\" | Referência: \"{}\".",
                    index + 1, error_rate, max_text_error_rate, cue.text, reference_text
                ));
            }
        }

        assert!(issues.is_empty(), "Problemas encontrados:\n{}", issues.join("\n"));
    }
}
