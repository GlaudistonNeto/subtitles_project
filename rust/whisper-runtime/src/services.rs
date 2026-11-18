use crate::Segment;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

const OLLAMA_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug)]
struct JsonResponse {
    status: u16,
    body: Result<Value, ()>,
}

fn refinement_prompt(language: &str, context: &[String], texts: &[String]) -> String {
    let data = format!(
        "CONTEXT_JSON:{}\nINPUT_JSON:{}",
        serde_json::to_string(context).expect("string arrays serialize"),
        serde_json::to_string(texts).expect("string arrays serialize")
    );
    match language {
        "ja" => format!("音声認識の誤りを修正してください。同音異義語の漢字や助詞を文脈に基づいて直してください。前のバッチは読み取り専用の文脈です。入力と同じ数の文字列を含む配列を持つ、linesのみをキーとするJSONオブジェクトを返してください。\n{data}"),
        "zh" => format!("请根据上下文纠正语音识别错误，包括同音汉字和助词。上一批内容仅供只读参考。只返回一个JSON对象，唯一键为lines，其值为字符串数组，数量必须与输入完全相同。\n{data}"),
        _ => format!("Correct speech recognition errors, including homophones and particles. The previous batch is read-only context. Return only a JSON object with the single key lines, whose value is an array of strings with exactly the same number of items as the input.\n{data}"),
    }
}

fn refinement_request(
    language: &str,
    model: &str,
    context: &[String],
    texts: &[String],
    is_last_batch: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "stream": false,
        "format": {
            "type": "object",
            "properties": { "lines": { "type": "array", "items": { "type": "string" } } },
            "required": ["lines"]
        },
        "options": { "temperature": 0 },
        "messages": [{ "role": "user", "content": refinement_prompt(language, context, texts) }]
    });
    if is_last_batch {
        body["keep_alive"] = json!(0);
    }
    body
}

fn parse_refined_lines(response: &JsonResponse, expected: usize) -> Result<Vec<String>, String> {
    if !(200..300).contains(&response.status) {
        return Err(format!("Ollama returned HTTP {}", response.status));
    }
    let body = response
        .body
        .as_ref()
        .map_err(|_| "Ollama returned invalid JSON".to_owned())?;
    let content = body
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Ollama response has no message content".to_owned())?;
    let parsed: Value = serde_json::from_str(content).map_err(|_| "Ollama returned invalid JSON".to_owned())?;
    let lines = parsed
        .get("lines")
        .and_then(Value::as_array)
        .filter(|lines| lines.len() == expected)
        .filter(|lines| lines.iter().all(Value::is_string))
        .ok_or_else(|| "Ollama returned an invalid number or type of lines".to_owned())?;
    Ok(lines
        .iter()
        .map(|line| line.as_str().expect("validated string").to_owned())
        .collect())
}

fn refine_with<F>(
    segments: &[Segment],
    source_language: &str,
    model: &str,
    mut send: F,
) -> Vec<Segment>
where
    F: FnMut(&Value) -> Result<JsonResponse, String>,
{
    let mut result = segments.to_vec();
    let mut context = Vec::new();
    for start in (0..segments.len()).step_by(8) {
        let end = (start + 8).min(segments.len());
        let texts: Vec<_> = segments[start..end]
            .iter()
            .map(|segment| segment.text.clone())
            .collect();
        let body = refinement_request(source_language, model, &context, &texts, end == segments.len());
        let refined = send(&body)
            .and_then(|response| parse_refined_lines(&response, texts.len()))
            .unwrap_or_else(|error| {
                eprintln!("Ollama refinement failed for segments {}-{}; keeping original text. {error}", start + 1, end);
                texts.clone()
            });
        for (index, text) in refined.iter().enumerate() {
            result[start + index].text.clone_from(text);
        }
        context = refined;
    }
    result
}

pub fn refine_segments(
    segments: &[Segment],
    source_language: &str,
    model: &str,
    host: &str,
) -> Vec<Segment> {
    let client = match Client::builder().timeout(OLLAMA_TIMEOUT).build() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Could not create Ollama client; keeping original text. {error}");
            return segments.to_vec();
        }
    };
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    refine_with(segments, source_language, model, |body| {
        let response = client
            .post(&url)
            .json(body)
            .send()
            .map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let body = response.json::<Value>().map_err(|_| "invalid JSON".to_owned());
        Ok(JsonResponse {
            status,
            body: body.map_err(|_| ()) ,
        })
    })
}

fn translation_request(
    segments: &[Segment],
    source_language: &str,
    start: usize,
    end: usize,
) -> Value {
    let texts: Vec<_> = segments[start..end]
        .iter()
        .map(|segment| segment.text.clone())
        .collect();
    let context = segments[start.saturating_sub(30)..start]
        .iter()
        .chain(segments[end..(end + 30).min(segments.len())].iter())
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut body = json!({ "text": texts, "target_lang": "EN-US" });
    if source_language != "auto" {
        body["source_lang"] = json!(source_language.to_uppercase());
    }
    if !context.is_empty() {
        body["context"] = json!(context);
    }
    body
}

fn parse_translations(response: JsonResponse, expected: usize) -> Result<Vec<String>, String> {
    if response.status == 456 {
        return Err("DeepL quota exceeded (HTTP 456).".to_owned());
    }
    if !(200..300).contains(&response.status) {
        return Err(format!("DeepL translation failed (HTTP {}).", response.status));
    }
    let payload = response
        .body
        .map_err(|_| "DeepL returned invalid JSON.".to_owned())?;
    let translations = payload
        .get("translations")
        .and_then(Value::as_array)
        .filter(|items| items.len() == expected)
        .filter(|items| items.iter().all(|item| item.get("text").and_then(Value::as_str).is_some()))
        .ok_or_else(|| "DeepL returned an unexpected number or format of translations.".to_owned())?;
    Ok(translations
        .iter()
        .map(|item| item["text"].as_str().expect("validated string").to_owned())
        .collect())
}

fn translate_with<F, W>(
    segments: &[Segment],
    source_language: &str,
    api_key: &str,
    mut send: F,
    mut wait: W,
) -> Result<Vec<Segment>, String>
where
    F: FnMut(&str, &Value) -> Result<JsonResponse, ()>,
    W: FnMut(Duration),
{
    let host = if api_key.ends_with(":fx") {
        "https://api-free.deepl.com"
    } else {
        "https://api.deepl.com"
    };
    let mut result = segments.to_vec();
    for start in (0..segments.len()).step_by(30) {
        let end = (start + 30).min(segments.len());
        let body = translation_request(segments, source_language, start, end);
        let mut response = None;
        for attempt in 0..3 {
            let current = send(&format!("{host}/v2/translate"), &body)
                .map_err(|_| "DeepL translation failed due to a network error.".to_owned())?;
            if (200..300).contains(&current.status) {
                response = Some(current);
                break;
            }
            if current.status == 456 {
                return Err("DeepL quota exceeded (HTTP 456).".to_owned());
            }
            if current.status != 429 && current.status < 500 {
                return Err(format!("DeepL translation failed (HTTP {}).", current.status));
            }
            if attempt == 2 {
                return Err(format!("DeepL translation failed after 3 attempts (HTTP {}).", current.status));
            }
            wait(Duration::from_millis(250 * 2_u64.pow(attempt)));
        }
        let texts = parse_translations(response.expect("successful response"), end - start)?;
        for (index, text) in texts.iter().enumerate() {
            result[start + index].text.clone_from(text);
        }
    }
    Ok(result)
}

pub fn translate_segments(
    segments: &[Segment],
    source_language: &str,
    api_key: &str,
) -> Result<Vec<Segment>, String> {
    let client = Client::builder()
        .build()
        .map_err(|_| "DeepL translation failed due to a network error.".to_owned())?;
    translate_with(
        segments,
        source_language,
        api_key,
        |url, body| {
            let response = client
                .post(url)
                .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
                .json(body)
                .send()
                .map_err(|_| ())?;
            let status = response.status().as_u16();
            let body = response.json::<Value>().map_err(|_| ())?;
            Ok(JsonResponse {
                status,
                body: Ok(body),
            })
        },
        std::thread::sleep,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        parse_refined_lines, parse_translations, refine_with, translate_with, JsonResponse,
    };
    use crate::Segment;
    use serde_json::{json, Value};
    use std::time::Duration;

    fn segments(count: usize) -> Vec<Segment> {
        (0..count)
            .map(|index| Segment {
                index: index + 1,
                start_ms: index as i64 * 1_000,
                end_ms: index as i64 * 1_000 + 500,
                text: format!("line {index}"),
            })
            .collect()
    }

    fn ollama_response(texts: &[String]) -> JsonResponse {
        JsonResponse {
            status: 200,
            body: Ok(json!({ "message": { "content": json!({ "lines": texts }).to_string() } })),
        }
    }

    fn deepl_response(texts: &[String]) -> JsonResponse {
        JsonResponse {
            status: 200,
            body: Ok(json!({ "translations": texts.iter().map(|text| json!({ "text": format!("EN {text}") })).collect::<Vec<_>>() })),
        }
    }

    #[test]
    fn refinement_batches_eight_and_carries_only_previous_results() {
        let input = segments(20);
        let mut request_sizes = Vec::new();
        let mut prompts = Vec::new();
        let result = refine_with(&input, "ja", "model", |body| {
            let prompt = body["messages"][0]["content"].as_str().unwrap().to_owned();
            let texts: Vec<String> = serde_json::from_str(
                prompt.split("INPUT_JSON:").nth(1).unwrap(),
            )
            .unwrap();
            request_sizes.push(texts.len());
            prompts.push(prompt);
            Ok(ollama_response(&texts.iter().map(|text| format!("fixed {text}")).collect::<Vec<_>>()))
        });
        assert_eq!(request_sizes, [8, 8, 4]);
        assert!(prompts[1].contains("fixed line 0"));
        assert_eq!(result[8].text, "fixed line 8");
        assert_eq!(result[8].start_ms, input[8].start_ms);
    }

    #[test]
    fn refinement_keeps_original_text_after_invalid_response() {
        let input = segments(1);
        let result = refine_with(&input, "ja", "model", |_| {
            Ok(JsonResponse { status: 200, body: Ok(json!({ "message": { "content": "not json" } })) })
        });
        assert_eq!(result[0].text, input[0].text);
        assert!(parse_refined_lines(&JsonResponse { status: 200, body: Ok(json!(["not object"])) }, 1).is_err());
    }

    #[test]
    fn translation_batches_and_preserves_surrounding_context() {
        let input = segments(65);
        let mut sizes = Vec::new();
        let mut bodies: Vec<Value> = Vec::new();
        let result = translate_with(&input, "ja", "key:fx", |url, body| {
            assert_eq!(url, "https://api-free.deepl.com/v2/translate");
            sizes.push(body["text"].as_array().unwrap().len());
            bodies.push(body.clone());
            let texts = body["text"]
                .as_array()
                .unwrap()
                .iter()
                .map(|text| text.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            Ok(deepl_response(&texts))
        }, |_| {});
        let result = result.unwrap();
        assert_eq!(sizes, [30, 30, 5]);
        assert!(bodies[1]["context"].as_str().unwrap().contains("line 0"));
        assert!(bodies[1]["context"].as_str().unwrap().contains("line 60"));
        assert_eq!(result[0].text, "EN line 0");
        assert_eq!(result[0].start_ms, input[0].start_ms);
    }

    #[test]
    fn translation_retries_rate_limits_with_exponential_delays() {
        let mut attempts = 0;
        let mut delays = Vec::new();
        let result = translate_with(&segments(1), "zh", "key", |_, _| {
            attempts += 1;
            if attempts == 1 {
                Ok(JsonResponse { status: 429, body: Err(()) })
            } else {
                Ok(deepl_response(&["translated".into()]))
            }
        }, |duration| delays.push(duration));
        assert_eq!(result.unwrap()[0].text, "EN translated");
        assert_eq!(attempts, 2);
        assert_eq!(delays, [Duration::from_millis(250)]);
    }

    #[test]
    fn translation_errors_do_not_include_the_api_key() {
        let key = "secret:fx";
        let error = translate_with(&segments(1), "ja", key, |_, _| {
            Ok(JsonResponse { status: 456, body: Err(()) })
        }, |_| {}).unwrap_err();
        assert!(error.contains("quota"));
        assert!(!error.contains(key));
        assert!(parse_translations(JsonResponse { status: 200, body: Ok(json!({ "translations": [] })) }, 1).is_err());
    }
}