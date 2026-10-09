# Qiyas Phase 2: AI and BOQ Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An AI staff member inside Qiyas that measures, counts, reads schedules, imports and links the BOQ, and reports every conflict and gap; plus an MCP server so Claude Desktop and Claude Code can drive Qiyas.

**Architecture:** Every AI action is a tool that wraps the existing command bus or a read-only query on the `Session`. `qiyas-ai` holds the tool catalogue, the Claude client (raw HTTPS, streaming, manual tool loop) and an OpenAI-compatible client, and runs the agent loop on a tokio task that locks the session only while a tool executes. `qiyas-takeoff` holds the BOQ, schedule and conflict models and the comparison engine. The MCP server is `rmcp` over Streamable HTTP on localhost with a token, plus a tiny stdio bridge binary that Claude Desktop spawns.

**Tech Stack:** Rust (reqwest 0.13 with `stream` and `rustls`, tokio 1, futures, serde, rmcp 3.5, keyring 4, tiny-skia for server-side view images), calamine, rust_xlsxwriter; React + TypeScript (Tauri events for streaming).

**Spec:** `docs/superpowers/specs/2026-10-08-qiyas-design.md` (sections "What the app is", "Conflicts and gaps", "AI"). Foundation plan: `docs/superpowers/plans/2026-10-08-qiyas-foundation.md`. Carried items: `docs/2026-10-08-qiyas-foundation-deferred.md`.

## Global Constraints

- Everything in the foundation plan's Global Constraints still binds (desktop only, no CI, Qiyas / قياس, Thmanyah for Arabic, permissive licences only, `scripts/check.sh` before every commit, never push without the founder's yes).
- Default model `claude-opus-5-5`; model id strings are used exactly as written, never with date suffixes. Thinking is adaptive (omit the `thinking` parameter); depth is `output_config.effort` with default `high`. Streaming always on. Server-side fallbacks on by default: header `anthropic-beta: server-side-fallback-2026-07-01` and body `"fallbacks": "default"`. Never use `tool_choice` `any`/`tool`; tools carry `strict: true` with `additionalProperties: false`. Handle `stop_reason: "refusal"` by showing the refusal text and `stop_details.category`. `max_tokens` 16000.
- The API key lives in the OS keychain (`keyring` service `qiyas`, account `<provider>`), never in a file, never in logs, never sent to the webview after it is stored. Settings (provider, model, base URL, effort, MCP port and token) live in `<app config dir>/settings.json`.
- Tool results sent to the model never exceed 40 KB; longer lists are truncated with a `truncated: true` flag and a `next_offset`.
- Every measurement the AI adds has `source: "ai"`, is a normal undoable command, and is highlighted in the viewport while its tool card is selected.
- The assistant never writes a quantity without geometry: `add_measurement` requires `handles` or `points`.
- Arabic UI strings for every new label, natural wording, identical key sets.
- BOQ tolerance default 5 percent; stored per item when the user changes it.

## Review Focus

1. The API key: a test must prove it is never serialised into `settings.json`, the project file, or any log line (grep the test's temp dirs). Task 3.
2. A tool loop that never ends (the model keeps calling tools): the agent must stop after 40 tool calls per turn with a visible "stopped after 40 steps" message, and the Stop button must cancel mid-stream within one second. Task 5.
3. A BOQ in Arabic with merged header cells and a unit column that says "م2": import must map it, and a sheet with no recognisable header must fail with a clear message, not import garbage. Task 7.
4. A conflicts report on a BOQ where one item is linked to three measurements across two sheets: the quantity mismatch must compare the sum, and "not in drawings" must not fire for linked items. Task 9.
5. Two clients at once (the panel and an MCP client) running commands: the session mutex must serialise them and neither may see a half-applied take-off. Task 11.

---

### Task 1: Take-off model grows: volume, items, source, AI-added measurements

**Files:**
- Modify: `crates/qiyas-core/src/takeoff.rs`, `crates/qiyas-core/src/commands.rs`, `crates/qiyas-core/src/session.rs`, `crates/qiyas-core/src/project.rs`, `src/api.ts`
- Test: `crates/qiyas-core/tests/measure.rs` (extend), `crates/qiyas-core/tests/project.rs` (extend)

**Interfaces:**
- Produces: `Kind` gains `Volume` (unit "m3", `shown = value × f³`, where `value` is area × height in drawing units³) and `Item` (unit as given, `shown = value`). `Measurement` gains `height: Option<f64>` (drawing units) and `boq_code: Option<String>`.
- `Command` gains `AddMeasurement { kind: Kind, value: Option<f64>, unit: Option<String>, points: Vec<[f64;2]>, handles: Vec<u64>, description: String, note: String }` (for `Length`/`Area`/`Count` the value is computed from geometry and `value` must be `None`; for `Volume` a `height` is required: add field `height: Option<f64>`; for `Item` `value` and `unit` are required), `SetHeight { id, height }` (turns an Area into a Volume measurement, keeping the area in `points`/`handles`; undoable), `LinkBoq { id, code: Option<String> }`, `SetNote { id, text }`.
- `Session::run_as(&mut self, cmd: Command, source: Source) -> Result<Outcome, CoreError>` where `pub enum Source { User, Ai, Mcp }`; `run` = `run_as(cmd, Source::User)`. Measurements record `source` as "user" | "ai" | "mcp".
- `Project` version 2: `#[serde(default)]` on every new field; a version-1 file loads.

- [ ] Write failing tests: `volume_from_area_and_height` (area 12 m², height 0.2 → Volume 2.4 m3, shown 2.4), `item_measurement_keeps_its_unit` (Item 5 "bags" → shown 5, unit "bags"), `add_measurement_requires_geometry` (no points, no handles → `InvalidCommand`), `run_as_ai_marks_source`, `link_boq_round_trips_in_project` (v2 file reloads; a hand-written v1 JSON still loads).
- [ ] Run: `cargo test -p qiyas-core` → compile errors.
- [ ] Implement; mirror the TS types in `src/api.ts` (`Kind` adds "volume" | "item"; `Command` adds the four variants).
- [ ] Run: `scripts/check.sh` → exit 0. Commit.

---

### Task 2: Read-only queries on the session (the AI's eyes)

**Files:**
- Create: `crates/qiyas-core/src/query.rs`, `crates/qiyas-core/src/tables.rs`
- Modify: `crates/qiyas-core/src/session.rs`, `crates/qiyas-core/src/lib.rs`
- Test: `crates/qiyas-core/tests/query.rs`

**Interfaces:**
- `pub struct EntityFilter { pub sheet: Option<String>, pub layer: Option<String>, pub kind: Option<String>, pub block_name: Option<String>, pub text_contains: Option<String>, pub within: Option<[[f64;2];2]>, pub offset: usize, pub limit: usize /* max 500 */ }`
- `#[derive(Serialize)] pub struct EntityRow { pub handle: u64, pub kind: String, pub layer: String, pub block_name: Option<String>, pub text: Option<String>, pub length: Option<f64>, pub area: Option<f64>, pub bounds: [[f64;2];2] }`
- `pub struct Page<T> { pub rows: Vec<T>, pub total: usize, pub next_offset: Option<usize> }`
- `Session::find_entities(&self, f: &EntityFilter) -> Page<EntityRow>`; `Session::read_text(&self, sheet, within) -> Vec<TextRow { handle, text, at: [f64;2], height }>` (Text, MText with formatting codes stripped via `opencadcodec::entities::mtext_format::parse_mtext`, and Insert attributes); `Session::read_dimensions(&self, sheet) -> Vec<DimRow { handle, kind, measurement: f64, text_override: Option<String>, at }>` (from `EntityType::Dimension`; the measured value from the dimension's definition points, computed for linear/aligned/radius/diameter/angular; others `None`).
- `tables.rs`: `pub struct Schedule { pub name: String, pub sheet: String, pub source: u64 /* table handle or 0 */, pub columns: Vec<String>, pub rows: Vec<Vec<String>> }`; `Session::read_tables(&self, sheet) -> Vec<Schedule>`: real `EntityType::Table` entities (cells by row/column; name = the first merged row's text or "Table at x,y") and text grids: cluster `Text`/`MText` whose insertion points share rows (|Δy| < 0.6 × height) and columns (x bands), at least 3 rows × 2 columns, named by the nearest text above the grid.
- Block summary: `Session::block_summary(&self, sheet) -> Vec<BlockCount { block_name, layer, count }>`.

- [ ] Write failing tests on `fixtures::sample_doc()` plus a new fixture `fixtures::schedule_doc()` (a 4×3 text grid "DOOR SCHEDULE" with header MARK/WIDTH/QTY and 3 rows, one real `Table` entity 2×2, a linear Dimension of 4.0): `find_entities_filters_and_pages` (limit 2 → total 7, next_offset 2), `read_text_strips_mtext_codes`, `read_tables_finds_the_text_grid_and_the_table_entity` (two schedules; the grid has 3 columns and 3 data rows), `read_dimensions_reports_linear_value` (4.0), `block_summary_counts_per_block`.
- [ ] Implement. Run `scripts/check.sh`. Commit.

---

### Task 3: Settings and the keychain

**Files:**
- Create: `crates/qiyas-desktop/src/settings.rs`, `crates/qiyas-desktop/src/secrets.rs`, `src/shell/SettingsDialog.tsx`
- Modify: `crates/qiyas-desktop/src/commands.rs`, `lib.rs`, `src/api.ts`, `src/App.tsx`, `src/i18n/*.json`
- Test: `crates/qiyas-desktop/tests/settings.rs` (settings file round trip in a temp dir; secrets tested with the `keyring` mock credential store feature)

**Interfaces:**
- `#[derive(Serialize, Deserialize, Clone)] pub struct Settings { pub provider: Provider /* "anthropic" | "openai_compatible" */, pub model: String /* default "claude-opus-5-5" */, pub base_url: Option<String>, pub effort: String /* "low"|"medium"|"high"|"xhigh"|"max", default "high" */, pub mcp_enabled: bool, pub mcp_port: u16 /* default 48123 */, pub mcp_token: String /* random 32 hex, generated once */, pub language, pub theme }` with `#[serde(default)]` everywhere; `load(dir) -> Settings`, `save(dir, &Settings)`.
- `secrets.rs`: `set_api_key(provider: &str, key: &str)`, `get_api_key(provider) -> Option<String>`, `delete_api_key(provider)`; service name `qiyas`.
- IPC: `get_settings() -> Settings`, `set_settings(s)`, `set_api_key(provider, key)`, `has_api_key(provider) -> bool`, `delete_api_key(provider)`. The key is write-only from the UI.
- `SettingsDialog`: provider select, model text with the four Claude ids as suggestions, base URL (OpenAI-compatible only), effort select, API key field (password, "Saved" badge when present, Remove), MCP section (enable, port, token with copy button, the exact `claude mcp add` and Claude Desktop JSON snippet for the bridge binary). Opened from the prefs area (gear button) and from the assistant panel.

- [ ] Tests: settings round trip; defaults when the file is missing or corrupt; a corrupt file is backed up to `settings.json.bak`; `api_key_never_written_to_disk` (set a key, save settings, grep the temp dir recursively for the key string → none).
- [ ] Implement. Commit.

---

### Task 4: Tool catalogue (`qiyas-ai`)

**Files:**
- Create: `crates/qiyas-ai/{Cargo.toml, src/lib.rs, src/tools.rs, src/execute.rs, src/image.rs}`
- Test: `crates/qiyas-ai/tests/tools.rs`

**Interfaces:**
- `pub struct ToolSpec { pub name: &'static str, pub description: &'static str, pub input_schema: serde_json::Value /* strict, additionalProperties false */, pub mutating: bool }`; `pub fn catalogue() -> Vec<ToolSpec>` with exactly: `list_sheets`, `set_sheet`, `list_layers`, `show_layers {names, visible}`, `find_entities {filter…}`, `get_entity {handle}`, `read_text {sheet?, within?}`, `read_tables {sheet?}`, `read_dimensions {sheet?}`, `block_summary {sheet?}`, `measure_length {handle | points}`, `measure_area {handle | points}`, `count_blocks {block_name, layer?}`, `add_measurement {…}`, `set_height {id, height}`, `list_takeoff`, `remove_measurement {id}`, `set_description {id, text}`, `import_boq {path}`, `list_boq`, `link_boq {id, code}`, `compare_boq {tolerance?}`, `list_conflicts`, `resolve_conflict {id, status, note}`, `add_markup {points, text, kind: "cloud"|"note"}`, `zoom_to {handles | bounds}`, `view_image {bounds?, width?}`.
- `pub enum ToolOutput { Json(serde_json::Value), Image { png: Vec<u8>, caption: String } }`
- `pub fn execute(session: &mut Session, settings_units: Units, name: &str, input: &serde_json::Value, source: Source) -> Result<ToolOutput, ToolError>`: dispatches to `Session` queries and `run_as`; JSON outputs are capped at 40 KB with `truncated`/`next_offset`. `zoom_to` and `add_markup` return a `ui` field the frontend acts on (zoom; markups are measurements of kind `Item` with value 0 on layer `QIYAS-MARKUP`, drawn as the overlay polyline with the text as description).
- `image.rs`: `render_png(batch: &Batch, bounds: [[f64;2];2], width: u32 /* max 1600 */) -> Vec<u8>` with `tiny-skia`: white background, lines in their colours (white→black), 1 px, plus the take-off overlay in violet. Used by `view_image` so the model can look at the drawing.

- [ ] Tests: `catalogue_schemas_are_strict_objects` (every schema has `"additionalProperties": false` and a `required` array), `execute_measure_area_by_handle_marks_ai_source`, `execute_find_entities_truncates_at_40kb` (a doc with 2000 lines → `truncated: true`), `view_image_is_a_png_of_the_right_size` (PNG signature, width as requested), `unknown_tool_is_an_error`.
- [ ] Implement. Commit.

---

### Task 5: Claude client and the agent loop

**Files:**
- Create: `crates/qiyas-ai/src/anthropic.rs`, `crates/qiyas-ai/src/openai.rs`, `crates/qiyas-ai/src/agent.rs`, `crates/qiyas-ai/src/prompt.rs`
- Test: `crates/qiyas-ai/tests/agent.rs` (against a local mock HTTP server using `wiremock` or a tiny `axum` handler serving canned SSE; no real API calls in tests)

**Interfaces:**
- `pub trait Provider: Send + Sync { async fn stream_turn(&self, req: TurnRequest, sink: &mut dyn FnMut(StreamEvent)) -> Result<TurnResult, AiError>; }` with `TurnRequest { system: String, messages: Vec<Message>, tools: Vec<ToolSpec>, model: String, effort: String, max_tokens: u32 }`, `StreamEvent::{TextDelta(String), ToolUseStart{id,name}, ToolInputDelta{id, json_fragment}, ToolUse{id,name,input}, Thinking(String), Done}`, `TurnResult { content: Vec<ContentBlock>, stop_reason: StopReason /* EndTurn | ToolUse | MaxTokens | Refusal{category, text} */ , usage }`.
- `anthropic.rs`: POST `https://api.anthropic.com/v1/messages` with headers `x-api-key`, `anthropic-version: 2023-06-01`, `anthropic-beta: server-side-fallback-2026-07-01`; body `model`, `max_tokens`, `stream: true`, `system`, `messages`, `tools` (`strict: true`), `output_config: {effort}`, `fallbacks: "default"`; no `thinking` field; parse SSE (`message_start`, `content_block_start/delta/stop`, `message_delta`, `message_stop`); tool inputs parsed with `serde_json::from_str` after the block stops (never string-matched); images as `{"type":"image","source":{"type":"base64","media_type":"image/png","data":…}}` inside `tool_result.content`. Errors: 401 → `AiError::Auth`, 429/529 → `AiError::RateLimited{retry_after}` (one retry after the header's delay, then surface), other → `AiError::Api{status, message}`.
- `openai.rs`: `POST {base_url}/chat/completions` with `tools` (function calling), `stream: true`, SSE `choices[].delta`; same `Provider` trait; images via `image_url` data URLs. Model ids as given by the user.
- `prompt.rs`: `pub fn system_prompt(lang, units, project_summary: &ProjectSummary) -> String`: who it is (Qiyas's AI staff for a quantity surveyor), the rules (never state a quantity without a measurement; prefer measuring entities over points; name sheets and layers you used; answer in the user's language), the drawing summary (sheets, layers with counts, blocks with counts, units), the take-off summary and BOQ status. English and Arabic variants of the fixed text.
- `agent.rs`: `pub struct Agent { provider: Box<dyn Provider>, session: Arc<Mutex<Option<Session>>>, settings, history: Vec<Message> }`; `pub async fn run_turn(&mut self, user_text: String, events: tokio::sync::mpsc::Sender<AgentEvent>, cancel: CancellationToken) -> Result<(), AiError>`: loop: stream a turn, forward deltas as `AgentEvent::Text`, on `ToolUse` lock the session, `execute`, send `AgentEvent::ToolCall{id,name,input}` then `AgentEvent::ToolResult{id, summary, handles, ui}`, append `tool_result` blocks, continue; stop at `EndTurn`; hard cap 40 tool calls per turn → `AgentEvent::Stopped{reason}`; `cancel` aborts the HTTP stream and sends `Stopped{cancelled}`. Every assistant content block (including thinking blocks) is appended back to history unchanged. History trimmed to the last 60 messages plus the first user message.

- [ ] Tests with canned SSE: `text_stream_is_forwarded_in_order`, `tool_use_round_trip_executes_against_the_session` (mock returns a `measure_area` call then end_turn; the session gains one AI measurement), `refusal_stop_reason_surfaces_category`, `loop_stops_after_40_tool_calls` (mock always returns a tool call), `cancel_stops_within_one_second`, `api_key_absent_from_logs_and_errors` (error strings never contain the key).
- [ ] Implement. Commit.

---

### Task 6: Assistant panel (frontend) and the AI IPC

**Files:**
- Create: `src/ai/AssistantPanel.tsx`, `src/ai/useAssistant.ts`, `src/ai/ToolCard.tsx`, `src/ai/messages.ts` (pure reducer over `AgentEvent`s, tested)
- Modify: `crates/qiyas-desktop/src/commands.rs` (`ai_send(text)`, `ai_cancel()`, `ai_clear()`, `ai_status() -> {provider, model, has_key, busy}`), `lib.rs` (the agent lives in `AppState` as `Arc<Mutex<Option<Agent>>>`; events emitted with `app.emit("ai:event", event)`), `src/api.ts`, `src/App.tsx`, `src/shell/theme.css`, `src/i18n/*.json`
- Test: `src/ai/messages.test.ts`

**Interfaces:**
- Panel on the right, tabbed with the take-off panel (tabs: الحصر / المساعد). Message list: user, assistant text (streamed, Markdown-lite: paragraphs, bullets, bold), tool cards (name, one-line summary, status spinner → done/error, "Show" button → highlight handles and zoom, "Undo" for mutating calls), refusal card, stopped card. Composer: textarea (Enter sends, Shift+Enter newline), Send, Stop, Clear, model badge, settings gear. Empty state with three example prompts in the current language ("Measure every wall on A-WALL and add it as length", "Count the doors on this sheet", "Read the door schedule and compare it to the plan").
- `messages.ts`: `reduce(state, event: AgentEvent)` builds the transcript; tested for ordering and for tool-card state transitions.
- After every `ToolResult` with `mutating`, the panel triggers `refreshTakeoff()` and a viewport redraw (new QIYAS overlay).

- [ ] Tests for the reducer (6 cases). Implement. Manual run with the real API using the founder's key only if he has stored one; otherwise verify with the mock provider through a `QIYAS_AI_MOCK=1` env var that makes `ai_send` use a canned provider (documented in the report, removed from release builds via `cfg(debug_assertions)`).
- [ ] Commit.

---

### Task 7: BOQ import (`qiyas-boq`)

**Files:**
- Create: `crates/qiyas-boq/src/import.rs`, `crates/qiyas-takeoff/{Cargo.toml, src/lib.rs, src/boq.rs}`
- Modify: `crates/qiyas-core` (Session gains `boq: Option<Boq>`, `Command::ImportBoq {path}`, `Command::SetBoqTolerance {code, percent}`; Project stores the BOQ), `src/api.ts`
- Test: `crates/qiyas-boq/tests/import.rs` with three fixtures written by the test with `rust_xlsxwriter`: an English BOQ (Code, Description, Unit, Qty, Rate), an Arabic BOQ with merged title rows (بند، الوصف، الوحدة، الكمية، سعر الوحدة) and units "م2"/"م3"/"عدد"/"م.ط", and a sheet with no header.

**Interfaces:**
- `qiyas_takeoff::boq::{Boq { source: String, items: Vec<BoqItem> }, BoqItem { code: String, description: String, unit: String /* normalised: m, m2, m3, pcs, kg, t, lm, or the raw text */, quantity: f64, rate: Option<f64>, section: String, tolerance: Option<f64>, row: u32 }}`.
- `qiyas_boq::import_boq(path) -> Result<Boq, BoqError>`: finds the header row by scoring cells against English and Arabic synonyms (code: code/item/ref/بند/رقم; description: description/desc/الوصف/البيان; unit: unit/الوحدة; qty: qty/quantity/الكمية; rate: rate/price/السعر/سعر الوحدة); requires at least description + unit + qty, else `BoqError::NoHeader(sheet names tried)`; rows with an empty description and quantity are section headers → `section`; unit normalisation table (m², sqm, م2, متر مربع → m2; m³, cum, م3 → m3; no, nr, pcs, عدد → pcs; lm, m, م.ط, متر طولي → m; kg/كجم → kg; ton/طن → t).

- [ ] Tests: `english_boq_imports_items_and_sections`, `arabic_boq_with_merged_title_rows_imports`, `units_are_normalised` (م2 → m2, عدد → pcs), `no_header_is_a_clear_error`, `import_boq_command_stores_in_project` (core).
- [ ] Implement. Commit.

---

### Task 8: BOQ panel and linking (frontend)

**Files:**
- Create: `src/boq/BoqPanel.tsx`, `src/boq/LinkPicker.tsx`
- Modify: `src/App.tsx`, `src/shell/Ribbon.tsx` (Home: Import BOQ), `src/shell/TakeoffPanel.tsx` (BOQ code column and a link button per row), `src/api.ts`, `src/i18n/*.json`
- Test: `src/boq/BoqPanel.test.tsx`

**Interfaces:**
- BOQ panel (third tab): rows with code, description, unit, BOQ qty, measured qty (sum of linked measurements converted to the BOQ unit when units agree, otherwise shown with a warning), difference percent coloured by the tolerance, link count; search box; click a row → highlight linked measurements' geometry.
- `LinkPicker`: from a take-off row, a searchable list of BOQ items → `LinkBoq`.

- [ ] Tests: panel renders rows and the difference colour; search filters. Implement. Commit.

---

### Task 9: Comparison engine and conflicts

**Files:**
- Create: `crates/qiyas-takeoff/src/compare.rs`, `crates/qiyas-takeoff/src/conflict.rs`, `crates/qiyas-boq/src/report.rs`
- Modify: `crates/qiyas-core` (`Session::compare(&mut self, tolerance_default) -> Vec<Conflict>` stores conflicts; `Command::ResolveConflict {id, status: Open|Resolved|Ignored, note}`; Project stores conflicts and schedules), `src/api.ts`
- Test: `crates/qiyas-takeoff/tests/compare.rs`

**Interfaces:**
- `Conflict { id, kind: ConflictKind, boq_code: Option<String>, measurements: Vec<String>, schedule: Option<String>, sheets: Vec<String>, expected: Option<f64>, found: Option<f64>, unit: String, reason: String, status, note }`; `ConflictKind::{QuantityMismatch, MissingFromBoq, NotInDrawings, ScheduleVsPlan, DrawingVsDrawing, UnitDoubt}`.
- `compare(boq: &Boq, takeoff: &Takeoff, schedules: &[Schedule], block_summary_per_sheet: &[(String, Vec<BlockCount>)], default_tol: f64) -> Vec<Conflict>`:
  - QuantityMismatch: for each BOQ item with linked measurements, sum their `shown` (unit-converted; m vs m2 mismatch → UnitDoubt instead), compare with `quantity`; |diff| > tol → conflict with expected/found.
  - MissingFromBoq: measurements with no `boq_code` whose description does not match any BOQ description (case-insensitive token overlap ≥ 0.5) → one conflict per description group.
  - NotInDrawings: BOQ items with no link and no measurement description match.
  - ScheduleVsPlan: a schedule whose columns include a count/qty column and a mark/type column: for each row whose mark matches a block name (case-insensitive, also attribute values), compare the schedule count with the plan count.
  - DrawingVsDrawing: the same block name counted on two sheets with different counts where the sheet names share a floor token (e.g. "GF", "01") → conflict.
  - UnitDoubt: measurement unit incompatible with BOQ unit.
- `report.rs`: `export_conflicts(path, &[Conflict]) -> xlsx` with one row per conflict and a "Summary" sheet with counts per kind.

- [ ] Tests (fixtures built in code): `quantity_mismatch_uses_the_sum_of_links` (three links across two sheets, sum within tolerance → no conflict; outside → one), `linked_items_never_report_not_in_drawings`, `missing_from_boq_groups_by_description`, `schedule_vs_plan_detects_count_difference`, `unit_doubt_for_length_vs_area`, `resolved_conflicts_keep_their_status_after_recompare` (status kept by matching kind + code + measurements).
- [ ] Implement. Commit.

---

### Task 10: Conflicts panel (frontend) and the AI tools for BOQ

**Files:**
- Create: `src/boq/ConflictsPanel.tsx`
- Modify: `src/App.tsx`, `src/shell/Ribbon.tsx` (Home: Compare, Export conflicts), `src/api.ts`, `src/i18n/*.json`, `crates/qiyas-ai/src/execute.rs` (the `import_boq`, `list_boq`, `link_boq`, `compare_boq`, `list_conflicts`, `resolve_conflict` tools now wired)
- Test: `src/boq/ConflictsPanel.test.tsx`, `crates/qiyas-ai/tests/tools.rs` (extend)

**Interfaces:** panel grouped by kind with counts, each row: reason, expected vs found, "Show" (zoom to measurements), status select, note; filter by status; Export button.

- [ ] Tests, implement, commit.

---

### Task 11: MCP server and the stdio bridge

**Files:**
- Create: `crates/qiyas-ai/src/mcp.rs`, `crates/qiyas-mcp/{Cargo.toml, src/main.rs}` (the bridge binary, shipped beside the app as a Tauri sidecar/external binary)
- Modify: `crates/qiyas-desktop/src/lib.rs` (start the MCP server on a tokio runtime when `settings.mcp_enabled`; restart on settings change), `tauri.conf.json` (externalBin), `src/shell/SettingsDialog.tsx` (status: listening on port, copy snippets)
- Test: `crates/qiyas-ai/tests/mcp.rs` (start the server on port 0 against a session with the sample drawing; connect with `rmcp`'s client over HTTP; `tools/list` returns the catalogue; `tools/call measure_area` adds a measurement with `source: "mcp"`; a wrong token is rejected with 401; two concurrent `count_blocks` calls both succeed and the take-off has exactly two items)

**Interfaces:**
- `mcp.rs`: `pub async fn serve(session: Arc<Mutex<Option<Session>>>, port: u16, token: String, shutdown: CancellationToken) -> Result<SocketAddr, AiError>` using `rmcp` with the Streamable HTTP transport bound to `127.0.0.1`; `Authorization: Bearer <token>` required; tools from `catalogue()`, executed with `Source::Mcp`; `view_image` returns an MCP image content block.
- `qiyas-mcp` binary: reads `QIYAS_MCP_URL` (default `http://127.0.0.1:48123/mcp`) and `QIYAS_MCP_TOKEN`; bridges stdio JSON-RPC to the HTTP endpoint (rmcp client as the server side of stdio); prints a clear error if Qiyas is not running.
- Settings dialog shows: `claude mcp add qiyas -e QIYAS_MCP_TOKEN=… -- <path>/qiyas-mcp` and the Claude Desktop JSON block.

- [ ] Tests as above. Implement. Commit.

---

### Task 12: Check, docs, hand-over

- [ ] `scripts/check.sh` exit 0. Update `qiyas/CLAUDE.md` (crates, settings file location, keychain, MCP).
- [ ] Manual run: with the founder's key present in the keychain (ask him to paste it into the settings dialog himself; agents never handle it), open `samples/test-plan.dwg`, ask the assistant "Measure every room area and count the doors, then import samples/test-boq.xlsx and compare". Add `samples/test-boq.xlsx` (written by a new example `write_test_boq`) with six items where two are deliberately wrong, so the conflicts panel must show exactly those two. Record the transcript in `docs/2026-10-08-qiyas-phase2-check.md` in the workspace repo.
- [ ] Report to the founder. No push.
