//! Finite compatible-response regressions; no real service, credentials or GPU.
use super::*;
use crate::languages::llm::LlmCompletionProvider;
use std::{net::TcpListener, thread};

fn choice(content: &str) -> Value {
    json!({"message":{"content":content},"finish_reason":"stop"})
}

fn empty_structured_contents() -> Vec<String> {
    [
        json!([]),
        json!([null, false, 7, {}]),
        json!([{"text":"ignored type","kind":"system"}]),
        json!([{"text":"missing type"}]),
        json!(["", "  ", "bad\u{0}text"]),
        json!([{"text":"界".repeat(417),"kind":"sentence"}]),
    ]
    .into_iter()
    .map(|values| json!({"candidates":values}).to_string())
    .collect()
}

#[test]
fn empty_structured_choice_retains_valid_plain_siblings_in_either_order() {
    for (prefix, text) in [
        ("hel", "hello from a valid choice"),
        ("你好", "你好，很高兴认识你。"),
        ("日本語", "日本語で入力しています。"),
    ] {
        for empty in empty_structured_contents() {
            for fenced in [false, true] {
                let empty = if fenced {
                    format!("```json\n{empty}\n```")
                } else {
                    empty.clone()
                };
                assert!(
                    parse_chat_completion_candidates(
                        &json!({"choices":[choice(&empty)]}).to_string(),
                        Some(prefix),
                    )
                    .is_empty(),
                    "the invalid sibling must remain unusable"
                );
                for empty_first in [false, true] {
                    let choices = if empty_first {
                        vec![choice(&empty), choice(text)]
                    } else {
                        vec![choice(text), choice(&empty)]
                    };
                    let body = json!({"choices":choices}).to_string();
                    assert_eq!(
                        parse_chat_completion_candidates(&body, Some(prefix)),
                        [text],
                        "prefix={prefix:?}, empty_first={empty_first}, fenced={fenced}"
                    );
                }
            }
        }
    }
}

#[test]
fn nonempty_structured_choice_keeps_existing_priority_kind_and_limits() {
    let expected = [
        "hello",
        "help",
        "helmet",
        "hello world",
        "hello everyone",
        "hello there",
    ];
    let content = json!({"candidates":expected.iter().enumerate().map(|(i, text)| {
        json!({"text":text,"kind":if i < 3 { "word" } else { "sentence" }})
    }).collect::<Vec<_>>()})
    .to_string();
    for empty_first in [false, true] {
        let mut choices = vec![choice("hello from a plain choice")];
        if empty_first {
            choices.push(choice(r#"{"candidates":[]}"#));
        }
        choices.push(choice(&content));
        choices.push(choice(r#"{"candidates":[]}"#));
        choices.push(choice("hello from a later plain choice"));
        let body = json!({"choices":choices}).to_string();
        assert_eq!(
            parse_chat_completion_candidates(&body, Some("hel")),
            expected
        );
        assert_eq!(
            response_candidate_kind(&body, false, "hello", Some("hel")),
            Some(PredictionKind::Word)
        );
        assert_eq!(
            response_candidate_kind(&body, false, "hello world", Some("hel")),
            Some(PredictionKind::Sentence)
        );
    }
    let plain = json!({"choices":[
        choice("hello\nhelp\nhello\nhelmet\nhello there"),
        choice(r#"{"candidates":[]}"#),
        choice(&content)
    ]})
    .to_string();
    assert_eq!(
        parse_chat_completion_candidates(&plain, Some("hel")),
        ["hello", "help", "helmet"]
    );
    for reason in ["length", "tool_calls", "content_filter", "unknown"] {
        let mut incomplete = choice(&content);
        incomplete["finish_reason"] = json!(reason);
        let body =
            json!({"choices":[incomplete, choice(r#"{"candidates":[]}"#), choice("hello safely")]})
                .to_string();
        assert_eq!(
            parse_chat_completion_candidates(&body, Some("hel")),
            ["hello safely"]
        );
    }
}

#[test]
fn empty_choice_does_not_hide_valid_loopback_results_in_either_provider_api() {
    let mut scenarios = Vec::new();
    for (language, seed, prefix, text) in [
        ("en", "hel", "hel", "hello from a valid choice"),
        ("zh-Hans", "nihao", "你好", "你好，很高兴认识你。"),
        ("ja", "nihongo", "日本語", "日本語で入力しています。"),
    ] {
        for empty_first in [false, true] {
            let rows = if empty_first {
                vec![choice(r#"{"candidates":[]}"#), choice(text)]
            } else {
                vec![choice(text), choice(r#"{"candidates":[]}"#)]
            };
            let body = json!({"choices":rows}).to_string();
            for legacy in [false, true] {
                scenarios.push((language, seed, prefix, text, legacy, body.clone()));
            }
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    );
    let responses: Vec<_> = scenarios.iter().map(|s| s.5.clone()).collect();
    let server = thread::spawn(move || {
        for body in responses {
            let (mut stream, _) = test_server::accept(&listener);
            let wire = test_server::read_request(&mut stream).unwrap();
            assert!(wire.starts_with("POST /v1/chat/completions "));
            assert!(!wire.to_ascii_lowercase().contains("\r\nauthorization:"));
            test_server::respond(&mut stream, "200 OK", &body).unwrap();
        }
    });
    let provider = HttpModelProvider::new(ModelProviderConfig {
        protocol: ModelProtocol::ChatCompletions,
        endpoint,
        model: "synthetic-choice-fixture".into(),
        ..Default::default()
    });
    for (language, seed, prefix, text, legacy, _) in scenarios {
        let input = PredictionInput {
            language_id: language.into(),
            seed_text: seed.into(),
            normalized_phrase: prefix.into(),
            context_before_cursor: String::new(),
            confidence: 1.0,
            degraded: false,
        };
        if legacy {
            let rows = provider.generate_checked(&input).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].text, text);
            assert_eq!(rows[0].kind, None);
        } else {
            let request = PredictionRequest::new(17, input);
            let response = provider
                .predict(&request, &PredictionCancellation::default())
                .unwrap();
            response.validate_for(&request).unwrap();
            assert_eq!(response.candidates.len(), 1);
            assert_eq!(response.candidates[0].text, text);
            assert_eq!(response.candidates[0].kind, None);
        }
    }
    server.join().unwrap();
}
