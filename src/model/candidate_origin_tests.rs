//! Candidate metadata belongs to the accepted row, not a same-text alternative.
//! Finite synthetic HTTP fixtures only; no credentials, real model or desktop.
use super::*;
use crate::languages::llm::LlmCompletionProvider;
use std::{net::TcpListener, thread};

fn choice(content: &str) -> Value {
    json!({"message":{"content":content},"finish_reason":"stop"})
}

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

fn cases() -> [(PredictionInput, [&'static str; 3]); 3] {
    [
        (input("en", "hel", "hel"), ["hello", "help", "hello world"]),
        (
            input("zh-Hans", "nihao", "你好"),
            ["你好呀", "你好吗", "你好，我们开始吧。"],
        ),
        (
            input("ja", "nihongo", "日本語"),
            ["日本語入力", "日本語変換", "日本語で入力します。"],
        ),
    ]
}

fn fetch(
    input: &PredictionInput,
    body: String,
    ollama: bool,
    legacy: bool,
) -> Vec<PredictionCandidate> {
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
            test_server::respond(
                &mut stream,
                "200 OK",
                &json!({"models":[{"name":"synthetic-candidate-origin"}]}).to_string(),
            )
            .unwrap();
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
        model: "synthetic-candidate-origin".into(),
        ..Default::default()
    });
    let result = if legacy {
        provider
            .generate_checked(input)
            .map(|rows| rows.into_iter().map(Into::into).collect())
    } else {
        let request = PredictionRequest::new(29, input.clone());
        provider
            .predict(&request, &PredictionCancellation::default())
            .map(|response| {
                response.validate_for(&request).unwrap();
                response.candidates
            })
    };
    server.join().unwrap();
    result.expect("finite candidate-origin response")
}

fn body(content: &str, ollama: bool) -> String {
    if ollama {
        json!({"done":true,"done_reason":"stop","message":{"content":content}}).to_string()
    } else {
        json!({"choices":[choice(content)]}).to_string()
    }
}

fn assert_rows(actual: &[PredictionCandidate], expected: &[(&str, Option<PredictionKind>)]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (row, (text, kind))) in actual.iter().zip(expected).enumerate() {
        assert_eq!(row.text, *text);
        assert_eq!(
            row.kind, *kind,
            "selected text {text:?} must keep its own metadata"
        );
        assert_eq!(row.score_bias, 0.55 - index as f32 * 0.05);
    }
}

#[test]
fn metadata_beyond_the_scan_boundary_cannot_override_the_selected_choice() {
    for (input, words) in cases() {
        for (selected, ignored) in [("word", "sentence"), ("sentence", "word")] {
            let mut rejected: Vec<_> = (0..12)
                .map(|index| json!(format!("unrelated {index}")))
                .collect();
            rejected.push(json!({"text":words[0],"kind":ignored}));
            let rejected = json!({"candidates":rejected}).to_string();
            let accepted = json!({"candidates":[{"text":words[0],"kind":selected}]}).to_string();
            let body = json!({"choices":[choice(&rejected),choice(&accepted)]}).to_string();
            for legacy in [false, true] {
                let rows = fetch(&input, body.clone(), false, legacy);
                let expected = if selected == "word" {
                    PredictionKind::Word
                } else {
                    PredictionKind::Sentence
                };
                assert_rows(&rows, &[(words[0], Some(expected))]);
            }
        }
    }
}

#[test]
fn an_unselected_later_choice_cannot_label_plain_or_string_results() {
    for (input, words) in cases() {
        let poison = json!({"candidates":[{"text":words[0],"kind":"sentence"}]}).to_string();
        for structured in [false, true] {
            let accepted = if structured {
                json!({"candidates":words}).to_string()
            } else {
                words.join("\n")
            };
            let body = json!({"choices":[choice(&accepted),choice(&poison)]}).to_string();
            for legacy in [false, true] {
                assert_rows(
                    &fetch(&input, body.clone(), false, legacy),
                    &words.map(|text| (text, None)),
                );
            }
        }
    }
}

#[test]
fn deduplicated_rows_do_not_change_the_first_accepted_rows_kind() {
    for (input, words) in cases() {
        let content = json!({"candidates":[
            words[0],
            {"text":words[0],"kind":"sentence"},
            {"text":words[1],"kind":"word"},
            {"text":words[1],"kind":"sentence"}
        ]})
        .to_string();
        for ollama in [false, true] {
            for legacy in [false, true] {
                assert_rows(
                    &fetch(&input, body(&content, ollama), ollama, legacy),
                    &[(words[0], None), (words[1], Some(PredictionKind::Word))],
                );
            }
        }
    }
}

#[test]
fn accepted_typed_rows_keep_exact_spacing_order_type_and_score_in_both_protocols() {
    let mut cases = cases().to_vec();
    cases.push((
        input("en", "  hel", "  hel"),
        ["  hello", "  help", "  hello world"],
    ));
    for (input, words) in cases {
        let content = json!({"candidates":[
            {"text":words[0],"kind":"system"},
            {"text":format!("{}  ",words[0]),"kind":"word"},
            {"text":words[0],"kind":"sentence"},
            words[1],
            {"text":words[2],"kind":"sentence"}
        ]})
        .to_string();
        for ollama in [false, true] {
            for legacy in [false, true] {
                assert_rows(
                    &fetch(&input, body(&content, ollama), ollama, legacy),
                    &[
                        (words[0], Some(PredictionKind::Word)),
                        (words[1], None),
                        (words[2], Some(PredictionKind::Sentence)),
                    ],
                );
            }
        }
    }
}
