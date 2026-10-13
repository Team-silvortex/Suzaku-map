//! Legacy and versioned entry points share owned, finite loopback services only.
use super::{HttpModelProvider, ModelProtocol, ModelProviderConfig, test_server};
use crate::{
    languages::llm::{LlmCancellation, LlmCompletion, LlmCompletionProvider, LlmProviderError},
    prediction::{
        PredictionCancellation, PredictionCandidate, PredictionError, PredictionInput,
        PredictionKind, PredictionLimits, PredictionProvider, PredictionRequest,
    },
};
use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

const MODEL: &str = "synthetic-legacy-compatibility";

fn input() -> PredictionInput {
    PredictionInput {
        language_id: "en".into(),
        seed_text: "hel".into(),
        normalized_phrase: "hel".into(),
        context_before_cursor: "Synthetic context.".into(),
        confidence: 0.8,
        degraded: true,
    }
}

fn canonical(completions: Vec<LlmCompletion>) -> Vec<PredictionCandidate> {
    completions.into_iter().map(Into::into).collect()
}

fn response_body(protocol: ModelProtocol, content: &str) -> String {
    if protocol == ModelProtocol::Ollama {
        json!({"message":{"content":content},"done":true}).to_string()
    } else {
        json!({"choices":[{"message":{"content":content},"finish_reason":"stop"}]}).to_string()
    }
}

fn config(listener: &TcpListener, protocol: ModelProtocol) -> ModelProviderConfig {
    let path = if protocol == ModelProtocol::Ollama {
        "api/chat"
    } else {
        "v1/chat/completions"
    };
    ModelProviderConfig {
        protocol,
        endpoint: format!("http://{}/{path}", listener.local_addr().unwrap()),
        model: MODEL.into(),
        ..Default::default()
    }
}

fn serve_inventory(listener: &TcpListener) {
    let (mut stream, _) = test_server::accept(listener);
    let wire = test_server::read_request(&mut stream).unwrap();
    assert!(wire.starts_with("GET /api/tags "), "{wire}");
    test_server::respond(
        &mut stream,
        "200 OK",
        &json!({"models":[{"name":MODEL}]}).to_string(),
    )
    .unwrap();
}

fn read_generation(stream: &mut TcpStream, protocol: ModelProtocol) -> Value {
    let wire = test_server::read_request(stream).unwrap();
    let request_line = if protocol == ModelProtocol::Ollama {
        "POST /api/chat "
    } else {
        "POST /v1/chat/completions "
    };
    assert!(wire.starts_with(request_line), "{wire}");
    let body: Value = serde_json::from_str(wire.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["model"], MODEL);
    body
}

fn assert_no_pending_connections(listener: &TcpListener) {
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock,
        "entry points must not add connections outside the expected request trace"
    );
}

fn fixture(
    protocol: ModelProtocol,
    replies: Vec<(&'static str, String)>,
) -> (HttpModelProvider, JoinHandle<(TcpListener, Vec<Value>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let provider = HttpModelProvider::new(config(&listener, protocol));
    let server = thread::spawn(move || {
        if protocol == ModelProtocol::Ollama {
            serve_inventory(&listener);
        }
        let mut captured = Vec::new();
        for (status, body) in replies {
            let (mut stream, _) = test_server::accept(&listener);
            captured.push(read_generation(&mut stream, protocol));
            test_server::respond(&mut stream, status, &body).unwrap();
        }
        (listener, captured)
    });
    (provider, server)
}

#[test]
fn legacy_and_prediction_share_wire_results_but_only_prediction_applies_call_budgets() {
    let content = json!({"candidates":[
        {"text":"hello world","kind":"sentence"},
        {"text":"hello","kind":"word"},
        {"text":"help","kind":"word"}
    ]})
    .to_string();
    let expected = vec![
        PredictionCandidate {
            text: "hello world".into(),
            score_bias: 0.55,
            kind: Some(PredictionKind::Sentence),
        },
        PredictionCandidate {
            text: "hello".into(),
            score_bias: 0.55 - 0.05,
            kind: Some(PredictionKind::Word),
        },
        PredictionCandidate {
            text: "help".into(),
            score_bias: 0.55 - 2.0 * 0.05,
            kind: Some(PredictionKind::Word),
        },
    ];
    for protocol in [ModelProtocol::Ollama, ModelProtocol::OpenAiCompatible] {
        let (provider, server) = fixture(
            protocol,
            vec![("200 OK", response_body(protocol, &content)); 4],
        );
        let input = input();
        let before = input.clone();
        let prediction = PredictionRequest::new(91, input.clone());
        assert_eq!(
            canonical(provider.generate_checked(&input).unwrap()),
            expected
        );
        assert_eq!(
            canonical(
                provider
                    .generate_cancellable(&input, &LlmCancellation::default())
                    .unwrap()
            ),
            expected
        );
        let result = provider
            .predict(&prediction, &PredictionCancellation::default())
            .unwrap();
        result.validate_for(&prediction).unwrap();
        assert_eq!(result.request_id, 91);
        assert_eq!(result.candidates, expected);

        let mut narrow = PredictionRequest::new(92, input.clone());
        narrow.limits = PredictionLimits {
            max_candidates: 1,
            max_generated_chars: 1,
        };
        let result = provider
            .predict(&narrow, &PredictionCancellation::default())
            .unwrap();
        result.validate_for(&narrow).unwrap();
        assert_eq!(result.request_id, 92);
        assert_eq!(result.candidates, vec![expected[2].clone()]);
        assert_eq!(input, before, "budgets must not mutate the caller's draft");

        let (listener, captured) = server.join().unwrap();
        assert_eq!(captured.len(), 4);
        assert!(captured.windows(2).all(|pair| pair[0] == pair[1]));
        let fields: Value =
            serde_json::from_str(captured[0]["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(fields["raw_composition"], input.seed_text);
        assert_eq!(fields["local_conversion"], input.normalized_phrase);
        assert_eq!(fields["committed_context"], input.context_before_cursor);
        assert!(fields.get("request_id").is_none());
        assert!(fields.get("version").is_none());
        assert_no_pending_connections(&listener);
    }
}

#[test]
fn legacy_errors_and_preflight_order_are_not_replaced_by_prediction_validation() {
    for protocol in [ModelProtocol::Ollama, ModelProtocol::OpenAiCompatible] {
        for (status, body, expected) in [
            (
                "429 Too Many Requests",
                "synthetic error",
                PredictionError::HttpStatus(429),
            ),
            ("200 OK", "not-json", PredictionError::InvalidResponse),
        ] {
            let (provider, server) = fixture(protocol, vec![(status, body.into()); 4]);
            let input = input();
            assert_eq!(provider.generate_checked(&input), Err(expected.clone()));
            assert_eq!(
                provider.generate_cancellable(&input, &LlmCancellation::default()),
                Err(expected.clone())
            );
            assert_eq!(
                provider.predict(
                    &PredictionRequest::new(93, input.clone()),
                    &PredictionCancellation::default()
                ),
                Err(expected)
            );
            assert!(provider.generate(&input).is_empty());
            let (listener, captured) = server.join().unwrap();
            assert_eq!(captured.len(), 4);
            assert!(captured.windows(2).all(|pair| pair[0] == pair[1]));
            assert_no_pending_connections(&listener);
        }
    }

    // The old input type has no versioned request validation. Preserve this
    // specific difference instead of requiring parity for every invalid input.
    let protocol = ModelProtocol::OpenAiCompatible;
    let content = json!({"candidates":[{"text":"hello","kind":"word"}]}).to_string();
    let (provider, server) = fixture(
        protocol,
        vec![("200 OK", response_body(protocol, &content))],
    );
    let mut invalid_input = input();
    invalid_input.confidence = 2.0;
    assert_eq!(
        provider.predict(
            &PredictionRequest::new(94, invalid_input.clone()),
            &PredictionCancellation::default()
        ),
        Err(PredictionError::InvalidRequest)
    );
    assert_eq!(
        provider.generate_checked(&invalid_input).unwrap()[0].text,
        "hello"
    );
    let (listener, captured) = server.join().unwrap();
    assert_eq!(captured.len(), 1);
    assert_no_pending_connections(&listener);

    let cancelled = PredictionCancellation::default();
    cancelled.cancel();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let valid_config = config(&listener, protocol);
    let provider = HttpModelProvider::new(valid_config.clone());
    let input = input();
    let prediction = PredictionRequest::new(95, input.clone());
    assert_eq!(
        provider.generate_cancellable(&input, &cancelled),
        Err(LlmProviderError::Cancelled)
    );
    assert_eq!(
        provider.predict(&prediction, &cancelled),
        Err(PredictionError::Cancelled)
    );
    assert_no_pending_connections(&listener);

    let invalid_provider = HttpModelProvider::new(ModelProviderConfig {
        endpoint: "not-an-endpoint".into(),
        ..valid_config
    });
    assert_eq!(
        invalid_provider.generate_cancellable(&input, &cancelled),
        Err(LlmProviderError::InvalidEndpoint),
        "legacy completion checks configuration before cancellation"
    );
    assert_eq!(
        invalid_provider.predict(&prediction, &cancelled),
        Err(PredictionError::Cancelled),
        "prediction checks cancellation before configuration"
    );
}

fn assert_peer_closed(stream: &mut TcpStream) {
    // The existing cancellation fixtures use the same bound. EOF or reset is
    // evidence of cancellation; a read timeout is not evidence of peer closure.
    stream
        .set_read_timeout(Some(Duration::from_millis(700)))
        .unwrap();
    match stream.read(&mut [0]) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ) => {}
        result => panic!("cancelled model socket stayed connected: {result:?}"),
    }
}

#[test]
fn cancelling_one_entry_point_preserves_inventory_for_the_other_entry_point() {
    for cancel_legacy in [true, false] {
        let protocol = ModelProtocol::Ollama;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let provider = HttpModelProvider::new(ModelProviderConfig {
            // Match the existing cancellation fixture's request deadline.
            timeout_ms: 2000,
            ..config(&listener, protocol)
        });
        let (entered, started) = mpsc::channel();
        let server = thread::spawn(move || {
            serve_inventory(&listener);
            let content = json!({"candidates":[{"text":"hello","kind":"word"}]}).to_string();
            let body = response_body(protocol, &content);
            let (mut cancelled, _) = test_server::accept(&listener);
            let cancelled_body = read_generation(&mut cancelled, protocol);
            // A complete-looking JSON body is not a complete HTTP envelope.
            write!(
                cancelled,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len() + 1
            )
            .unwrap();
            cancelled.flush().unwrap();
            entered.send(()).unwrap();
            assert_peer_closed(&mut cancelled);
            drop(cancelled);

            // A rediscovery here must fail the trace, not receive another inventory.
            let (mut recovered, _) = test_server::accept(&listener);
            let recovered_body = read_generation(&mut recovered, protocol);
            test_server::respond(&mut recovered, "200 OK", &body).unwrap();
            (listener, cancelled_body, recovered_body)
        });
        let worker_provider = provider.clone();
        let cancellation = PredictionCancellation::default();
        let worker_cancellation = cancellation.clone();
        let worker = thread::spawn(move || {
            if cancel_legacy {
                worker_provider
                    .generate_cancellable(&input(), &worker_cancellation)
                    .map(canonical)
            } else {
                worker_provider
                    .predict(&PredictionRequest::new(96, input()), &worker_cancellation)
                    .map(|response| response.candidates)
            }
        });
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        cancellation.cancel();
        assert_eq!(worker.join().unwrap(), Err(PredictionError::Cancelled));
        let recovered = if cancel_legacy {
            let request = PredictionRequest::new(97, input());
            let response = provider
                .predict(&request, &PredictionCancellation::default())
                .unwrap();
            response.validate_for(&request).unwrap();
            assert_eq!(response.request_id, 97);
            response.candidates
        } else {
            canonical(provider.generate_checked(&input()).unwrap())
        };
        assert_eq!(
            recovered,
            vec![PredictionCandidate {
                text: "hello".into(),
                score_bias: 0.55,
                kind: Some(PredictionKind::Word),
            }]
        );
        let (listener, cancelled_body, recovered_body) = server.join().unwrap();
        assert_eq!(cancelled_body, recovered_body);
        assert_no_pending_connections(&listener);
    }
}
