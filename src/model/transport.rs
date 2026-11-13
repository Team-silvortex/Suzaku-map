//! Local requests never resolve DNS or use proxies. Remote requests require HTTPS
//! and separate consent; credentials, URLs and response bodies never enter errors.
use super::{
    MAX_RESPONSE_BYTES, ModelProviderConfig, ModelScope, check_cancellation,
    local_http::http_request_with_cancel,
};
use crate::prediction::{PredictionCancellation, PredictionError};
use reqwest::{
    Client, ClientBuilder,
    header::{AUTHORIZATION, HeaderValue},
};
use std::{
    cell::RefCell,
    future::Future,
    task::Poll,
    time::{Duration, Instant},
};

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().enumerate().all(|(index, ch)| {
            ch == b'_' || ch.is_ascii_alphabetic() || (index > 0 && ch.is_ascii_digit())
        })
}

pub(super) fn valid_api_key(value: &str) -> bool {
    !value.is_empty() && value.len() <= 8192 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

pub(super) fn is_cloud_endpoint(endpoint: &str) -> bool {
    if endpoint.len() > 2048
        || endpoint.contains(['@', '?', '#', '\\'])
        || endpoint
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return false;
    }
    reqwest::Url::parse(endpoint).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.port() != Some(0)
            && url.path() != "/"
    })
}

fn client_builder() -> ClientBuilder {
    Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .referer(false)
        .http1_only()
        .connect_timeout(Duration::from_secs(5))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
}

// Construct AND drop these on the prediction worker, never the UI. Field order
// drops the pooled Client before its current-thread runtime. No request owns a
// detached worker; the same runtime drives its one pinned send/body future.
struct CloudTransport {
    client: Client,
    runtime: tokio::runtime::Runtime,
}

impl CloudTransport {
    fn new(builder: ClientBuilder) -> Result<Self, PredictionError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| PredictionError::Unavailable)?;
        let client = {
            let _entered = runtime.enter();
            builder.build().map_err(map_error)?
        };
        Ok(Self { client, runtime })
    }
}

// Reuse connections without retaining an Authorization header on the client.
thread_local! {
    static CLIENT: RefCell<Option<CloudTransport>> = const { RefCell::new(None) };
}

// Observe the actual bounded body path in the private slow-body fixture only.
// The notification carries no request, response, or credential contents.
#[cfg(test)]
thread_local! {
    static BODY_CHUNK_SEEN: RefCell<Option<std::sync::mpsc::Sender<()>>> = const { RefCell::new(None) };
}

pub(super) fn request(
    config: &ModelProviderConfig,
    method: &str,
    body: Option<&str>,
    timeout: Duration,
) -> Result<String, PredictionError> {
    request_with_cancel(config, method, body, timeout, None)
}

pub(super) fn request_with_cancel(
    config: &ModelProviderConfig,
    method: &str,
    body: Option<&str>,
    timeout: Duration,
    cancellation: Option<&PredictionCancellation>,
) -> Result<String, PredictionError> {
    let started = Instant::now();
    config.validate()?;
    check_cancellation(cancellation)?;
    if config.scope == ModelScope::Local {
        return http_request_with_cancel(&config.endpoint, method, body, timeout, cancellation);
    }
    if !config.cloud_consent {
        return Err(PredictionError::CloudConsentRequired);
    }
    let authorization = authorization(config.api_key_env.as_deref(), |name| {
        std::env::var(name).ok()
    })?;
    let deadline = started
        .checked_add(timeout)
        .ok_or(PredictionError::Timeout)?;
    let result = CLIENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(CloudTransport::new(client_builder())?);
        }
        let transport = slot.as_ref().unwrap();
        https_request_with_deadline(
            transport,
            &config.endpoint,
            method,
            body,
            authorization,
            deadline,
            cancellation,
        )
    });
    check_cancellation(cancellation)?;
    result
}

fn authorization(
    name: Option<&str>,
    read: impl FnOnce(&str) -> Option<String>,
) -> Result<Option<HeaderValue>, PredictionError> {
    let Some(name) = name else {
        return Ok(None);
    };
    if !valid_env_name(name) {
        return Err(PredictionError::MissingCredentials);
    }
    let key = read(name).ok_or(PredictionError::MissingCredentials)?;
    if !valid_api_key(&key) {
        return Err(PredictionError::MissingCredentials);
    }
    let mut header = HeaderValue::from_str(&format!("Bearer {key}"))
        .map_err(|_| PredictionError::MissingCredentials)?;
    header.set_sensitive(true);
    Ok(Some(header))
}

fn map_error(error: reqwest::Error) -> PredictionError {
    if error.is_timeout() {
        PredictionError::Timeout
    } else {
        PredictionError::Unavailable
    }
}

#[cfg(test)]
fn https_request(
    transport: &CloudTransport,
    endpoint: &str,
    method: &str,
    body: Option<&str>,
    authorization: Option<HeaderValue>,
    timeout: Duration,
) -> Result<String, PredictionError> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(PredictionError::Timeout)?;
    https_request_with_deadline(
        transport,
        endpoint,
        method,
        body,
        authorization,
        deadline,
        None,
    )
}

fn https_request_with_deadline(
    transport: &CloudTransport,
    endpoint: &str,
    method: &str,
    body: Option<&str>,
    authorization: Option<HeaderValue>,
    deadline: Instant,
    cancellation: Option<&PredictionCancellation>,
) -> Result<String, PredictionError> {
    let request = prepare_then_send(
        deadline,
        cancellation,
        || {
            let method = reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|_| PredictionError::InvalidEndpoint)?;
            let mut request = transport
                .client
                .request(method, endpoint)
                .header("Accept", "application/json");
            if let Some(auth) = authorization {
                request = request.header(AUTHORIZATION, auth);
            }
            if let Some(body) = body {
                request = request
                    .header("Content-Type", "application/json")
                    .body(body.to_owned());
            }
            Ok(request)
        },
        Instant::now,
        |request, remaining| Ok(request.timeout(remaining)),
    )?;
    transport.runtime.block_on(await_inflight(
        async move {
            let mut response = request.send().await.map_err(map_error)?;
            if response.status().as_u16() != 200 {
                return Err(PredictionError::HttpStatus(response.status().as_u16()));
            }
            if response
                .content_length()
                .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
            {
                return Err(PredictionError::ResponseTooLarge);
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|error| {
                if error.is_timeout() {
                    PredictionError::Timeout
                } else {
                    PredictionError::InvalidResponse
                }
            })? {
                if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                    return Err(PredictionError::ResponseTooLarge);
                }
                bytes.extend_from_slice(&chunk);
                #[cfg(test)]
                BODY_CHUNK_SEEN.with(|slot| {
                    if let Some(observer) = slot.borrow_mut().take() {
                        let _ = observer.send(());
                    }
                });
            }
            String::from_utf8(bytes).map_err(|_| PredictionError::InvalidResponse)
        },
        deadline,
        cancellation,
        Instant::now,
    ))
}

/// Poll one future, not a new request on every tick. Dropping it stops waiting
/// for this response; it cannot retract input already sent to a remote service
/// or guarantee that server-side work/billing or OS DNS resolution has stopped.
async fn await_inflight<Output>(
    operation: impl Future<Output = Result<Output, PredictionError>>,
    deadline: Instant,
    cancellation: Option<&PredictionCancellation>,
    now: impl Fn() -> Instant,
) -> Result<Output, PredictionError> {
    let mut operation = std::pin::pin!(operation);
    let wake_at =
        || tokio::time::Instant::from_std(deadline.min(now() + CANCELLATION_POLL_INTERVAL));
    let mut tick = std::pin::pin!(tokio::time::sleep_until(wake_at()));
    std::future::poll_fn(|context| {
        if let Err(error) = check_cancellation(cancellation) {
            return Poll::Ready(Err(error));
        }
        if now() >= deadline {
            return Poll::Ready(Err(PredictionError::Timeout));
        }
        let polled = operation.as_mut().poll(context);
        // One poll may itself consume the remaining time or race cancellation.
        // Check Pending too, so an elapsed timer is never reset into the past.
        if let Err(error) = check_cancellation(cancellation) {
            return Poll::Ready(Err(error));
        }
        if now() >= deadline {
            return Poll::Ready(Err(PredictionError::Timeout));
        }
        if let Poll::Ready(result) = polled {
            return Poll::Ready(result);
        }
        if tick.as_mut().poll(context).is_ready() {
            tick.as_mut().reset(wake_at());
            // Register the reset timer before yielding; no extra request or task.
            if tick.as_mut().poll(context).is_ready() {
                // The absolute deadline can pass between its guard and reset.
                // A ready/past timer must not yield without another wake-up.
                context.waker().wake_by_ref();
            }
        }
        Poll::Pending
    })
    .await
}

// Both cold Client initialization and request construction must spend the same
// absolute budget. Recheck immediately before sending, not before preparation.
fn prepare_then_send<Prepared, Output>(
    deadline: Instant,
    cancellation: Option<&PredictionCancellation>,
    prepare: impl FnOnce() -> Result<Prepared, PredictionError>,
    now: impl FnOnce() -> Instant,
    send: impl FnOnce(Prepared, Duration) -> Result<Output, PredictionError>,
) -> Result<Output, PredictionError> {
    let prepared = prepare()?;
    check_cancellation(cancellation)?;
    let remaining = deadline
        .checked_duration_since(now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(PredictionError::Timeout)?;
    send(prepared, remaining)
}

#[cfg(test)]
mod tests {
    use super::super::test_server;
    use super::*;
    use crate::languages::llm::LlmProviderError;
    use std::{net::TcpListener, sync::Arc, thread, time::Instant};

    #[test]
    fn a_single_future_poll_cannot_complete_or_stall_past_the_original_deadline() {
        use std::cell::Cell;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for returns_ready in [true, false] {
            let started = Instant::now();
            let deadline = started + Duration::from_secs(1);
            let clock = Cell::new(started);
            let polls = Cell::new(0);
            let operation = std::future::poll_fn(|_| {
                polls.set(polls.get() + 1);
                clock.set(deadline);
                if returns_ready {
                    Poll::Ready(Ok(()))
                } else {
                    // Deliberately no wake-up: the transport must itself reject
                    // this elapsed Pending result instead of awaiting a timer.
                    Poll::Pending
                }
            });
            assert_eq!(
                runtime.block_on(await_inflight(operation, deadline, None, || clock.get())),
                Err(PredictionError::Timeout)
            );
            assert_eq!(polls.get(), 1);
        }
    }

    #[test]
    fn inflight_cancellation_precedes_timeout_and_ready_response() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let cancelled = PredictionCancellation::default();
        cancelled.cancel();
        let never_poll = std::future::poll_fn(|_| -> Poll<Result<(), PredictionError>> {
            panic!("cancelled operations must not be polled")
        });
        assert_eq!(
            runtime.block_on(await_inflight(
                never_poll,
                Instant::now(),
                Some(&cancelled),
                Instant::now,
            )),
            Err(PredictionError::Cancelled)
        );
        let cancellation = PredictionCancellation::default();
        let operation = std::future::poll_fn(|_| {
            cancellation.cancel();
            Poll::Ready(Ok(()))
        });
        assert_eq!(
            runtime.block_on(await_inflight(
                operation,
                Instant::now() + Duration::from_secs(1),
                Some(&cancellation),
                Instant::now,
            )),
            Err(PredictionError::Cancelled)
        );
    }

    #[test]
    fn api_key_format_validation_is_pure_and_preserves_original_bounds() {
        for value in ["synthetic-key", "a_B.1-~", &"k".repeat(8192)] {
            assert!(valid_api_key(value));
        }
        for value in [
            "",
            " synthetic-key",
            "synthetic-key ",
            "synthetic\tkey",
            "synthetic\r\nkey",
            "synthetic\0key",
            "synthetic\u{7f}key",
            "synthetic-模型",
            &"k".repeat(8193),
        ] {
            assert!(!valid_api_key(value));
        }
    }

    #[test]
    fn cancelled_after_preparation_never_calls_the_sender_or_uses_a_new_budget() {
        let cancellation = PredictionCancellation::default();
        let prepared = std::cell::Cell::new(0);
        let sent = std::cell::Cell::new(0);
        let result = prepare_then_send(
            Instant::now(),
            Some(&cancellation),
            || {
                prepared.set(prepared.get() + 1);
                cancellation.cancel();
                Ok(())
            },
            || panic!("cancellation must precede the remaining-time check"),
            |(), _| {
                sent.set(sent.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result, Err(PredictionError::Cancelled));
        assert_eq!(prepared.get(), 1);
        assert_eq!(sent.get(), 0);
    }

    #[test]
    fn preparation_that_exhausts_the_deadline_never_calls_the_sender() {
        let started = Instant::now();
        let deadline = started + Duration::from_millis(1200);
        for finished in [deadline, deadline + Duration::from_millis(1)] {
            let prepared = std::cell::Cell::new(0);
            let sent = std::cell::Cell::new(0);
            let result = prepare_then_send(
                deadline,
                None,
                || {
                    prepared.set(prepared.get() + 1);
                    Ok(())
                },
                || {
                    assert_eq!(prepared.get(), 1);
                    finished
                },
                |(), _| {
                    sent.set(sent.get() + 1);
                    Ok(())
                },
            );
            assert_eq!(result, Err(PredictionError::Timeout));
            assert_eq!(prepared.get(), 1);
            assert_eq!(sent.get(), 0);
        }
    }

    #[test]
    fn prepared_request_is_sent_once_with_only_its_original_remaining_budget() {
        let started = Instant::now();
        let prepared = std::cell::Cell::new(0);
        let sent = std::cell::Cell::new(0);
        let result = prepare_then_send(
            started + Duration::from_millis(1200),
            Some(&PredictionCancellation::default()),
            || {
                prepared.set(prepared.get() + 1);
                Ok("prepared request")
            },
            || {
                assert_eq!(prepared.get(), 1);
                started + Duration::from_millis(450)
            },
            |request, remaining| {
                sent.set(sent.get() + 1);
                assert_eq!(request, "prepared request");
                assert_eq!(remaining, Duration::from_millis(750));
                Ok("synthetic response")
            },
        );
        assert_eq!(result, Ok("synthetic response"));
        assert_eq!(prepared.get(), 1);
        assert_eq!(sent.get(), 1);
    }

    #[test]
    fn preparation_failure_keeps_its_error_without_sending_or_resetting_the_clock() {
        let sent = std::cell::Cell::new(0);
        let result = prepare_then_send(
            Instant::now(),
            None,
            || Err::<(), _>(PredictionError::InvalidEndpoint),
            || panic!("failed preparation does not start a request timeout"),
            |(), _| {
                sent.set(sent.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result, Err(PredictionError::InvalidEndpoint));
        assert_eq!(sent.get(), 0);
    }

    fn tls_server(
        handler: impl FnOnce(rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>)
        + Send
        + 'static,
    ) -> (String, reqwest::Certificate, thread::JoinHandle<()>) {
        let key = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let certificate = reqwest::Certificate::from_der(key.cert.der()).unwrap();
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![key.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(key.key_pair.serialize_der()).into(),
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "https://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let server = thread::spawn(move || {
            let (stream, _) = test_server::accept(&listener);
            handler(rustls::StreamOwned::new(
                rustls::ServerConnection::new(Arc::new(config)).unwrap(),
                stream,
            ));
        });
        (url, certificate, server)
    }

    fn latest_draft_replaces_a_sent_stalled_cloud_request(stall_body: bool) {
        use crate::{
            ime::{
                EngineConfig, PredictionStatus, XRTabletImeEngine, candidate_mix::CandidateSource,
            },
            model::HttpModelProvider,
            prediction::{PredictionProvider, PredictionRequest, PredictionResponse},
        };
        use std::{
            io::{ErrorKind, Read, Write},
            sync::{Mutex, mpsc},
        };

        let key = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let certificate = reqwest::Certificate::from_der(key.cert.der()).unwrap();
        let config = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![key.cert.der().clone()],
                    rustls::pki_types::PrivatePkcs8KeyDer::from(key.key_pair.serialize_der())
                        .into(),
                )
                .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "https://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let (sent, received) = mpsc::channel();
        let (body_seen, body_received) = mpsc::channel();
        let server = thread::spawn(move || {
            let (stream, _) = test_server::accept(&listener);
            let mut first = rustls::StreamOwned::new(
                rustls::ServerConnection::new(config.clone()).unwrap(),
                stream,
            );
            let old_request = test_server::read_request(&mut first).unwrap();
            if stall_body {
                first.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 256\r\nConnection: keep-alive\r\n\r\n{\"choices\":[").unwrap();
                first.flush().unwrap();
            }
            sent.send(()).unwrap();
            first
                .sock
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            // An elapsed socket timeout is NOT evidence of cancellation. TLS
            // without close_notify legitimately reports UnexpectedEof instead
            // of read(0); both mean the cancelled future released this socket.
            match first.read(&mut [0u8; 1]) {
                Ok(0) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::UnexpectedEof
                            | ErrorKind::ConnectionReset
                            | ErrorKind::ConnectionAborted
                            | ErrorKind::BrokenPipe
                    ) => {}
                other => panic!(
                    "cancelled TLS peer did not close before the old five-second budget: {other:?}"
                ),
            }
            drop(first);
            let (stream, _) = test_server::accept(&listener);
            let mut latest =
                rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), stream);
            let latest_request = test_server::read_request(&mut latest).unwrap();
            let latest_body: serde_json::Value =
                serde_json::from_str(latest_request.split_once("\r\n\r\n").unwrap().1).unwrap();
            let latest_input: serde_json::Value =
                serde_json::from_str(latest_body["messages"][1]["content"].as_str().unwrap())
                    .unwrap();
            // Preserve the actual engine conversion prefix, independently of
            // local lexicon growth. This is a transport fixture, not a claim
            // that its synthetic completion is linguistically useful.
            let model_candidate = format!(
                "{}fixture",
                latest_input["local_conversion"].as_str().unwrap()
            );
            test_server::respond(
                &mut latest,
                "200 OK",
                &serde_json::json!({"choices":[{"message":{"content":model_candidate},"finish_reason":"stop"}]}).to_string(),
            )
            .unwrap();
            listener.set_nonblocking(true).unwrap();
            assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
            ([old_request, latest_request], model_candidate)
        });

        #[derive(Debug)]
        struct ObservedCall {
            thread: thread::ThreadId,
            seed: String,
            result: Result<PredictionResponse, PredictionError>,
        }
        struct FixtureProvider {
            provider: HttpModelProvider,
            certificate: reqwest::Certificate,
            body_seen: Option<mpsc::Sender<()>>,
            calls: Arc<Mutex<Vec<ObservedCall>>>,
        }
        impl PredictionProvider for FixtureProvider {
            fn predict(
                &self,
                request: &PredictionRequest,
                cancellation: &PredictionCancellation,
            ) -> Result<PredictionResponse, PredictionError> {
                // Only worker-local fixture trust changes. Both requests use
                // the real provider, client, runtime, cancellation and worker.
                CLIENT.with(|slot| {
                    let mut slot = slot.borrow_mut();
                    assert!(slot.is_none() || self.calls.lock().unwrap().len() == 1);
                    if slot.is_none() {
                        *slot = Some(
                            CloudTransport::new(
                                client_builder().add_root_certificate(self.certificate.clone()),
                            )
                            .unwrap(),
                        );
                        BODY_CHUNK_SEEN.with(|slot| *slot.borrow_mut() = self.body_seen.clone());
                    }
                });
                let result = self.provider.predict(request, cancellation);
                self.calls.lock().unwrap().push(ObservedCall {
                    thread: thread::current().id(),
                    seed: request.input.seed_text.clone(),
                    result: result.clone(),
                });
                result
            }
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = Arc::new(FixtureProvider {
            provider: HttpModelProvider::new(ModelProviderConfig {
                scope: ModelScope::Cloud,
                cloud_consent: true,
                endpoint: url,
                model: "synthetic-cloud-model".into(),
                timeout_ms: 5000,
                api_key_env: None,
                ..Default::default()
            }),
            certificate,
            body_seen: stall_body.then_some(body_seen),
            calls: calls.clone(),
        });
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.set_preferences_enabled(false);
        engine.enable_ibus_candidate_mix();
        engine.configure_prediction_provider(Some(provider));
        engine.seed("hel");
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        if stall_body {
            // Server flush alone cannot prove the response body was consumed.
            // Observe the production chunk loop before cancelling its wait.
            body_received.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        let cancelled_at = Instant::now();
        engine.seed("hello i");
        engine.seed("hello f");
        let observation_deadline = cancelled_at + Duration::from_secs(2);
        while engine.prediction_pending() && Instant::now() < observation_deadline {
            engine.poll_prediction();
            if engine.prediction_pending() {
                thread::sleep(Duration::from_millis(2));
            }
        }
        let (requests, model_candidate) = server.join().unwrap();
        assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
        assert!(cancelled_at.elapsed() < Duration::from_secs(2));
        assert_eq!(engine.snapshot().seed_text, "hello f");
        assert!(engine.snapshot().committed_text.is_empty());
        assert!(
            engine
                .candidates()
                .iter()
                .any(|candidate| candidate.text == model_candidate
                    && candidate.source == CandidateSource::Model)
        );
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "no retries or intermediate draft requests");
        assert_eq!(calls[0].seed, "hel");
        assert_eq!(calls[0].result, Err(PredictionError::Cancelled));
        assert_eq!(calls[1].seed, "hello f");
        assert!(calls[1].result.is_ok());
        assert_eq!(
            calls[0].thread, calls[1].thread,
            "the same PredictionWorker handles both drafts"
        );
        for (request, expected) in requests.into_iter().zip(["hel", "hello f"]) {
            let body: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            let input: serde_json::Value =
                serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
            assert_eq!(input["raw_composition"], expected);
        }
    }

    #[test]
    fn sent_cloud_request_waiting_for_headers_yields_to_latest_draft_on_same_engine_worker() {
        latest_draft_replaces_a_sent_stalled_cloud_request(false);
    }

    #[test]
    fn sent_cloud_request_waiting_for_body_yields_to_latest_draft_on_same_engine_worker() {
        latest_draft_replaces_a_sent_stalled_cloud_request(true);
    }

    #[test]
    fn one_tls_connection_reuses_transport_without_retaining_request_credentials() {
        use std::io::Write;
        let keys = [Some("synthetic-first"), Some("synthetic-second"), None];
        let (url, cert, server) = tls_server(move |mut stream| {
            for (index, key) in keys.into_iter().enumerate() {
                let request = test_server::read_request(&mut stream).unwrap();
                assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
                let headers = request.split_once("\r\n\r\n").unwrap().0;
                let authorization = headers
                    .lines()
                    .filter_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("authorization")
                            .then(|| value.trim())
                    })
                    .collect::<Vec<_>>();
                let expected = key.map(|key| format!("Bearer {key}"));
                assert_eq!(
                    authorization,
                    expected.as_deref().into_iter().collect::<Vec<_>>()
                );
                let connection = if index == 2 { "close" } else { "keep-alive" };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: {connection}\r\n\r\n{{}}"
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.flush().unwrap();
            }
        });
        // The fixture accepts exactly one socket and handles all three requests
        // on it. No test certificate or credential enters the production cache.
        let client = CloudTransport::new(client_builder().add_root_certificate(cert)).unwrap();
        for key in keys {
            assert_eq!(
                https_request(
                    &client,
                    &url,
                    "POST",
                    Some("{}"),
                    authorization(key.map(|_| "TEST_KEY"), |_| key.map(String::from)).unwrap(),
                    Duration::from_secs(2),
                ),
                Ok("{}".into())
            );
        }
        server.join().unwrap();
    }

    #[test]
    fn https_supports_bearer_auth_unicode_and_arbitrary_model_ids() {
        let (url, cert, server) = tls_server(|mut stream| {
            let request = test_server::read_request(&mut stream).unwrap();
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer synthetic-key\r\n")
            );
            let body: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["model"], "unrelated-family/custom-model");
            assert!(
                body["messages"][1]["content"]
                    .as_str()
                    .unwrap()
                    .contains("你好")
            );
            test_server::respond(
                &mut stream,
                "200 OK",
                r#"{"choices":[{"message":{"content":"你好世界\n你好朋友"}}]}"#,
            )
            .unwrap();
        });
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            cloud_consent: true,
            endpoint: url.clone(),
            model: "unrelated-family/custom-model".into(),
            ..Default::default()
        };
        let provider = super::super::HttpModelProvider::new(config);
        let request = crate::languages::llm::LlmCompletionRequest {
            language_id: "zh-Hans".into(),
            seed_text: "你好".into(),
            normalized_phrase: "你好".into(),
            context_before_cursor: String::new(),
            confidence: 1.0,
            degraded: false,
        };
        // Only this test client trusts the ephemeral fixture certificate.
        let client = CloudTransport::new(client_builder().add_root_certificate(cert)).unwrap();
        let response = https_request(
            &client,
            &url,
            "POST",
            Some(&provider.request_body(&request)),
            authorization(Some("TEST_KEY"), |_| Some("synthetic-key".into())).unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            super::super::parse_chat_completion_candidates(&response, None),
            ["你好世界", "你好朋友"]
        );
        server.join().unwrap();
    }

    #[test]
    fn cloud_provider_runs_all_three_language_profiles_through_verified_https() {
        use crate::languages::llm::{LlmCompletionProvider, LlmCompletionRequest};
        for (language, seed, expected) in [
            ("en", "hel", "hello"),
            ("zh-Hans", "你好", "你好世界"),
            ("ja", "日本語", "日本語入力"),
        ] {
            let (url, cert, server) = tls_server(move |mut stream| {
                let request = test_server::read_request(&mut stream).unwrap();
                let body: serde_json::Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                let input: serde_json::Value =
                    serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
                assert_eq!(input["language"], language);
                assert_eq!(body["model"], "any-model-family");
                test_server::respond(
                    &mut stream,
                    "200 OK",
                    &serde_json::json!({"choices":[{"message":{"content":expected}}]}).to_string(),
                )
                .unwrap();
            });
            CLIENT.with(|slot| {
                *slot.borrow_mut() =
                    Some(CloudTransport::new(client_builder().add_root_certificate(cert)).unwrap())
            });
            let provider = super::super::HttpModelProvider::new(ModelProviderConfig {
                scope: ModelScope::Cloud,
                cloud_consent: true,
                endpoint: url,
                model: "any-model-family".into(),
                ..Default::default()
            });
            let candidates = provider
                .generate_checked(&LlmCompletionRequest {
                    language_id: language.into(),
                    seed_text: seed.into(),
                    normalized_phrase: seed.into(),
                    context_before_cursor: String::new(),
                    confidence: 1.0,
                    degraded: false,
                })
                .unwrap();
            assert_eq!(candidates[0].text, expected);
            server.join().unwrap();
            CLIENT.with(|slot| *slot.borrow_mut() = None);
        }
    }

    #[test]
    fn https_rejects_untrusted_certificates_before_sending_input() {
        let (url, _, server) = tls_server(|mut stream| {
            assert!(test_server::read_request(&mut stream).is_err());
        });
        let client = CloudTransport::new(client_builder()).unwrap();
        assert_eq!(
            https_request(
                &client,
                &url,
                "POST",
                Some("synthetic"),
                None,
                Duration::from_secs(1)
            ),
            Err(LlmProviderError::Unavailable)
        );
        server.join().unwrap();
    }

    #[test]
    fn redirects_never_forward_credentials_or_input_to_another_service() {
        use std::io::Write;
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        let location = format!("https://{}/stolen", destination.local_addr().unwrap());
        let (url, cert, server) = tls_server(move |mut stream| {
            test_server::read_request(&mut stream).unwrap();
            write!(stream, "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            stream.flush().unwrap();
        });
        let client = CloudTransport::new(client_builder().add_root_certificate(cert)).unwrap();
        assert_eq!(
            https_request(
                &client,
                &url,
                "POST",
                Some("synthetic"),
                authorization(Some("TEST_KEY"), |_| Some("synthetic-key".into())).unwrap(),
                Duration::from_secs(1)
            ),
            Err(LlmProviderError::HttpStatus(307))
        );
        server.join().unwrap();
        destination.set_nonblocking(true).unwrap();
        assert_eq!(
            destination.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn https_enforces_size_caps_and_a_total_body_deadline() {
        use std::io::Write;
        for (response, expected) in [
            (
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                    MAX_RESPONSE_BYTES + 1
                ),
                LlmProviderError::ResponseTooLarge,
            ),
            (
                format!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{}",
                    "x".repeat(MAX_RESPONSE_BYTES + 1)
                ),
                LlmProviderError::ResponseTooLarge,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nx".into(),
                LlmProviderError::Timeout,
            ),
            (
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n".into(),
                LlmProviderError::HttpStatus(401),
            ),
        ] {
            let slow = expected == LlmProviderError::Timeout;
            let (url, cert, server) = tls_server(move |mut stream| {
                test_server::read_request(&mut stream).unwrap();
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                if slow {
                    thread::sleep(Duration::from_millis(600));
                }
            });
            let client = CloudTransport::new(client_builder().add_root_certificate(cert)).unwrap();
            let started = Instant::now();
            assert_eq!(
                https_request(
                    &client,
                    &url,
                    "POST",
                    Some("synthetic"),
                    None,
                    Duration::from_millis(300)
                ),
                Err(expected)
            );
            assert!(started.elapsed() < Duration::from_secs(1));
            server.join().unwrap();
        }
    }

    #[test]
    fn cloud_urls_require_tls_and_cannot_embed_credentials() {
        assert!(is_cloud_endpoint(
            "https://models.example/v1/chat/completions"
        ));
        for url in [
            "http://models.example/v1/chat/completions",
            "https://user:key@models.example/v1",
            "https://models.example/v1?key=secret",
            "https://models.example/v1#secret",
            "https://models.example:0/v1",
            "https://models.example",
            "https://models.example/\nsecret",
        ] {
            assert!(!is_cloud_endpoint(url), "{url}");
        }
    }

    #[test]
    fn credentials_are_explicit_sensitive_and_reject_header_injection() {
        assert_eq!(
            authorization(None, |_| panic!("must not search the environment")).unwrap(),
            None
        );
        let header = authorization(Some("TEST_MODEL_KEY"), |_| Some("synthetic-key".into()))
            .unwrap()
            .unwrap();
        assert!(header.is_sensitive());
        assert_eq!(header.to_str().unwrap(), "Bearer synthetic-key");
        for key in [
            None,
            Some(""),
            Some("secret\r\nx-extra: injected"),
            Some(" secret"),
        ] {
            assert_eq!(
                authorization(Some("TEST_MODEL_KEY"), |_| key.map(String::from)),
                Err(LlmProviderError::MissingCredentials)
            );
        }
        for name in ["", "1KEY", "KEY=secret", "KEY\n", "模型"] {
            assert!(!valid_env_name(name));
        }
    }

    #[test]
    fn consent_is_checked_before_credentials_or_network() {
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            endpoint: "https://unreachable.invalid/v1/chat/completions".into(),
            model: "any-model".into(),
            api_key_env: Some("SUZAKU_MISSING_TEST_KEY".into()),
            ..Default::default()
        };
        assert_eq!(
            request(
                &config,
                "POST",
                Some("synthetic"),
                Duration::from_millis(20)
            ),
            Err(LlmProviderError::CloudConsentRequired)
        );
    }
}
