//! Any OpenAI-compatible chat service (OpenAI, OpenRouter, a local server, ...): Chat Completions
//! with streaming and function calling.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::provider::{
    http_client, read_events, send, AiError, BoxFuture, ContentBlock, Message, Provider,
    ResultPart, Role, Sink, StopReason, StreamEvent, TurnRequest, TurnResult, Usage,
};

/// An OpenAI-compatible service at `base_url` (e.g. `https://api.openai.com/v1`). The key is only
/// ever put in the `Authorization` header; this type has no `Debug`.
pub struct OpenAiCompatible {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatible {
    /// `api_key` may be `None` for local servers that take none.
    pub fn new(base_url: &str, api_key: Option<String>) -> OpenAiCompatible {
        OpenAiCompatible {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            api_key,
            client: http_client(),
        }
    }
}

impl Provider for OpenAiCompatible {
    fn stream_turn<'a>(
        &'a self,
        req: TurnRequest,
        sink: Sink<'a>,
    ) -> BoxFuture<'a, Result<TurnResult, AiError>> {
        Box::pin(async move {
            let body = request_body(&req);
            let url = format!("{}/chat/completions", self.base_url);
            let response = send(
                || {
                    let r = self.client.post(&url).json(&body);
                    match &self.api_key {
                        Some(key) => r.bearer_auth(key),
                        None => r,
                    }
                },
                self.api_key.as_deref().unwrap_or(""),
            )
            .await?;
            let mut turn = Turn::default();
            read_events(response, |e| turn.on_event(&e.data, sink))
                .await
                .map_err(|e| e.scrubbed(self.api_key.as_deref().unwrap_or("")))?;
            Ok(turn.finish(sink))
        })
    }
}

/// The JSON body. `effort` and `max_tokens` are not sent: services differ on those names
/// (`max_tokens` is refused by OpenAI's reasoning models) and each has its own default.
pub fn request_body(req: &TurnRequest) -> Value {
    let mut messages = vec![json!({ "role": "system", "content": req.system })];
    for m in &req.messages {
        push_messages(&mut messages, m);
    }
    let tools: Vec<Value> = req
        .tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                }
            })
        })
        .collect();
    let mut body = json!({
        "model": req.model,
        "stream": true,
        "messages": messages,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        body["tool_choice"] = json!("auto");
    }
    body
}

/// One of our messages as Chat Completions messages. Tool results become `tool` messages; their
/// pictures follow in one user message, as a tool message can't carry images.
fn push_messages(out: &mut Vec<Value>, m: &Message) {
    match m.role {
        Role::Assistant => {
            let text: String = m
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            let calls: Vec<Value> = m
                .tool_uses()
                .map(|(id, name, input)| {
                    // An input that was not valid JSON goes back as an empty object (its tool
                    // result says so): services refuse the whole request over bad arguments.
                    let arguments = if input.is_object() {
                        input.to_string()
                    } else {
                        "{}".to_string()
                    };
                    json!({
                        "id": id,
                        "type": "function",
                        "function": { "name": name, "arguments": arguments }
                    })
                })
                .collect();
            let mut msg = json!({ "role": "assistant", "content": text });
            if !calls.is_empty() {
                if text.is_empty() {
                    msg["content"] = Value::Null;
                }
                msg["tool_calls"] = Value::Array(calls);
            }
            out.push(msg);
        }
        Role::User => {
            let mut parts: Vec<Value> = Vec::new();
            for b in &m.content {
                match b {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => {
                        let text: Vec<&str> = content
                            .iter()
                            .filter_map(|p| match p {
                                ResultPart::Text(t) => Some(t.as_str()),
                                ResultPart::Png(_) => None,
                            })
                            .collect();
                        out.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": text.join("\n"),
                        }));
                        for p in content {
                            if let ResultPart::Png(data) = p {
                                parts.push(json!({
                                    "type": "image_url",
                                    "image_url": { "url": format!("data:image/png;base64,{data}") }
                                }));
                            }
                        }
                    }
                    ContentBlock::Text { text } => {
                        parts.push(json!({ "type": "text", "text": text }));
                    }
                    _ => {}
                }
            }
            if !parts.is_empty() {
                out.push(json!({ "role": "user", "content": parts }));
            }
        }
    }
}

/// A function call while it streams.
#[derive(Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
struct Turn {
    text: String,
    /// In the order they began.
    calls: Vec<Call>,
    /// The stream's `index` of each call to the newest call in `calls` with it.
    by_index: HashMap<u64, usize>,
    finish_reason: Option<String>,
    usage: Usage,
}

impl Turn {
    fn on_event(&mut self, data: &str, sink: Sink<'_>) -> Result<bool, AiError> {
        if data.trim() == "[DONE]" {
            return Ok(true);
        }
        let Ok(e) = serde_json::from_str::<Value>(data) else {
            return Err(AiError::Stream("an event was not valid JSON".into()));
        };
        if let Some(err) = e.get("error") {
            let message = err["message"].as_str().unwrap_or("error").to_string();
            return Err(AiError::Stream(message));
        }
        if let Some(u) = e.get("usage").filter(|u| u.is_object()) {
            self.usage.input_tokens = u["prompt_tokens"].as_u64().unwrap_or(0);
            self.usage.output_tokens = u["completion_tokens"].as_u64().unwrap_or(0);
        }
        let choice = &e["choices"][0];
        let delta = &choice["delta"];
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            self.text.push_str(text);
            sink(StreamEvent::TextDelta(text.to_string()));
        }
        for c in delta["tool_calls"].as_array().into_iter().flatten() {
            let id = c["id"].as_str().filter(|s| !s.is_empty());
            let name = c["function"]["name"].as_str().filter(|s| !s.is_empty());
            let at = self.slot(c["index"].as_u64(), id, name);
            let call = &mut self.calls[at];
            if let Some(id) = id {
                call.id = id.to_string();
            }
            if let Some(name) = name {
                call.name.push_str(name);
                if call.id.is_empty() {
                    call.id = format!("call_{at}");
                }
                sink(StreamEvent::ToolUseStart {
                    id: call.id.clone(),
                    name: call.name.clone(),
                });
            }
            if let Some(args) = c["function"]["arguments"].as_str() {
                call.arguments.push_str(args);
                sink(StreamEvent::ToolInputDelta {
                    id: call.id.clone(),
                    json_fragment: args.to_string(),
                });
            }
        }
        if let Some(r) = choice["finish_reason"].as_str() {
            self.finish_reason = Some(r.to_string());
        }
        Ok(false)
    }

    /// Where a streamed call fragment goes in `calls`. OpenAI gives each call its own `index` and
    /// sends its id and name once, in its first fragment. Gemini sends parallel calls whole, all
    /// at index 0 or with none, so another id, or a name for a call that has one, starts a call.
    fn slot(&mut self, index: Option<u64>, id: Option<&str>, name: Option<&str>) -> usize {
        let current = match index {
            Some(i) => self.by_index.get(&i).copied(),
            None => self.calls.len().checked_sub(1),
        };
        let at = match current {
            Some(at) => {
                let call = &self.calls[at];
                let other_id = id.is_some_and(|id| !call.id.is_empty() && call.id != id);
                let second_name = id.is_none() && name.is_some() && !call.name.is_empty();
                if other_id || second_name {
                    None
                } else {
                    Some(at)
                }
            }
            None => None,
        };
        at.unwrap_or_else(|| {
            self.calls.push(Call::default());
            let at = self.calls.len() - 1;
            if let Some(i) = index {
                self.by_index.insert(i, at);
            }
            at
        })
    }

    fn finish(self, sink: Sink<'_>) -> TurnResult {
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(ContentBlock::Text {
                text: self.text.clone(),
            });
        }
        let has_calls = !self.calls.is_empty();
        for (at, call) in self.calls.into_iter().enumerate() {
            let id = if call.id.is_empty() {
                format!("call_{at}")
            } else {
                call.id
            };
            let parsed = if call.arguments.trim().is_empty() {
                Ok(json!({}))
            } else {
                serde_json::from_str::<Value>(&call.arguments)
            };
            let input = match parsed {
                Ok(v) if v.is_object() => {
                    sink(StreamEvent::ToolUse {
                        id: id.clone(),
                        name: call.name.clone(),
                        input: v.clone(),
                    });
                    v
                }
                _ => Value::String(call.arguments),
            };
            content.push(ContentBlock::ToolUse {
                id,
                name: call.name,
                input,
            });
        }
        sink(StreamEvent::Done);
        let stop_reason = match self.finish_reason.as_deref() {
            // Some servers say "stop" even when they called tools.
            _ if has_calls => StopReason::ToolUse,
            Some("stop") | None => StopReason::EndTurn,
            Some("length") => StopReason::MaxTokens,
            Some("content_filter") => StopReason::Refusal {
                category: Some("content_filter".into()),
                text: self.text,
            },
            Some(other) => StopReason::Other(other.to_string()),
        };
        TurnResult {
            content,
            stop_reason,
            usage: self.usage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The calls a stream of `data:` payloads ends with, as (id, name, input).
    fn calls_of(events: &[Value]) -> Vec<(String, String, Value)> {
        let mut turn = Turn::default();
        let mut sink = |_: StreamEvent| {};
        for e in events {
            turn.on_event(&e.to_string(), &mut sink).unwrap();
        }
        turn.finish(&mut sink)
            .content
            .into_iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => Some((id, name, input)),
                _ => None,
            })
            .collect()
    }

    fn call(id: Option<&str>, index: Option<u64>, name: &str, args: &str) -> Value {
        let mut c = json!({"type":"function","function":{"name":name,"arguments":args}});
        if let Some(id) = id {
            c["id"] = json!(id);
        }
        if let Some(i) = index {
            c["index"] = json!(i);
        }
        json!({"choices":[{"index":0,"delta":{"tool_calls":[c]}}]})
    }

    #[test]
    fn parallel_calls_without_their_own_index_stay_apart() {
        // Gemini (also behind Runware) streams each parallel call whole, all at index 0 or
        // with no index at all.
        for index in [Some(0), None] {
            let calls = calls_of(&[
                call(Some("f1"), index, "block_summary", "{}"),
                call(Some("f2"), index, "list_layers", "{}"),
                call(Some("f3"), index, "read_text", r#"{"layer":"A-TEXT"}"#),
            ]);
            let names: Vec<&str> = calls.iter().map(|c| c.1.as_str()).collect();
            assert_eq!(
                names,
                ["block_summary", "list_layers", "read_text"],
                "{index:?}"
            );
            assert_eq!(calls[1].0, "f2");
            assert_eq!(calls[2].2, json!({"layer": "A-TEXT"}));
        }
    }

    #[test]
    fn parallel_calls_with_no_id_are_told_apart_by_their_name() {
        let calls = calls_of(&[
            call(None, None, "list_layers", "{}"),
            call(None, None, "list_takeoff", "{}"),
        ]);
        let names: Vec<&str> = calls.iter().map(|c| c.1.as_str()).collect();
        assert_eq!(names, ["list_layers", "list_takeoff"]);
        assert_ne!(calls[0].0, calls[1].0, "each call gets its own id");
    }

    #[test]
    fn input_that_was_not_json_goes_back_as_an_empty_object() {
        let mut out = Vec::new();
        push_messages(
            &mut out,
            &Message::assistant(vec![ContentBlock::ToolUse {
                id: "c1".into(),
                name: "list_layers".into(),
                input: Value::String("{}{}".into()),
            }]),
        );
        assert_eq!(out[0]["tool_calls"][0]["function"]["arguments"], "{}");
    }

    #[test]
    fn tool_results_become_tool_messages_and_images_follow_as_user() {
        let mut out = Vec::new();
        push_messages(
            &mut out,
            &Message::assistant(vec![ContentBlock::ToolUse {
                id: "c1".into(),
                name: "view_image".into(),
                input: json!({}),
            }]),
        );
        push_messages(
            &mut out,
            &Message::user(vec![ContentBlock::ToolResult {
                tool_use_id: "c1".into(),
                content: vec![
                    ResultPart::Text("cap".into()),
                    ResultPart::Png("QUJD".into()),
                ],
                is_error: false,
            }]),
        );
        assert_eq!(out[0]["content"], Value::Null);
        assert_eq!(out[0]["tool_calls"][0]["function"]["arguments"], "{}");
        assert_eq!(
            out[1],
            json!({"role":"tool","tool_call_id":"c1","content":"cap"})
        );
        assert_eq!(out[2]["role"], "user");
        assert_eq!(
            out[2]["content"][0]["image_url"]["url"],
            "data:image/png;base64,QUJD"
        );
    }
}
