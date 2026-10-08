//! The agent loop and both providers against a local mock HTTP server that serves canned
//! Server-Sent Events. No real API is called.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use qiyas_ai::anthropic::Anthropic;
use qiyas_ai::openai::OpenAiCompatible;
use qiyas_ai::{Agent, AgentEvent, AgentSettings, AiError, Provider, StreamEvent, TurnRequest};
use qiyas_core::Session;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const KEY: &str = "sk-ant-test-SECRET-0123456789";

// ---------------------------------------------------------------------------------------------
// The mock server

/// What the mock answers one request with.
enum Reply {
    /// 200 with these SSE events, each written separately with a short pause.
    Sse(Vec<Value>),
    /// 200 with these raw `data:` payloads (OpenAI style, ends with `[DONE]`).
    Data(Vec<String>),
    /// A status, extra headers and a body.
    Status(u16, Vec<(&'static str, &'static str)>, String),
    /// 200, a text block that gets one `text_delta` every 200 ms until the client hangs up.
    Drip,
}

#[derive(Debug, Clone)]
struct Request {
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

struct Mock {
    base: String,
    requests: Arc<Mutex<Vec<Request>>>,
    /// Set when a dripping reply finds the client gone.
    hung_up: Arc<AtomicBool>,
}

impl Mock {
    fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    fn anthropic(&self) -> Anthropic {
        Anthropic::with_url(KEY.to_string(), format!("{}/v1/messages", self.base))
    }
}

/// Starts a server on a free local port; `reply(n)` answers the n-th request (0-based).
async fn serve(reply: impl Fn(usize) -> Reply + Send + Sync + 'static) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let hung_up = Arc::new(AtomicBool::new(false));
    let reply = Arc::new(reply);
    let (reqs, hung) = (requests.clone(), hung_up.clone());
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let (reqs, hung, reply) = (reqs.clone(), hung.clone(), reply.clone());
            tokio::spawn(async move {
                let Some(request) = read_request(&stream).await else {
                    return;
                };
                let n = {
                    let mut r = reqs.lock().unwrap();
                    r.push(request);
                    r.len() - 1
                };
                if write_reply(stream, reply(n)).await.is_err() {
                    hung.store(true, Ordering::SeqCst);
                }
            });
        }
    });
    Mock {
        base,
        requests,
        hung_up,
    }
}

async fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        stream.readable().await.ok()?;
        match stream.try_read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => return None,
        }
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let path = lines.next()?.split(' ').nth(1)?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        stream.readable().await.ok()?;
        match stream.try_read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => return None,
        }
    }
    Some(Request {
        path,
        headers,
        body: serde_json::from_slice(&body).unwrap_or(Value::Null),
    })
}

const SSE_HEAD: &str = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                        cache-control: no-cache\r\nconnection: close\r\n\r\n";

async fn write_reply(mut stream: TcpStream, reply: Reply) -> std::io::Result<()> {
    match reply {
        Reply::Sse(events) => {
            stream.write_all(SSE_HEAD.as_bytes()).await?;
            for e in events {
                let name = e["type"].as_str().unwrap_or("").to_string();
                let frame = format!("event: {name}\ndata: {e}\n\n");
                stream.write_all(frame.as_bytes()).await?;
                stream.flush().await?;
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        Reply::Data(lines) => {
            stream.write_all(SSE_HEAD.as_bytes()).await?;
            for d in lines {
                stream
                    .write_all(format!("data: {d}\n\n").as_bytes())
                    .await?;
                stream.flush().await?;
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        Reply::Status(status, headers, body) => {
            let mut head = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n\
                 content-length: {}\r\nconnection: close\r\n",
                body.len()
            );
            for (n, v) in headers {
                head.push_str(&format!("{n}: {v}\r\n"));
            }
            head.push_str("\r\n");
            stream.write_all(head.as_bytes()).await?;
            stream.write_all(body.as_bytes()).await?;
        }
        Reply::Drip => {
            stream.write_all(SSE_HEAD.as_bytes()).await?;
            for e in [
                message_start(),
                json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            ] {
                stream
                    .write_all(format!("data: {e}\n\n").as_bytes())
                    .await?;
            }
            loop {
                let e = json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"la "}});
                stream
                    .write_all(format!("event: content_block_delta\ndata: {e}\n\n").as_bytes())
                    .await?;
                stream.flush().await?;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    stream.shutdown().await
}

// ---------------------------------------------------------------------------------------------
// Canned Claude answers

fn message_start() -> Value {
    json!({"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant",
        "model":"claude-opus-5-5","content":[],"stop_reason":null,
        "usage":{"input_tokens":10,"output_tokens":1}}})
}

fn stop(reason: &str) -> Vec<Value> {
    vec![
        json!({"type":"message_delta","delta":{"stop_reason":reason},"usage":{"output_tokens":5}}),
        json!({"type":"message_stop"}),
    ]
}

/// A text answer streamed in `parts`, ending the turn.
fn text_turn(parts: &[&str]) -> Reply {
    let mut e = vec![
        message_start(),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"ping"}),
    ];
    for p in parts {
        e.push(
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":p}}),
        );
    }
    e.push(json!({"type":"content_block_stop","index":0}));
    e.extend(stop("end_turn"));
    Reply::Sse(e)
}

/// One `tool_use` block per call `(id, name, input fragments)`, stopping for tool use.
fn tool_turn(calls: &[(&str, &str, &[&str])]) -> Reply {
    let mut e = vec![message_start()];
    for (i, (id, name, fragments)) in calls.iter().enumerate() {
        e.push(json!({"type":"content_block_start","index":i,
            "content_block":{"type":"tool_use","id":id,"name":name,"input":{}}}));
        for f in *fragments {
            e.push(json!({"type":"content_block_delta","index":i,
                "delta":{"type":"input_json_delta","partial_json":f}}));
        }
        e.push(json!({"type":"content_block_stop","index":i}));
    }
    e.extend(stop("tool_use"));
    Reply::Sse(e)
}

// ---------------------------------------------------------------------------------------------
// Helpers

fn sample() -> (tempfile::TempDir, Arc<Mutex<Option<Session>>>) {
    let dir = tempfile::tempdir().unwrap();
    let path = qiyas_core::fixtures::write_sample(dir.path(), "dxf");
    let session = Session::open(&path).unwrap();
    (dir, Arc::new(Mutex::new(Some(session))))
}

/// Runs one user turn and returns its result and every event it sent.
async fn turn(agent: &mut Agent, text: &str) -> (Result<(), AiError>, Vec<AgentEvent>) {
    let (tx, mut rx) = mpsc::channel(10_000);
    let result = agent
        .run_turn(text.to_string(), tx, CancellationToken::new())
        .await;
    let mut events = Vec::new();
    while let Ok(e) = rx.try_recv() {
        events.push(e);
    }
    (result, events)
}

fn texts(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Tests

#[tokio::test]
async fn text_stream_is_forwarded_in_order() {
    let mock = serve(|_| text_turn(&["Hel", "lo", ", wor", "ld"])).await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session,
        AgentSettings::default(),
    );

    let (result, events) = turn(&mut agent, "Hi").await;
    result.unwrap();
    assert_eq!(texts(&events), ["Hel", "lo", ", wor", "ld"]);
    assert!(
        matches!(events.last(), Some(AgentEvent::Done { .. })),
        "{events:?}"
    );

    // The request is the one the API expects.
    let reqs = mock.requests();
    assert_eq!(reqs.len(), 1);
    let r = &reqs[0];
    assert_eq!(r.path, "/v1/messages");
    assert_eq!(r.header("x-api-key"), Some(KEY));
    assert_eq!(r.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(
        r.header("anthropic-beta"),
        Some("server-side-fallback-2026-07-01")
    );
    let b = &r.body;
    assert_eq!(b["model"], "claude-opus-5-5");
    assert_eq!(b["max_tokens"], 16000);
    assert_eq!(b["stream"], true);
    assert_eq!(b["output_config"], json!({"effort": "high"}));
    assert_eq!(b["fallbacks"], "default");
    assert!(b.get("thinking").is_none());
    assert!(b.get("tool_choice").is_none());
    assert_eq!(b["system"][0]["type"], "text");
    assert_eq!(
        b["system"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert!(b["system"][0]["text"].as_str().unwrap().contains("Qiyas"));
    let tools = b["tools"].as_array().unwrap();
    assert!(tools.iter().all(|t| t["strict"] == true));
    assert_eq!(
        tools.last().unwrap()["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert!(tools[..tools.len() - 1]
        .iter()
        .all(|t| t.get("cache_control").is_none()));
    assert_eq!(b["cache_control"], json!({"type": "ephemeral"}));
    // One user message: the [state] block, then what the user typed.
    let messages = b["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    let content = messages[0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert!(content[0]["text"].as_str().unwrap().starts_with("[state]"));
    assert_eq!(content[1], json!({"type": "text", "text": "Hi"}));

    // The answer is kept for the next turn.
    let h = agent.history();
    assert_eq!(h.len(), 2);
    assert_eq!(h[0].content.len(), 2);
    assert_eq!(
        h[1].content,
        vec![qiyas_ai::ContentBlock::Text {
            text: "Hello, world".into()
        }]
    );
}

#[tokio::test]
async fn tool_use_round_trip_executes_against_the_session() {
    let mock = serve(|n| match n {
        // Two calls in one answer: the input of the first arrives in pieces.
        0 => tool_turn(&[
            (
                "toolu_1",
                "measure_area",
                &[r#"{"points": [[0, 0], [10"#, r#", 0], [10, 10], [0, 10]]}"#],
            ),
            ("toolu_2", "list_layers", &[]),
        ]),
        _ => text_turn(&["Measured 100 m2."]),
    })
    .await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session.clone(),
        AgentSettings::default(),
    );

    let (result, events) = turn(&mut agent, "Measure the slab").await;
    result.unwrap();

    // The session gained one measurement, made by the AI.
    {
        let guard = session.lock().unwrap();
        let items = &guard.as_ref().unwrap().takeoff.items;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "ai");
    }

    // Events: the call, then its result, for each tool, then the closing text.
    let calls: Vec<(&str, bool)> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolCall { name, mutating, .. } => Some((name.as_str(), *mutating)),
            _ => None,
        })
        .collect();
    assert_eq!(calls, [("measure_area", true), ("list_layers", false)]);
    let first_call = events
        .iter()
        .position(|e| matches!(e, AgentEvent::ToolCall { id, .. } if id == "toolu_1"))
        .unwrap();
    match &events[first_call + 1] {
        AgentEvent::ToolResult {
            id,
            is_error,
            summary,
            ..
        } => {
            assert_eq!(id, "toolu_1");
            assert!(!is_error, "{summary}");
        }
        other => panic!("expected the tool result, got {other:?}"),
    }
    assert_eq!(texts(&events), ["Measured 100 m2."]);

    // The second request sends both results back in ONE user message, after the assistant's
    // tool calls kept unchanged.
    let reqs = mock.requests();
    assert_eq!(reqs.len(), 2);
    let messages = reqs[1].body["messages"].as_array().unwrap().clone();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"][0]["type"], "tool_use");
    assert_eq!(
        messages[1]["content"][0]["input"],
        json!({"points": [[0, 0], [10, 0], [10, 10], [0, 10]]})
    );
    assert_eq!(messages[2]["role"], "user");
    let results = messages[2]["content"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["type"], "tool_result");
    assert_eq!(results[0]["tool_use_id"], "toolu_1");
    assert_eq!(results[1]["tool_use_id"], "toolu_2");
    assert!(results.iter().all(|r| r.get("is_error").is_none()));
}

#[tokio::test]
async fn invalid_tool_json_is_answered_with_an_error_and_not_run() {
    let mock = serve(|n| match n {
        0 => tool_turn(&[("toolu_1", "measure_area", &[r#"{"points": [[0, "#])]),
        _ => text_turn(&["Sorry."]),
    })
    .await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session.clone(),
        AgentSettings::default(),
    );
    let (result, events) = turn(&mut agent, "Measure").await;
    result.unwrap();
    assert!(session
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .takeoff
        .items
        .is_empty());
    assert!(
        events.iter().any(|e| matches!(e,
        AgentEvent::ToolResult { is_error: true, summary, .. } if summary == "invalid JSON input"))
    );
    let body = &mock.requests()[1].body;
    let result = &body["messages"][2]["content"][0];
    assert_eq!(result["is_error"], true);
    assert_eq!(result["content"][0]["text"], "invalid JSON input");
    // The broken input goes back as an empty object, so the history stays valid.
    assert_eq!(body["messages"][1]["content"][0]["input"], json!({}));
}

#[tokio::test]
async fn refusal_stop_reason_surfaces_category() {
    let mock = serve(|_| {
        Reply::Sse(vec![
            message_start(),
            json!({"type":"message_delta",
                "delta":{"stop_reason":"refusal","stop_details":{"type":"refusal",
                    "category":"cyber","explanation":"This looks like malware work."}},
                "usage":{"output_tokens":0}}),
            json!({"type":"message_stop"}),
        ])
    })
    .await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session,
        AgentSettings::default(),
    );
    let (result, events) = turn(&mut agent, "Hi").await;
    result.unwrap();
    assert!(
        events.iter().any(|e| matches!(e,
            AgentEvent::Refused { category: Some(c), text }
                if c == "cyber" && text == "This looks like malware work.")),
        "{events:?}"
    );
    // Nothing of the declined answer is kept; the user's message stays, last.
    assert_eq!(agent.history().len(), 1);
}

#[tokio::test]
async fn loop_stops_after_40_tool_calls() {
    let mock = serve(|n| {
        let id = format!("toolu_{n}");
        let id: &'static str = Box::leak(id.into_boxed_str());
        tool_turn(&[(id, "list_layers", &["{}"])])
    })
    .await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session,
        AgentSettings::default(),
    );
    let (result, events) = turn(&mut agent, "Loop forever").await;
    result.unwrap();

    let run = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AgentEvent::ToolResult {
                    is_error: false,
                    ..
                }
            )
        })
        .count();
    assert_eq!(run, 40);
    assert_eq!(mock.requests().len(), 40);
    assert_eq!(
        events.last(),
        Some(&AgentEvent::Stopped {
            reason: "stopped after 40 steps".into()
        })
    );
    // Every call made was answered, so the history is valid to continue from.
    let last = agent.history().last().unwrap();
    assert!(last.has_tool_results());
}

#[tokio::test]
async fn malformed_tool_input_still_stops_after_40_calls() {
    let mock = serve(|n| {
        let id: &'static str = Box::leak(format!("toolu_{n}").into_boxed_str());
        tool_turn(&[(id, "measure_area", &[r#"{"points": [[0, "#])])
    })
    .await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session,
        AgentSettings::default(),
    );
    let (result, events) = turn(&mut agent, "Loop").await;
    result.unwrap();
    let answered = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ToolResult { is_error: true, .. }))
        .count();
    assert_eq!(answered, 40);
    assert_eq!(mock.requests().len(), 40);
    assert_eq!(
        events.last(),
        Some(&AgentEvent::Stopped {
            reason: "stopped after 40 steps".into()
        })
    );
}

/// Each message of a request body as its JSON text.
fn serialised(body: &Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::to_string)
        .collect()
}

#[tokio::test]
async fn history_is_append_only_and_system_prompt_is_stable() {
    let mock = serve(|n| match n {
        0 => tool_turn(&[(
            "toolu_1",
            "measure_area",
            &[r#"{"points": [[0, 0], [10, 0], [10, 10], [0, 10]]}"#],
        )]),
        _ => text_turn(&["Done."]),
    })
    .await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session,
        AgentSettings::default(),
    );
    turn(&mut agent, "Measure the slab").await.0.unwrap();
    turn(&mut agent, "And now?").await.0.unwrap();

    let reqs = mock.requests();
    assert_eq!(reqs.len(), 3);
    // The take-off changed between the turns, yet the system prompt and tools did not.
    let system = |r: &Request| r.body["system"].to_string();
    assert_eq!(system(&reqs[0]), system(&reqs[2]));
    assert_eq!(system(&reqs[1]), system(&reqs[2]));
    assert_eq!(reqs[0].body["tools"], reqs[2].body["tools"]);
    assert!(
        !system(&reqs[2]).contains("m2"),
        "no live take-off in the system prompt"
    );

    // Each request only appends to the one before it.
    for pair in reqs.windows(2) {
        let (before, after) = (serialised(&pair[0].body), serialised(&pair[1].body));
        assert!(after.len() > before.len());
        assert_eq!(
            after[..before.len()],
            before[..],
            "an earlier message changed"
        );
    }

    // The second user message starts with the state block, which now shows the measurement.
    let messages = reqs[2].body["messages"].as_array().unwrap();
    let last = messages.last().unwrap();
    assert_eq!(last["role"], "user");
    let state = last["content"][0]["text"].as_str().unwrap();
    assert!(state.starts_with("[state]"), "{state}");
    assert!(
        state.contains("- area: 1 measurement, total 100.000 m2"),
        "{state}"
    );
    assert_eq!(last["content"][1]["text"], "And now?");
    let first_state = messages[0]["content"][0]["text"].as_str().unwrap();
    assert!(first_state.contains("no measurements yet"), "{first_state}");
}

#[tokio::test]
async fn cancel_stops_within_one_second() {
    let mock = serve(|_| Reply::Drip).await;
    let (_dir, session) = sample();
    let mut agent = Agent::new(
        Box::new(mock.anthropic()),
        session,
        AgentSettings::default(),
    );
    let (tx, mut rx) = mpsc::channel(10_000);
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let cancelled_at = Arc::new(Mutex::new(None));
    let at = cancelled_at.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(700)).await;
        *at.lock().unwrap() = Some(Instant::now());
        trigger.cancel();
    });

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        agent.run_turn("Talk".into(), tx, cancel),
    )
    .await
    .expect("the turn did not stop");
    let returned = Instant::now();
    result.unwrap();
    let cancelled = cancelled_at.lock().unwrap().unwrap();
    assert!(returned - cancelled < Duration::from_secs(1));

    let mut events = Vec::new();
    while let Ok(e) = rx.try_recv() {
        events.push(e);
    }
    assert!(
        !texts(&events).is_empty(),
        "the drip should have arrived first"
    );
    assert_eq!(
        events.last(),
        Some(&AgentEvent::Stopped {
            reason: "cancelled".into()
        })
    );

    // The HTTP stream was dropped: the server sees the client gone.
    let start = Instant::now();
    while !mock.hung_up.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(2) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(mock.hung_up.load(Ordering::SeqCst));
}

#[tokio::test]
async fn api_key_absent_from_logs_and_errors() {
    let echo = format!(
        r#"{{"type":"error","error":{{"type":"invalid_request_error","message":"bad key {KEY}"}}}}"#
    );
    let mock = serve(move |n| match n {
        0 => Reply::Status(401, vec![], r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.into()),
        1 => Reply::Status(400, vec![], echo.clone()),
        // 429 twice: one retry, then the error.
        2 | 3 => Reply::Status(429, vec![("retry-after", "0")], "{}".into()),
        4 => Reply::Status(500, vec![], format!("internal {KEY}")),
        _ => Reply::Sse(vec![
            message_start(),
            json!({"type":"error","error":{"type":"overloaded_error","message":format!("echo {KEY}")}}),
        ]),
    })
    .await;
    let provider = mock.anthropic();
    let req = TurnRequest {
        system: "s".into(),
        messages: vec![qiyas_ai::Message::user(vec![
            qiyas_ai::ContentBlock::Text { text: "hi".into() },
        ])],
        tools: vec![],
        model: "claude-opus-5-5".into(),
        effort: "high".into(),
        max_tokens: 100,
    };
    let mut errors = Vec::new();
    for _ in 0..5 {
        let mut sink = |_: StreamEvent| {};
        errors.push(
            provider
                .stream_turn(req.clone(), &mut sink)
                .await
                .unwrap_err(),
        );
    }
    assert_eq!(errors[0], AiError::Auth);
    assert!(
        matches!(&errors[1], AiError::Api { status: 400, message } if message.contains("bad key"))
    );
    assert!(matches!(
        errors[2],
        AiError::RateLimited {
            retry_after: Some(0)
        }
    ));
    assert_eq!(mock.requests().len(), 6, "a 429 is retried once");
    assert!(matches!(&errors[3], AiError::Api { status: 500, .. }));
    assert!(matches!(&errors[4], AiError::Stream(m) if m.contains("overloaded_error")));

    // Nothing reachable: a network error.
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead = format!("http://{}/v1/messages", closed.local_addr().unwrap());
    drop(closed);
    let mut sink = |_: StreamEvent| {};
    let e = Anthropic::with_url(KEY.into(), dead)
        .stream_turn(req.clone(), &mut sink)
        .await
        .unwrap_err();
    assert!(matches!(e, AiError::Network(_)), "{e:?}");
    errors.push(e);

    // The OpenAI-compatible client too.
    let mut sink = |_: StreamEvent| {};
    let e = OpenAiCompatible::new(&mock.base, Some(KEY.into()))
        .stream_turn(req.clone(), &mut sink)
        .await
        .unwrap_err();
    errors.push(e);

    // And through the agent.
    let (_dir, session) = sample();
    let mut agent = Agent::new(Box::new(provider), session, AgentSettings::default());
    let (result, events) = turn(&mut agent, "hi").await;
    errors.push(result.unwrap_err());

    for e in &errors {
        for text in [e.to_string(), format!("{e:?}")] {
            assert!(!text.contains(KEY), "the key leaked: {text}");
            assert!(!text.contains("SECRET"), "part of the key leaked: {text}");
        }
    }
    for e in &events {
        assert!(!format!("{e:?}").contains(KEY));
    }
}

#[tokio::test]
async fn openai_compatible_streams_text_and_tool_calls() {
    let mock = serve(|n| match n {
        0 => Reply::Data(vec![
            json!({"choices":[{"index":0,"delta":{"role":"assistant","content":"Let me look."}}]}).to_string(),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function",
                "function":{"name":"list_layers","arguments":""}}]}}]}).to_string(),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"off"}}]}}]}).to_string(),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"set\": 0}"}}]}}]}).to_string(),
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}).to_string(),
            "[DONE]".into(),
        ]),
        _ => Reply::Data(vec![
            json!({"choices":[{"index":0,"delta":{"content":"Three layers."}}]}).to_string(),
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}).to_string(),
            "[DONE]".into(),
        ]),
    })
    .await;
    let (_dir, session) = sample();
    let provider = OpenAiCompatible::new(&format!("{}/v1/", mock.base), Some(KEY.into()));
    let settings = AgentSettings {
        model: "gpt-test".into(),
        ..AgentSettings::default()
    };
    let mut agent = Agent::new(Box::new(provider), session, settings);
    let (result, events) = turn(&mut agent, "Which layers?").await;
    result.unwrap();
    assert_eq!(texts(&events), ["Let me look.", "Three layers."]);
    assert!(events.iter().any(|e| matches!(e,
        AgentEvent::ToolCall { id, name, input, mutating }
            if id == "call_1" && name == "list_layers" && *input == json!({"offset": 0})
                && !mutating)));

    let reqs = mock.requests();
    assert_eq!(reqs.len(), 2);
    assert_eq!(reqs[0].path, "/v1/chat/completions");
    assert_eq!(
        reqs[0].header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
    let b = &reqs[0].body;
    assert_eq!(b["model"], "gpt-test");
    assert_eq!(b["stream"], true);
    assert_eq!(b["tool_choice"], "auto");
    assert_eq!(b["tools"][0]["type"], "function");
    assert!(b["tools"][0]["function"]["parameters"].is_object());
    assert_eq!(b["messages"][0]["role"], "system");
    let second = reqs[1].body["messages"].as_array().unwrap();
    assert_eq!(second[2]["role"], "assistant");
    assert_eq!(second[2]["tool_calls"][0]["id"], "call_1");
    assert_eq!(second[3]["role"], "tool");
    assert_eq!(second[3]["tool_call_id"], "call_1");
}
