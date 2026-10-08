//! The agent loop: stream a turn, run the tools the model asks for on the shared session, send
//! the results back, and repeat until the model ends its turn.

use std::sync::{Arc, Mutex, PoisonError};

use qiyas_core::{Session, Source};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::anthropic::{png_base64, INVALID_JSON};
use crate::prompt::{system_prompt, Lang, ProjectSummary};
use crate::provider::{
    AiError, ContentBlock, Message, Provider, ResultPart, Role, StopReason, StreamEvent,
    TurnRequest, Usage,
};
use crate::{catalogue, execute, ToolOutput};

/// Most tool calls in one user turn.
pub const MAX_TOOL_CALLS: usize = 40;
/// Messages kept after the first user message.
pub const KEEP_MESSAGES: usize = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct AgentSettings {
    pub model: String,
    pub effort: String,
    pub max_tokens: u32,
    pub language: Lang,
}

impl Default for AgentSettings {
    fn default() -> Self {
        AgentSettings {
            model: "claude-opus-5-5".into(),
            effort: "high".into(),
            max_tokens: 16_000,
            language: Lang::En,
        }
    }
}

/// What the agent tells the window while a turn runs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        id: String,
        /// One line for the user.
        summary: String,
        /// Entities the result is about, to highlight.
        handles: Vec<u64>,
        /// What the window should do (`show_sheet`, `zoom`, `redraw`, ...), from the tool.
        ui: Option<Value>,
        is_error: bool,
    },
    /// The model or a safety classifier declined; `category` is the API's `stop_details`.
    Refused {
        category: Option<String>,
        text: String,
    },
    /// The turn stopped early: "cancelled", "stopped after 40 steps", "max_tokens" or another
    /// stop reason.
    Stopped {
        reason: String,
    },
    /// The model finished its answer.
    Done {
        usage: Usage,
    },
}

pub struct Agent {
    provider: Box<dyn Provider>,
    session: Arc<Mutex<Option<Session>>>,
    settings: AgentSettings,
    history: Vec<Message>,
}

impl Agent {
    pub fn new(
        provider: Box<dyn Provider>,
        session: Arc<Mutex<Option<Session>>>,
        settings: AgentSettings,
    ) -> Agent {
        Agent {
            provider,
            session,
            settings,
            history: Vec::new(),
        }
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn set_settings(&mut self, settings: AgentSettings) {
        self.settings = settings;
    }

    /// Answers `user_text`, running tools on the session as the model asks, until it ends its
    /// turn, hits a limit, or `cancel` fires. Events go to `events`; a closed receiver is fine.
    pub async fn run_turn(
        &mut self,
        user_text: String,
        events: mpsc::Sender<AgentEvent>,
        cancel: CancellationToken,
    ) -> Result<(), AiError> {
        self.push_user(vec![ContentBlock::Text { text: user_text }]);
        // Built once per user turn so the cached prefix holds through the tool loop.
        let system = self.system();
        let tools = catalogue();
        let mut calls = 0;
        let mut usage = Usage::default();
        loop {
            if cancel.is_cancelled() {
                return stopped(&events, "cancelled").await;
            }
            self.trim();
            let req = TurnRequest {
                system: system.clone(),
                messages: self.history.clone(),
                tools: tools.clone(),
                model: self.settings.model.clone(),
                effort: self.settings.effort.clone(),
                max_tokens: self.settings.max_tokens,
            };
            let Some(turn) = self.stream(req, &events, &cancel).await else {
                // The partial answer is dropped; the next user message joins this turn's.
                return stopped(&events, "cancelled").await;
            };
            let turn = turn?;
            usage.add(turn.usage);
            match turn.stop_reason {
                StopReason::Refusal { category, text } => {
                    // A refusal can cut a tool call off; nothing of it is kept or run.
                    let _ = events.send(AgentEvent::Refused { category, text }).await;
                    return Ok(());
                }
                StopReason::ToolUse => {
                    let assistant = Message::assistant(turn.content);
                    let results = self.run_tools(&assistant, &mut calls, &events).await;
                    self.history.push(assistant);
                    self.history.push(Message::user(results));
                    if calls >= MAX_TOOL_CALLS {
                        return stopped(&events, &format!("stopped after {MAX_TOOL_CALLS} steps"))
                            .await;
                    }
                }
                reason => {
                    let assistant = Message::assistant(turn.content);
                    // Tool calls cut off at max_tokens are answered, never run.
                    let unrun: Vec<ContentBlock> = assistant
                        .tool_uses()
                        .map(|(id, _, _)| error_result(id, "Not run: the answer was cut off."))
                        .collect();
                    self.history.push(assistant);
                    if !unrun.is_empty() {
                        self.history.push(Message::user(unrun));
                    }
                    return match reason {
                        StopReason::EndTurn => {
                            let _ = events.send(AgentEvent::Done { usage }).await;
                            Ok(())
                        }
                        StopReason::MaxTokens => stopped(&events, "max_tokens").await,
                        StopReason::Other(r) => stopped(&events, &r).await,
                        _ => Ok(()),
                    };
                }
            }
        }
    }

    /// Streams one turn, forwarding text and thinking as they arrive. `None` when cancelled:
    /// the HTTP request is dropped mid-stream.
    async fn stream(
        &self,
        req: TurnRequest,
        events: &mpsc::Sender<AgentEvent>,
        cancel: &CancellationToken,
    ) -> Option<Result<crate::provider::TurnResult, AiError>> {
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamEvent>();
        let turn = async move {
            let mut sink = move |e: StreamEvent| {
                let _ = tx.send(e);
            };
            // `sink` (and the sender in it) is dropped when the turn ends, which ends `forward`.
            self.provider.stream_turn(req, &mut sink).await
        };
        let forward = async {
            while let Some(e) = rx.recv().await {
                let event = match e {
                    StreamEvent::TextDelta(text) => AgentEvent::Text { text },
                    StreamEvent::Thinking(text) => AgentEvent::Thinking { text },
                    _ => continue,
                };
                let _ = events.send(event).await;
            }
        };
        tokio::select! {
            (result, ()) = async { tokio::join!(turn, forward) } => Some(result),
            () = cancel.cancelled() => None,
        }
    }

    /// Runs the tool calls of `assistant` in order and returns one result block for each, all
    /// for one user message. Past `MAX_TOOL_CALLS` the calls are answered but not run.
    async fn run_tools(
        &self,
        assistant: &Message,
        calls: &mut usize,
        events: &mpsc::Sender<AgentEvent>,
    ) -> Vec<ContentBlock> {
        let mut results = Vec::new();
        for (id, name, input) in assistant.tool_uses() {
            let _ = events
                .send(AgentEvent::ToolCall {
                    id: id.to_string(),
                    name: name.to_string(),
                    input: input.clone(),
                })
                .await;
            let (block, event) = if *calls >= MAX_TOOL_CALLS {
                let why = "Not run: the limit of tool calls for this message was reached.";
                (error_result(id, why), error_event(id, why))
            } else if !input.is_object() {
                (
                    error_result(id, INVALID_JSON),
                    error_event(id, INVALID_JSON),
                )
            } else {
                *calls += 1;
                self.run_tool(id, name, input)
            };
            results.push(block);
            let _ = events.send(event).await;
        }
        results
    }

    /// Locks the session, runs one tool, and lets go of the lock before anything else happens.
    fn run_tool(&self, id: &str, name: &str, input: &Value) -> (ContentBlock, AgentEvent) {
        let output = {
            let mut guard = self.session.lock().unwrap_or_else(PoisonError::into_inner);
            match guard.as_mut() {
                Some(session) => execute(session, name, input, Source::Ai),
                None => {
                    let why = "No drawing is open. Ask the user to open one.";
                    return (error_result(id, why), error_event(id, why));
                }
            }
        };
        match output {
            Err(e) => {
                let why = e.to_string();
                (error_result(id, &why), error_event(id, &why))
            }
            Ok(ToolOutput::Json(value)) => {
                let event = AgentEvent::ToolResult {
                    id: id.to_string(),
                    summary: summary(&value),
                    handles: handles(&value),
                    ui: value.get("ui").cloned(),
                    is_error: false,
                };
                let block = ContentBlock::ToolResult {
                    tool_use_id: id.to_string(),
                    content: vec![ResultPart::Text(value.to_string())],
                    is_error: false,
                };
                (block, event)
            }
            Ok(ToolOutput::Image { png, caption }) => {
                let event = AgentEvent::ToolResult {
                    id: id.to_string(),
                    summary: caption.clone(),
                    handles: Vec::new(),
                    ui: None,
                    is_error: false,
                };
                let block = ContentBlock::ToolResult {
                    tool_use_id: id.to_string(),
                    content: vec![ResultPart::Text(caption), ResultPart::Png(png_base64(&png))],
                    is_error: false,
                };
                (block, event)
            }
        }
    }

    fn system(&self) -> String {
        let guard = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        let (units, summary) = match guard.as_ref() {
            Some(s) => (s.info().units, ProjectSummary::of(s)),
            None => (String::new(), ProjectSummary::none()),
        };
        drop(guard);
        system_prompt(self.settings.language, &units, &summary)
    }

    /// Adds user content, joining it to a user message already last (after a cancel or a
    /// refusal), so roles keep alternating.
    fn push_user(&mut self, content: Vec<ContentBlock>) {
        match self.history.last_mut() {
            Some(last) if last.role == Role::User => last.content.extend(content),
            _ => self.history.push(Message::user(content)),
        }
    }

    /// Keeps the first user message and about the last `KEEP_MESSAGES`, starting the kept part
    /// at an assistant message so no tool result loses its tool call.
    fn trim(&mut self) {
        let len = self.history.len();
        if len <= KEEP_MESSAGES + 1 {
            return;
        }
        let mut start = len - KEEP_MESSAGES;
        while start < len && self.history[start].role != Role::Assistant {
            start += 1;
        }
        if start >= len {
            return;
        }
        self.history.drain(1..start);
    }
}

async fn stopped(events: &mpsc::Sender<AgentEvent>, reason: &str) -> Result<(), AiError> {
    let _ = events
        .send(AgentEvent::Stopped {
            reason: reason.to_string(),
        })
        .await;
    Ok(())
}

fn error_result(id: &str, why: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: vec![ResultPart::Text(why.to_string())],
        is_error: true,
    }
}

fn error_event(id: &str, why: &str) -> AgentEvent {
    AgentEvent::ToolResult {
        id: id.to_string(),
        summary: why.to_string(),
        handles: Vec::new(),
        ui: None,
        is_error: true,
    }
}

/// One line about a tool's JSON result.
fn summary(value: &Value) -> String {
    if let Some(m) = value.get("measurement").filter(|m| m.is_object()) {
        let shown = m["shown"].as_f64().unwrap_or(0.0);
        return format!(
            "{shown:.3} {}: {}",
            m["unit"].as_str().unwrap_or(""),
            m["description"].as_str().unwrap_or("")
        );
    }
    if let Some(total) = value.get("total").and_then(Value::as_u64) {
        return format!("{total} found");
    }
    if let Some(removed) = value.get("removed").and_then(Value::as_str) {
        return format!("Removed {removed}");
    }
    "Done".to_string()
}

/// The entity handles a result is about: a measurement's, or a zoom's.
fn handles(value: &Value) -> Vec<u64> {
    let list = if value["measurement"]["handles"].is_array() {
        &value["measurement"]["handles"]
    } else {
        &value["ui"]["handles"]
    };
    list.as_array()
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{BoxFuture, Sink, TurnResult};
    use serde_json::json;

    struct Never;
    impl Provider for Never {
        fn stream_turn<'a>(
            &'a self,
            _: TurnRequest,
            _: Sink<'a>,
        ) -> BoxFuture<'a, Result<TurnResult, AiError>> {
            Box::pin(async { Err(AiError::Network("unused".into())) })
        }
    }

    fn text(role: Role, t: &str) -> Message {
        Message {
            role,
            content: vec![ContentBlock::Text { text: t.into() }],
        }
    }

    #[test]
    fn trim_keeps_first_message_and_tool_pairs() {
        let mut agent = Agent::new(
            Box::new(Never),
            Arc::new(Mutex::new(None)),
            AgentSettings::default(),
        );
        agent.history.push(text(Role::User, "first"));
        for i in 0..40 {
            agent
                .history
                .push(Message::assistant(vec![ContentBlock::ToolUse {
                    id: format!("t{i}"),
                    name: "list_sheets".into(),
                    input: json!({}),
                }]));
            agent
                .history
                .push(Message::user(vec![error_result(&format!("t{i}"), "x")]));
        }
        agent.trim();
        let h = &agent.history;
        assert!(h.len() <= KEEP_MESSAGES + 1);
        assert_eq!(h[0], text(Role::User, "first"));
        assert_eq!(h[1].role, Role::Assistant);
        // Every tool result answers the tool call just before it.
        for pair in h[1..].chunks(2) {
            let (id, _, _) = pair[0].tool_uses().next().unwrap();
            assert!(matches!(&pair[1].content[0],
                ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == id));
        }
    }
}
