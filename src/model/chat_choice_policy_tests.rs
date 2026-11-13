//! Request-policy regressions through finite HTTP fixtures, never a real model.
use super::*;
use crate::languages::llm::LlmCompletionProvider;
use std::{net::TcpListener, thread};

fn input(language: &str, seed: &str, phrase: &str) -> PredictionInput {
    PredictionInput {
        language_id: language.into(),
        seed_text: seed.into(),
        normalized_phrase: phrase.into(),
        context_before_cursor: String::new(),
        confidence: 1.0,
        degraded: false,
    }
}

fn choice(content: &str) -> Value {
    json!({"message":{"content":content},"finish_reason":"stop"})
}

fn body(content: &str, ollama: bool) -> String {
    if ollama {
        json!({"done":true,"done_reason":"stop","message":{"content":content}}).to_string()
    } else {
        json!({"choices":[choice(content)]}).to_string()
    }
}

fn fetch(
    input: &PredictionInput,
    body: String,
    legacy: bool,
    ollama: bool,
    limits: Option<crate::prediction::PredictionLimits>,
) -> Result<Vec<PredictionCandidate>, PredictionError> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let path = if ollama {
        "/api/chat"
    } else {
        "/v1/chat/completions"
    };
    let endpoint = format!("http://{}{path}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        if ollama {
            let (mut stream, _) = test_server::accept(&listener);
            let wire = test_server::read_request(&mut stream).unwrap();
            assert!(wire.starts_with("GET /api/tags "));
            assert!(!wire.to_ascii_lowercase().contains("\r\nauthorization:"));
            let inventory = json!({"models":[{"name":"synthetic-choice-policy"}]}).to_string();
            test_server::respond(&mut stream, "200 OK", &inventory).unwrap();
        }
        let (mut stream, _) = test_server::accept(&listener);
        let wire = test_server::read_request(&mut stream).unwrap();
        assert!(wire.starts_with(&format!("POST {path} ")));
        assert!(!wire.to_ascii_lowercase().contains("\r\nauthorization:"));
        test_server::respond(&mut stream, "200 OK", &body).unwrap();
    });
    let provider = HttpModelProvider::new(ModelProviderConfig {
        protocol: if ollama {
            ModelProtocol::Ollama
        } else {
            ModelProtocol::ChatCompletions
        },
        endpoint,
        model: "synthetic-choice-policy".into(),
        ..Default::default()
    });
    let result = if legacy {
        assert!(limits.is_none());
        provider
            .generate_checked(input)
            .map(|rows| rows.into_iter().map(Into::into).collect())
    } else {
        let mut request = PredictionRequest::new(23, input.clone());
        if let Some(limits) = limits {
            request.limits = limits;
        }
        provider
            .predict(&request, &PredictionCancellation::default())
            .map(|response| {
                response.validate_for(&request).unwrap();
                response.candidates
            })
    };
    server.join().unwrap();
    result
}

fn cases() -> [PredictionInput; 3] {
    [
        input("en", "hel", "hel"),
        input("zh-Hans", "nihao", "你好"),
        input("ja", "nihongo", "日本語"),
    ]
}

fn rejected(input: &PredictionInput) -> Vec<String> {
    vec![
        input.normalized_phrase.clone(),
        "unrelated response".into(),
        format!(
            "{}{{\"local_conversion\":1,\"raw_composition\":2}}",
            input.normalized_phrase
        ),
    ]
}

fn expected(input: &PredictionInput) -> Vec<String> {
    match input.language_id.as_str() {
        "en" => [
            "hello",
            "help",
            "helmet",
            "hello world",
            "hello there",
            "hello friend",
        ],
        "zh-Hans" => [
            "你好呀",
            "你好啊",
            "你好吗",
            "你好，很高兴认识你。",
            "你好，请多指教。",
            "你好，我们开始吧。",
        ],
        "ja" => [
            "日本語入力",
            "日本語変換",
            "日本語学習",
            "日本語で入力しています。",
            "日本語を勉強しています。",
            "日本語で確認します。",
        ],
        _ => unreachable!(),
    }
    .map(String::from)
    .to_vec()
}

#[test]
fn unusable_structured_choices_do_not_hide_valid_siblings_after_request_filtering() {
    for input in cases() {
        let text = expected(&input).remove(0);
        assert!(useful_candidate(&text, &input));
        assert!(
            rejected(&input)
                .iter()
                .all(|text| !useful_candidate(text, &input))
        );
        let bad = json!({"candidates":rejected(&input)}).to_string();
        for typed in [false, true] {
            let good = if typed {
                json!({"candidates":[{"text":text,"kind":"word"}]}).to_string()
            } else {
                text.clone()
            };
            for bad_first in [false, true] {
                let bad = format!("```json\n{bad}\n```");
                let choices = if bad_first {
                    vec![choice(&bad), choice(&good)]
                } else {
                    vec![choice(&good), choice(&bad)]
                };
                for legacy in [false, true] {
                    let rows = fetch(
                        &input,
                        json!({"choices":choices}).to_string(),
                        legacy,
                        false,
                        None,
                    )
                    .expect("unusable structured choice must not hide a valid sibling");
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].text, text);
                    assert_eq!(rows[0].kind, typed.then_some(PredictionKind::Word));
                }
            }
        }
    }
}

#[test]
fn rejected_structured_rows_do_not_consume_usable_output_slots() {
    for input in cases() {
        let expected = expected(&input);
        assert!(expected.iter().all(|text| useful_candidate(text, &input)));
        for typed in [false, true] {
            let values: Vec<_> = rejected(&input)
                .into_iter()
                .chain(expected.iter().cloned())
                .map(|text| {
                    if typed {
                        json!({"text":text,"kind":"word"})
                    } else {
                        json!(text)
                    }
                })
                .collect();
            let content = json!({"candidates":values}).to_string();
            for ollama in [false, true] {
                for legacy in [false, true] {
                    let rows = fetch(&input, body(&content, ollama), legacy, ollama, None).unwrap();
                    let limit = if typed { 6 } else { 3 };
                    assert_eq!(
                        rows.iter().map(|row| &row.text).collect::<Vec<_>>(),
                        expected[..limit].iter().collect::<Vec<_>>()
                    );
                    assert!(
                        rows.iter()
                            .all(|row| row.kind == typed.then_some(PredictionKind::Word))
                    );
                }
            }
        }
    }
}

#[test]
fn rejected_plain_lines_do_not_hide_later_valid_lines_or_consume_the_three_row_cap() {
    let input = input("en", "hel", "hel");
    let content = "hel\nunrelated response\nhel there\nhel now\nhel later\nhel again\nhello\nhelp\nhelmet\nhello friend";
    for legacy in [false, true] {
        let rows = fetch(&input, body(content, false), legacy, false, None).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(),
            ["hello", "help", "helmet"]
        );
        assert!(rows.iter().all(|row| row.kind.is_none()));
    }
}

#[test]
fn request_filtering_keeps_scan_bounds_priority_empty_errors_and_dynamic_budgets() {
    let input = input("en", "hel", "hel");
    for ollama in [false, true] {
        let mut values: Vec<_> = (0..12).map(|i| json!(format!("unrelated {i}"))).collect();
        values.push(json!("hello"));
        for legacy in [false, true] {
            assert_eq!(
                fetch(
                    &input,
                    body(&json!({"candidates":values}).to_string(), ollama),
                    legacy,
                    ollama,
                    None
                ),
                Err(PredictionError::NoCandidates)
            );
            assert_eq!(
                fetch(
                    &input,
                    body(&json!({"candidates":rejected(&input)}).to_string(), ollama),
                    legacy,
                    ollama,
                    None
                ),
                Err(PredictionError::NoCandidates)
            );
        }
        let content = json!({"candidates":["hel", "hello", "help"]}).to_string();
        let rows = fetch(
            &input,
            body(&content, ollama),
            false,
            ollama,
            Some(crate::prediction::PredictionLimits {
                max_candidates: 1,
                max_generated_chars: 1,
            }),
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "help");
    }
    let selected = json!({"candidates":[{"text":"hello world","kind":"sentence"}]}).to_string();
    let later = json!({"candidates":[{"text":"helmet","kind":"word"}]}).to_string();
    for legacy in [false, true] {
        let choices = vec![choice("hello plainly"), choice(&selected), choice(&later)];
        let rows = fetch(
            &input,
            json!({"choices":choices}).to_string(),
            legacy,
            false,
            None,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "hello world");
        assert_eq!(rows[0].kind, Some(PredictionKind::Sentence));
    }
}
