# Qiyas Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A desktop app named Qiyas that opens a DWG or DXF, shows it, lets a QS measure length, area and count into a take-off, exports the take-off to Excel, and saves the drawing with the measurements inside it.

**Architecture:** Rust holds the drawing (an `opencadcodec::CadDocument`) and every command; Tauri exposes a small set of IPC commands; React draws the ribbon, panels and a WebGL2 viewport fed with line segments as binary buffers. One command bus in `qiyas-core` so the AI (next plan) calls the same commands as the UI.

**Tech Stack:** Rust 1.97, Tauri 2.11, opencadcodec (git rev `a35f43e`, MPL-2.0), rust_xlsxwriter, calamine, rstar, serde; React 19, TypeScript, Vite, vitest, pnpm 11.

**Spec:** `docs/superpowers/specs/2026-10-08-qiyas-design.md` (this plan is Phase 1a of it: open, view, layers, measure, take-off, export, save). Phase 1b (PDF and image underlays, xrefs, snaps, named views, properties palette, command line, data extraction, recents) is the next plan.

## Global Constraints

- Desktop only. No web build, no CI workflows. Local check is `scripts/check.sh`.
- Window title and product name: `Qiyas`. Arabic name: `قياس`. Bundle identifier: `com.qsmind.qiyas`.
- Repo `qiyas/` is its own git repo (private, `qs-mind/qiyas`). Commit inside it. Never push without the founder's yes.
- Only MPL-2.0, Apache-2.0, MIT, BSD or OFL dependencies. No code copied from Open CAD Studio (GPL).
- Thmanyah font files are never committed. `scripts/fonts.sh` copies them from `~/Downloads/Thmanyah-Font-Family/thmanyah typeface` into the git-ignored `src/fonts/thmanyah/`. Instrument Sans (OFL) is committed.
- English text says "Qiyas" and "QS Mind"; Arabic text says "قياس" and "كيو إس مايند". Arabic uses Thmanyah Sans; headings use `font-feature-settings: "swsh"`.
- Measured values are stored in drawing units and displayed in metres, square metres and pieces. Unit factor comes from the drawing header (`insertion_units`): 1 = inches, 2 = feet, 4 = mm, 5 = cm, 6 = m, 0 or unknown = treat as mm and show a warning in the status bar.
- Every mutating command is undoable. Undo and redo stacks live in `qiyas-core`.
- Brand colours: Ink `#0E0F12`, Stone `#F2EEE7`, Paper `#F9F8F4`, Slate `#6E6A64`, Qiyas face anodised titanium `#C9A6E0` to `#6E4A9E`.

## Review Focus

1. A DWG the failsafe reader only partly recovers (corrupt sections): the app must open what it could read and show "Recovered with N problems" in the status bar, never a blank window. Test in Task 2.
2. A drawing whose header says unitless (`insertion_units = 0`): measurements must still work, assume mm, and the status bar must say "Units unknown, assuming mm". Test in Task 8.
3. A block inserted inside a block inside a block, scaled and rotated, where a QS clicks the inner line to measure it: the measured length must be the world length after all transforms. Test in Task 3.
4. A polygon drawn by the user that crosses itself: area must be reported as the absolute shoelace value and the measurement flagged `self_intersecting = true` so the panel can warn. Test in Task 8.
5. Saving to DWG when the source was DXF (or the reverse) and reopening in Qiyas: measurements rebuilt from xdata must equal the ones saved (same ids, kinds, values within 1e-6). Test in Task 11.

---

### Task 1: Repo scaffold

**Files:**
- Create: `qiyas/Cargo.toml` (workspace), `qiyas/package.json`, `qiyas/vite.config.ts`, `qiyas/tsconfig.json`, `qiyas/index.html`, `qiyas/src/main.tsx`, `qiyas/src/App.tsx`
- Create: `qiyas/crates/qiyas-desktop/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,src/main.rs,src/lib.rs}`
- Create: `qiyas/crates/qiyas-core/{Cargo.toml,src/lib.rs}` (empty library, filled in Task 2)
- Create: `qiyas/scripts/check.sh`, `qiyas/scripts/fonts.sh`, `qiyas/.gitignore`, `qiyas/CLAUDE.md`, `qiyas/README.md`, `qiyas/LICENSES/opencadcodec-MPL-2.0.txt`
- Modify: `repos.txt` (workspace root), `CLAUDE.md` (workspace root, apps table)

**Interfaces:**
- Produces: workspace layout every later task uses. `scripts/check.sh` runs `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pnpm typecheck`, `pnpm test`, and exits non-zero on the first failure.

- [ ] **Step 1: Create the repo and workspace files**

```bash
mkdir -p /home/kareem/Work/qs-mind/qiyas && cd /home/kareem/Work/qs-mind/qiyas && git init -b main
```

`Cargo.toml` workspace: `members = ["crates/*"]`, `resolver = "2"`, `[workspace.package] version = "0.1.0", edition = "2021", license = "UNLICENSED", rust-version = "1.85"`. Workspace dependencies: `opencadcodec = { git = "https://github.com/HakanSeven12/opencadcodec.git", rev = "a35f43e", features = ["serde"] }`, `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `thiserror = "2"`, `tauri = "2.11"`, `tauri-build = "2.6"`, `tauri-plugin-dialog = "2.7"`, `rstar = "0.12"`, `rust_xlsxwriter = "0.9"`, `calamine = "0.36"`, `tempfile = "3"`.

`package.json`: name `qiyas`, private, `packageManager: "pnpm@11.16.0"`, scripts `dev: "vite"`, `build: "tsc --noEmit && vite build"`, `typecheck: "tsc --noEmit"`, `test: "vitest run"`, `tauri: "tauri"`. Dependencies: `react`, `react-dom`, `@tauri-apps/api`, `@tauri-apps/plugin-dialog`. Dev: `vite`, `@vitejs/plugin-react`, `typescript`, `vitest`, `@types/react`, `@types/react-dom`, `@tauri-apps/cli`.

- [ ] **Step 2: Create the Tauri app crate**

`crates/qiyas-desktop/tauri.conf.json`: `productName: "Qiyas"`, `identifier: "com.qsmind.qiyas"`, `build.devUrl: "http://localhost:1420"`, `build.frontendDist: "../../dist"`, `build.beforeDevCommand: "pnpm dev"`, `build.beforeBuildCommand: "pnpm build"`, one window `title: "Qiyas"`, 1440 by 900, `bundle.active: true`, `bundle.icon` pointing at `icons/` (placeholder icons generated with `pnpm tauri icon` from a plain square PNG until Task 12).

`src/lib.rs` exposes `pub fn run()` that builds the Tauri app with the dialog plugin and an empty `invoke_handler`. `src/main.rs` calls it.

- [ ] **Step 3: Create `scripts/check.sh` and `scripts/fonts.sh`**

`check.sh` runs the five commands from the Interfaces block in order with `set -euo pipefail`. `fonts.sh` copies every `*.ttf` and `*.otf` under `~/Downloads/Thmanyah-Font-Family/thmanyah typeface` into `src/fonts/thmanyah/` and prints the count. `.gitignore`: `target/`, `node_modules/`, `dist/`, `src/fonts/thmanyah/`, `*.qiyas.bak`.

- [ ] **Step 4: Write `CLAUDE.md` for the repo**

Content: what Qiyas is (one paragraph), the layout from the spec's Architecture block, the Global Constraints above, "run `scripts/check.sh` before every commit", "never push or release without the founder's yes", and a pointer to the spec and this plan in the workspace `docs/`.

- [ ] **Step 5: Verify the scaffold runs**

Run: `pnpm install && scripts/check.sh`
Expected: exits 0 (empty crates, no tests yet is fine; vitest must be told `passWithNoTests: true` in `vite.config.ts`).

Run: `pnpm tauri dev` from `crates/qiyas-desktop` (or `pnpm tauri dev` at root with `--config crates/qiyas-desktop/tauri.conf.json`), then close it.
Expected: a window titled "Qiyas" opens showing the text "Qiyas".

- [ ] **Step 6: Register the app in the workspace**

Append `qiyas https://github.com/qs-mind/qiyas.git` to `repos.txt`. Add a row to the apps table in the workspace `CLAUDE.md`: `qiyas/` | Desktop measurement CAD: open DWG and DXF, measure and edit, AI builds and checks BOQs | Private | Tauri, React, Rust.

- [ ] **Step 7: Commit (two repos)**

```bash
cd /home/kareem/Work/qs-mind/qiyas && git add -A && git commit -m "chore: scaffold Qiyas (Tauri, React, Rust workspace)"
cd /home/kareem/Work/qs-mind && git add repos.txt CLAUDE.md && git commit -m "chore: add Qiyas to the workspace"
```

---

### Task 2: Open a drawing (`qiyas-core::Drawing`)

**Files:**
- Create: `crates/qiyas-core/src/drawing.rs`, `crates/qiyas-core/src/units.rs`, `crates/qiyas-core/src/error.rs`
- Create: `crates/qiyas-core/src/fixtures.rs` (behind `#[cfg(any(test, feature = "fixtures"))]`, feature `fixtures` declared in Cargo.toml so later crates' tests can use it)
- Test: `crates/qiyas-core/tests/open.rs`

**Interfaces:**
- Produces:
  - `pub struct Drawing { pub path: PathBuf, pub doc: CadDocument, pub problems: Vec<String>, pub units: Units }`
  - `pub fn Drawing::open(path: &Path) -> Result<Drawing, CoreError>`; DXF by extension (case-insensitive) via `DxfReader::from_file(path)?.read_with_stats()`, otherwise `DwgReader::from_file_with_options(path, DwgReadOptions::failsafe())?.read_with_stats()`; `problems` are the `ReadStats` diagnostics messages.
  - `pub struct Sheet { pub name: String, pub block: String, pub is_model: bool }`; `pub fn Drawing::sheets(&self) -> Vec<Sheet>`: Model first, then paper layouts by `tab_order` (same rule as Quantix's `spaces_of`).
  - `pub struct LayerInfo { pub name: String, pub color: [u8; 3], pub off: bool, pub frozen: bool, pub locked: bool }`; `pub fn Drawing::layers(&self) -> Vec<LayerInfo>` sorted by name.
  - `pub fn Drawing::entities_in(&self, sheet_block: &str) -> impl Iterator<Item = &EntityType>` (uses `doc.entities_in_block`).
  - `pub enum Units { Inches, Feet, Millimetres, Centimetres, Metres, Unknown }`, `pub fn Units::from_header(insertion_units: i16) -> Units`, `pub fn Units::to_metres(&self) -> f64` (Unknown returns 0.001).
  - `pub enum CoreError { Unreadable(String), Io(std::io::Error), NoSuchSheet(String), NoSuchEntity(u64), ... }` with `thiserror`.
  - `fixtures::sample_doc() -> CadDocument`: layers `A-WALL` (colour index 1) and `A-DOOR` (index 3); on A-WALL a `Line` from (0,0) to (10,0), a closed `LwPolyline` rectangle (0,0) (4,0) (4,3) (0,3), an `Arc` centre (20,0) radius 2 from 0 to π; block `DOOR` holding a `Line` (0,0) to (0.9,0); three `Insert`s of `DOOR` on A-DOOR at (1,1), (5,1) rotated π/2, (9,1) scaled 2; a `Text` "GROUND FLOOR PLAN" at (0,-2) height 0.5; `header.insertion_units = 6`. `fixtures::write_sample(dir: &Path, ext: &str) -> PathBuf` writes it as `sample.dxf` or `sample.dwg`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn opens_sample_dxf_and_dwg() {
    for ext in ["dxf", "dwg"] {
        let dir = tempfile::tempdir().unwrap();
        let path = qiyas_core::fixtures::write_sample(dir.path(), ext);
        let d = Drawing::open(&path).unwrap();
        assert_eq!(d.units, Units::Metres);
        assert_eq!(d.sheets()[0].name, "Model");
        let names: Vec<_> = d.layers().into_iter().map(|l| l.name).collect();
        assert!(names.contains(&"A-WALL".to_string()) && names.contains(&"A-DOOR".to_string()));
        assert_eq!(d.entities_in("*Model_Space").count(), 7);
    }
}
#[test] fn unreadable_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("junk.dwg"); std::fs::write(&p, b"not a drawing").unwrap();
    assert!(matches!(Drawing::open(&p), Err(CoreError::Unreadable(_))));
}
#[test] fn truncated_dwg_opens_with_problems() {
    // write sample.dwg, cut the file to 70 percent of its length, open: Ok with problems non-empty OR Err(Unreadable). Both acceptable; never a panic.
}
#[test] fn units_table() {
    assert_eq!(Units::from_header(4), Units::Millimetres);
    assert_eq!(Units::from_header(0), Units::Unknown);
    assert!((Units::Feet.to_metres() - 0.3048).abs() < 1e-9);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p qiyas-core`
Expected: compile errors, `Drawing` not found.

- [ ] **Step 3: Implement `fixtures.rs`, `units.rs`, `error.rs`, `drawing.rs`**

Insert scale uses `Insert::new(name, point).with_scale(2.0, 2.0, 2.0)`; layers are added with `doc.layers.add(Layer { name, color: Color::Index(1), .. })`; block via `doc.block_records.add(BlockRecord::new("DOOR"))` plus `doc.add_entity_to_layout`-style placement (use whichever `CadDocument` method adds an entity to a named block; check `add_entity`'s owner handling and `entities_in_block` in the pinned revision).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p qiyas-core`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/qiyas-core && git commit -m "feat(core): open DWG and DXF, sheets, layers, units"
```

---

### Task 3: Flatten geometry (`qiyas-geom`)

**Files:**
- Create: `crates/qiyas-geom/{Cargo.toml,src/lib.rs,src/path.rs,src/flatten.rs,src/transform.rs}`
- Test: `crates/qiyas-geom/tests/flatten.rs`

**Interfaces:**
- Consumes: `opencadcodec::EntityType`, `CadDocument` (for block lookup).
- Produces:
  - `pub struct Path { pub points: Vec<[f64; 2]>, pub closed: bool }` with `pub fn length(&self) -> f64`, `pub fn area(&self) -> f64` (absolute shoelace, 0.0 when open), `pub fn self_intersects(&self) -> bool` (segment pair test, O(n²) is fine), `pub fn bounds(&self) -> Option<[[f64; 2]; 2]>`.
  - `pub struct Xform { pub m: [[f64; 3]; 3] }` affine 2D: `identity()`, `translate(dx, dy)`, `rotate(rad)`, `scale(sx, sy)`, `then(&self, other: &Xform) -> Xform`, `apply(&self, p: [f64; 2]) -> [f64; 2]`.
  - `pub fn flatten(doc: &CadDocument, entity: &EntityType, xf: &Xform, tol: f64, out: &mut Vec<Path>)`: Line, LwPolyline (bulge arcs sampled so chord error ≤ `tol`), Polyline, Circle, Arc, Ellipse, Spline (polyline through fit points, else control points), Hatch (each boundary path's edges), Insert (block entities recursively with `translate ∘ rotate ∘ scale`, depth limit 32, row and column counts honoured), Text and MText (a single closed rectangle from insertion point, height and `value.len() * height * 0.6` width, so picking works), Point (nothing), anything else (nothing).
  - `pub fn arc_points(center: [f64; 2], r: f64, a0: f64, a1: f64, tol: f64) -> Vec<[f64; 2]>` counter-clockwise, used by Arc, Circle and bulges.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn rectangle_area_and_perimeter() { /* sample_doc rect -> one closed Path, area 12.0, length 14.0 (1e-9) */ }
#[test] fn circle_area_converges() { /* Circle r=1, tol=1e-4: area within 1e-3 of PI, closed, length within 1e-3 of 2PI */ }
#[test] fn bulge_semicircle() { /* LwPolyline (0,0) bulge 1 -> (2,0) closed: area within 1e-3 of PI/2 */ }
#[test] fn nested_scaled_rotated_insert() {
    // block A: line (0,0)-(1,0). block B: insert A at (0,0) scaled 2. Model: insert B at (5,5) rotated PI/2 scaled 3.
    // flatten the model insert -> one Path of length 6.0 from (5,5) to (5,11) (1e-9)
}
#[test] fn self_intersecting_bowtie() { /* (0,0)(2,2)(2,0)(0,2) closed: self_intersects() true, area 2.0 */ }
#[test] fn open_path_has_zero_area() { /* line: area 0.0 */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p qiyas-geom`
Expected: compile errors.

- [ ] **Step 3: Implement `path.rs`, `transform.rs`, `flatten.rs`**

Bulge to arc: included angle `θ = 4·atan(b)`, radius `r = d·(1+b²)/(4|b|)` for chord `d`; centre on the perpendicular bisector on the side given by the sign of `b`. Insert transform order: scale, then rotate, then translate; block `base_point` subtracted first.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p qiyas-geom`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/qiyas-geom && git commit -m "feat(geom): flatten entities to 2D paths with transforms, length, area"
```

---

### Task 4: Render batches and picking (`qiyas-render`)

**Files:**
- Create: `crates/qiyas-render/{Cargo.toml,src/lib.rs,src/batch.rs,src/color.rs,src/index.rs}`
- Test: `crates/qiyas-render/tests/batch.rs`

**Interfaces:**
- Consumes: `qiyas_core::Drawing`, `qiyas_geom::{flatten, Path, Xform}`.
- Produces:
  - `pub struct Batch { pub segments: Vec<f32>, pub colors: Vec<u32>, pub handles: Vec<u64>, pub ranges: Vec<(u64, u32, u32)>, pub bounds: [[f64; 2]; 2] }`: `segments` is `x0 y0 x1 y1` per segment; `colors` one `0xRRGGBB` per segment; `handles` one per segment; `ranges` is `(handle, first_segment, count)`.
  - `pub struct Visibility { pub hidden_layers: HashSet<String> }`.
  - `pub fn build_batch(drawing: &Drawing, sheet_block: &str, vis: &Visibility, tol: f64) -> Batch`: skips entities whose layer is in `hidden_layers`, is off or frozen, or whose `common.invisible` is set; colour resolved by `color.rs`.
  - `pub fn resolve_color(entity_color: &Color, layer_color: &Color) -> [u8; 3]`: ByLayer uses the layer, ByBlock and None use `[255,255,255]`, Index uses the ACI table (`Color::rgb()` from the codec), true colour passes through. Index 7 maps to `[255,255,255]`; the frontend swaps white and black per theme.
  - `pub fn Batch::to_bytes(&self) -> Vec<u8>`: little-endian; header `u32 segment_count`, `u32 range_count`, `f64 ×4 bounds`, then segments `f32`, colors `u32`, handles `u64`, ranges `(u64,u32,u32)`.
  - `pub struct PickIndex` built by `PickIndex::new(&Batch)` over an `rstar::RTree` of segments; `pub fn pick(&self, p: [f64; 2], tol: f64) -> Option<u64>` returns the handle of the nearest segment within `tol`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn sample_batch_counts_and_bounds() { /* open sample dxf, build at tol 1e-3: ranges.len() == 7 (the Text yields a rectangle, so it counts); bounds min x == 0.0, max x >= 22.0 (the arc reaches 22) */ }
#[test] fn hidden_layer_removes_segments() { /* hide A-DOOR: ranges.len() == 4 */ }
#[test] fn bylayer_color_comes_from_layer() { /* A-WALL index 1 -> [255,0,0] */ }
#[test] fn bytes_round_trip_header() { /* to_bytes: first u32 == segments.len()/4 */ }
#[test] fn pick_finds_the_line() { /* pick([5.0, 0.05], 0.1) == Some(handle of the Line); pick([50,50],0.1) == None */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p qiyas-render`
Expected: compile errors.

- [ ] **Step 3: Implement the crate**

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p qiyas-render`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/qiyas-render && git commit -m "feat(render): line batches, colours, binary encoding, picking"
```

---

### Task 5: Session and Tauri IPC (`qiyas-desktop`)

**Files:**
- Create: `crates/qiyas-core/src/session.rs` (Tauri-free, testable), `crates/qiyas-desktop/src/commands.rs`, `crates/qiyas-desktop/src/state.rs`
- Modify: `crates/qiyas-desktop/src/lib.rs`, `crates/qiyas-desktop/capabilities/default.json` (allow `dialog:allow-open`, `dialog:allow-save`)
- Test: `crates/qiyas-core/tests/session.rs`

**Interfaces:**
- Produces (core):
  - `pub struct Session { pub drawing: Drawing, pub sheet: String, pub visibility: Visibility, pub takeoff: Takeoff /* Task 8 adds this; a placeholder unit struct until then */ }`
  - `pub fn Session::open(path: &Path) -> Result<Session, CoreError>` (sheet = model block)
  - `pub fn Session::info(&self) -> DrawingInfo` where `#[derive(Serialize)] pub struct DrawingInfo { pub path: String, pub sheets: Vec<Sheet>, pub layers: Vec<LayerInfo>, pub units: String, pub problems: Vec<String> }`
  - `pub fn Session::set_sheet(&mut self, block: &str) -> Result<(), CoreError>`
  - `pub fn Session::set_layer_hidden(&mut self, name: &str, hidden: bool)`
  - `pub fn Session::batch(&self) -> Batch` and `pub fn Session::pick(&self, p: [f64; 2], tol: f64) -> Option<u64>` (rebuilds the index lazily; cache keyed by sheet and visibility hash)
  - `#[derive(Serialize)] pub struct EntityInfo { pub handle: u64, pub kind: String, pub layer: String, pub length: Option<f64>, pub area: Option<f64>, pub block_name: Option<String>, pub text: Option<String> }`; `pub fn Session::entity_info(&self, handle: u64) -> Result<EntityInfo, CoreError>` (length and area in drawing units via `qiyas_geom`).
- Produces (Tauri commands, all `async`, state `Mutex<Option<Session>>`): `open_drawing(path: String) -> Result<DrawingInfo, String>`, `drawing_info() -> Result<DrawingInfo, String>`, `set_sheet(block: String)`, `set_layer_hidden(name: String, hidden: bool)`, `render_batch() -> Result<tauri::ipc::Response, String>` (bytes from `Batch::to_bytes`), `pick(x: f64, y: f64, tol: f64) -> Result<Option<u64>, String>`, `entity_info(handle: u64) -> Result<EntityInfo, String>`.

- [ ] **Step 1: Write the failing tests (core session only)**

```rust
#[test] fn session_reports_info_and_switches_sheet() { /* open sample; info().layers.len()==2 (plus "0" if the writer adds it: assert contains A-WALL); set_sheet("*Model_Space") ok; set_sheet("NOPE") is Err(NoSuchSheet) */ }
#[test] fn hiding_a_layer_changes_the_batch() { /* ranges 7 -> 4 */ }
#[test] fn entity_info_for_rectangle() { /* find rect handle via pick([2.0, 0.0], 0.05) is ambiguous with the Line: use pick([4.0, 1.5], 0.05); kind "LwPolyline", area Some(12.0), length Some(14.0) */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p qiyas-core`
Expected: compile errors.

- [ ] **Step 3: Implement `session.rs`, then `commands.rs` and `state.rs`, register in `lib.rs`**

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test -p qiyas-core && cargo clippy -p qiyas-desktop -- -D warnings`
Expected: 3 passed, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates && git commit -m "feat(desktop): session state and IPC commands for open, sheets, layers, render, pick"
```

---

### Task 6: Viewport (WebGL2) and camera

**Files:**
- Create: `src/viewport/camera.ts`, `src/viewport/batch.ts`, `src/viewport/gl.ts`, `src/viewport/Viewport.tsx`, `src/api.ts`
- Test: `src/viewport/camera.test.ts`, `src/viewport/batch.test.ts`

**Interfaces:**
- Consumes: the IPC commands of Task 5 through `src/api.ts` (thin `invoke` wrappers with the same names and types, `renderBatch(): Promise<ArrayBuffer>`).
- Produces:
  - `camera.ts`: `type Camera = { cx: number; cy: number; scale: number /* world units per pixel */; width: number; height: number }`; `worldToScreen(c, p)`, `screenToWorld(c, p)`, `zoomAt(c, screenPoint, factor)`, `pan(c, dxPx, dyPx)`, `fitBounds(c, bounds, marginPx)`, all pure and returning a new Camera.
  - `batch.ts`: `parseBatch(buf: ArrayBuffer): Batch` with `type Batch = { segments: Float32Array; colors: Uint32Array; handles: BigUint64Array; ranges: Array<{ handle: bigint; first: number; count: number }>; bounds: [[number, number],[number, number]] }` matching `Batch::to_bytes`.
  - `gl.ts`: `createRenderer(canvas): { setBatch(b: Batch): void; setHighlight(handles: Set<bigint>): void; draw(camera: Camera, theme: "light" | "dark"): void; dispose(): void }`. Lines drawn as `GL_LINES` with per-vertex colour; white (`0xFFFFFF`) becomes Ink on the light theme. Highlight redraws the chosen ranges in the Qiyas violet `#6E4A9E` at 2 px.
  - `Viewport.tsx`: props `{ batch: Batch | null; highlight: Set<bigint>; onClick(world: [number, number], screen: [number, number]): void; onMove(world: [number, number]): void; tool: Tool }`; wheel zooms at the cursor, middle drag or space plus drag pans, double middle click zooms extents, `Z` then drag draws a zoom window.

- [ ] **Step 1: Write the failing tests**

```ts
test("round trip world/screen", () => { const c = {cx:0,cy:0,scale:0.5,width:800,height:600}; const p:[number,number]=[12.5,-3]; expect(screenToWorld(c, worldToScreen(c,p))).toEqual([12.5,-3]); });
test("zoomAt keeps the point under the cursor", () => { /* zoomAt(c,[100,100],0.5): screenToWorld before == after at [100,100] within 1e-9 */ });
test("fitBounds centres and fits the longer side", () => { /* bounds [[0,0],[40,10]], 800x600, margin 20: cx 20, cy 5, scale 40/(800-40) */ });
test("parseBatch reads the header", () => { /* build a buffer by hand with 1 segment and 1 range; expect segments length 4, ranges[0].count 1 */ });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test`
Expected: FAIL, modules not found.

- [ ] **Step 3: Implement `camera.ts`, `batch.ts`, `gl.ts`, `Viewport.tsx`, `api.ts`**

- [ ] **Step 4: Run tests and typecheck**

Run: `pnpm test && pnpm typecheck`
Expected: 4 passed, no type errors.

- [ ] **Step 5: Wire a temporary open button in `App.tsx`, run the app, open `sample.dxf`**

Run: `pnpm tauri dev`
Expected: the sample drawing appears, red line and rectangle, green door blocks, zoom and pan work, zoom extents fits it.

- [ ] **Step 6: Commit**

```bash
git add src && git commit -m "feat(ui): WebGL2 viewport with camera, batch parsing, highlight"
```

---

### Task 7: App shell: ribbon, sheets, layers, status bar, theme, language

**Files:**
- Create: `src/App.tsx` (rewrite), `src/shell/Ribbon.tsx`, `src/shell/SheetTabs.tsx`, `src/shell/LayersPanel.tsx`, `src/shell/StatusBar.tsx`, `src/shell/theme.css`, `src/i18n/{index.tsx,en.json,ar.json}`, `src/fonts/instrument-sans/*.woff2` (OFL, committed), `src/fonts/fonts.css`
- Test: `src/i18n/i18n.test.ts`, `src/shell/LayersPanel.test.tsx` (vitest with `@testing-library/react`; add it and `jsdom` to dev deps)

**Interfaces:**
- Produces:
  - `i18n`: `useT(): (key: string, vars?: Record<string, string | number>) => string`, `LanguageProvider`, `useLanguage(): { lang: "en" | "ar"; setLang }`; sets `document.documentElement.dir` to `rtl` for Arabic and `lang`. Missing key returns the key. `ar.json` and `en.json` have identical key sets (tested).
  - `Ribbon`: tabs `home`, `measure`, `view`; props `{ active: Tool; onTool(t: Tool): void; onOpen(): void; onSave(): void; onSaveAs(): void; onExport(): void; onZoomExtents(): void; onUndo(): void; onRedo(): void }`. `type Tool = "select" | "distance" | "area" | "count" | "zoomWindow"`.
  - `LayersPanel`: props `{ layers: LayerInfo[]; hidden: Set<string>; onToggle(name: string): void; onIsolate(name: string): void; onShowAll(): void }`; a colour swatch, name, eye toggle; isolate hides every other layer.
  - `StatusBar`: props `{ cursor: [number, number] | null; units: string; unitsWarning: boolean; problems: number; message: string }`. Shows cursor as `x, y` in drawing units with 3 decimals, "Units unknown, assuming mm" when `unitsWarning`, "Recovered with N problems" when `problems > 0`.
  - `theme.css`: CSS variables for Ink, Stone, Paper, Slate, the violet face pair, `[data-theme="dark"]` swaps background and ink. Arabic `body[dir="rtl"]` uses `Thmanyah Sans`; `h1, h2, .heading` get `font-feature-settings: "swsh"`. `fonts.css` declares Instrument Sans and, if the files exist, Thmanyah from `src/fonts/thmanyah/` (missing files fall back to the system Arabic font without error).

- [ ] **Step 1: Write the failing tests**

```ts
test("en and ar have the same keys", () => { expect(Object.keys(ar).sort()).toEqual(Object.keys(en).sort()); });
test("arabic sets rtl", () => { /* render with lang ar; document.documentElement.dir === "rtl" */ });
test("layers panel toggles and isolates", () => { /* click eye on A-DOOR -> onToggle("A-DOOR"); click isolate on A-WALL -> onIsolate("A-WALL") */ });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test`
Expected: FAIL.

- [ ] **Step 3: Implement the shell, with English and Arabic strings for every label used so far** (Open, Save, Save as, Export take-off, Zoom extents, Undo, Redo, Layers, Isolate, Show all, Distance, Area, Count, Select, Model, Home, Measure, View, the two status messages)

- [ ] **Step 4: Run tests and typecheck**

Run: `pnpm test && pnpm typecheck`
Expected: all pass.

- [ ] **Step 5: Run the app in both languages and both themes**

Run: `pnpm tauri dev`
Expected: ribbon, sheet tabs, layers panel and status bar visible; switching to Arabic flips the layout to RTL with Thmanyah; dark theme uses Ink background with light lines.

- [ ] **Step 6: Commit**

```bash
git add src package.json pnpm-lock.yaml && git commit -m "feat(ui): app shell with ribbon, sheets, layers, status bar, themes, English and Arabic"
```

---

### Task 8: Measure commands and the take-off (`qiyas-core`)

**Files:**
- Create: `crates/qiyas-core/src/takeoff.rs`, `crates/qiyas-core/src/commands.rs`, `crates/qiyas-core/src/undo.rs`
- Modify: `crates/qiyas-core/src/session.rs` (real `Takeoff` field, `run`, `undo`, `redo`)
- Test: `crates/qiyas-core/tests/measure.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, PartialEq)] pub enum Kind { Length, Area, Count }`
  - `#[derive(Serialize, Deserialize, Clone)] pub struct Measurement { pub id: String /* uuid v4 */, pub kind: Kind, pub value: f64 /* drawing units, units², or pieces */, pub unit: String /* "m", "m2", "pcs" after conversion */, pub shown: f64 /* value converted */, pub description: String, pub layer: String, pub sheet: String, pub handles: Vec<u64>, pub points: Vec<[f64; 2]>, pub block_name: Option<String>, pub self_intersecting: bool, pub source: String /* "user" | "ai" */, pub note: String, pub created: String /* RFC 3339 */ }`
  - `pub struct Takeoff { pub items: Vec<Measurement> }` with `add`, `remove(id)`, `get(id)`, `totals() -> Vec<Total>` where `#[derive(Serialize)] pub struct Total { pub kind: Kind, pub unit: String, pub count: usize, pub sum: f64 }`.
  - `#[derive(Serialize, Deserialize, Clone)] #[serde(tag = "type", rename_all = "snake_case")] pub enum Command { MeasureDistance { points: Vec<[f64; 2]> }, MeasureArea { points: Vec<[f64; 2]> }, MeasureEntity { handle: u64 }, CountBlock { block_name: String, layer: Option<String> }, RemoveMeasurement { id: String }, SetDescription { id: String, text: String }, SetLayerHidden { name: String, hidden: bool }, SetSheet { block: String } }`
  - `#[derive(Serialize)] #[serde(tag = "type", rename_all = "snake_case")] pub enum Outcome { Measured { measurement: Measurement }, Removed { id: String }, Changed, Nothing }`
  - `pub fn Session::run(&mut self, cmd: Command) -> Result<Outcome, CoreError>`: records an undo entry for the first six variants (snapshot of `takeoff` before and after is enough at this size); `pub fn Session::undo(&mut self) -> bool`, `pub fn Session::redo(&mut self) -> bool`, `pub fn Session::can_undo(&self) -> bool`, `can_redo`.
  - Rules: `MeasureDistance` sums the polyline length of `points` (≥ 2 points); `MeasureArea` needs ≥ 3 points, closes the ring, sets `self_intersecting`; `MeasureEntity` measures the flattened entity: closed paths give Area with the area and open paths give Length, an `Insert` gives Count 1 with `block_name`; `CountBlock` counts `Insert`s of that block in the current sheet on visible layers (and on `layer` only when given), value = count, `handles` = all of them. Description defaults: `"Distance"`, `"Area"`, `"<EntityKind> on <layer>"`, `"<block_name> count"`. Conversion: `shown = value * f` (Length), `value * f²` (Area), `value` (Count) with `f = units.to_metres()`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn distance_and_area_commands() { /* MeasureDistance [(0,0),(3,0),(3,4)] -> Length 7.0, unit "m", shown 7.0; MeasureArea rect 4x3 -> Area 12.0, not self_intersecting */ }
#[test] fn bowtie_area_is_flagged() { /* MeasureArea (0,0)(2,2)(2,0)(0,2) -> self_intersecting true, value 2.0 */ }
#[test] fn measure_entity_rect_line_and_insert() { /* rect handle -> Area 12; line -> Length 10; an insert -> Count 1 with block_name DOOR */ }
#[test] fn count_blocks_respects_hidden_layers() { /* CountBlock DOOR -> 3; hide A-DOOR -> 0 */ }
#[test] fn unknown_units_assume_mm() { /* sample with insertion_units 0: Length 1000 drawing units -> shown 1.0 "m"; info().units == "unknown" */ }
#[test] fn undo_redo_measurements() { /* two measures, undo -> 1 item, undo -> 0, redo -> 1 */ }
#[test] fn totals_group_by_kind_and_unit() { /* two areas 12 and 3 -> one Total {Area, "m2", 2, 15.0} */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p qiyas-core`
Expected: compile errors.

- [ ] **Step 3: Implement `takeoff.rs`, `undo.rs`, `commands.rs`; extend `session.rs`**

- [ ] **Step 4: Run tests**

Run: `cargo test -p qiyas-core`
Expected: all pass.

- [ ] **Step 5: Expose over IPC**: add Tauri commands `run_command(cmd: Command) -> Result<Outcome, String>`, `undo() -> bool`, `redo() -> bool`, `takeoff() -> Result<Vec<Measurement>, String>`, `totals() -> Result<Vec<Total>, String>` in `commands.rs`; mirror in `src/api.ts`.

- [ ] **Step 6: Commit**

```bash
git add crates src/api.ts && git commit -m "feat(core): measure distance, area, entity and count into an undoable take-off"
```

---

### Task 9: Measure tools and take-off panel (frontend)

**Files:**
- Create: `src/tools/useTool.ts`, `src/tools/overlay.ts`, `src/shell/TakeoffPanel.tsx`
- Modify: `src/App.tsx`, `src/viewport/Viewport.tsx` (draw the in-progress rubber band from `overlay.ts`), `src/i18n/{en,ar}.json`
- Test: `src/tools/useTool.test.ts`, `src/shell/TakeoffPanel.test.tsx`

**Interfaces:**
- Consumes: Task 8 IPC (`runCommand`, `takeoff`, `totals`, `undo`, `redo`, `pick`, `entityInfo`).
- Produces:
  - `useTool(tool: Tool, api)`: returns `{ points: [number, number][]; onClick(world); onDoubleClick(); onEscape(); hint: string }`. Rules: `select` click picks and highlights, then a second click on the same highlighted entity runs `MeasureEntity`; `distance` collects points and runs `MeasureDistance` on double click or Enter (≥ 2 points); `area` the same with `MeasureArea` (≥ 3); `count` picks an entity, and if it is an `Insert` runs `CountBlock { block_name, layer: null }`; Escape clears points. Pure state logic in a reducer `toolReducer(state, event)` so it is testable without React.
  - `overlay.ts`: `overlaySegments(points, cursor, tool): Float32Array` for the rubber band (closed preview for `area`).
  - `TakeoffPanel`: props `{ items: Measurement[]; totals: Total[]; selectedId: string | null; onSelect(id); onRemove(id); onRename(id, text) }`. Rows show description, shown value with 3 decimals, unit, layer; totals at the bottom per kind. Selecting a row highlights its handles or points in the viewport and zooms to them.

- [ ] **Step 1: Write the failing tests**

```ts
test("distance tool needs two points then a finish", () => { /* reducer: click, click, finish -> emits {type:"measure_distance", points:[...]} */ });
test("area tool rejects finish with two points", () => { /* click, click, finish -> no emit, hint key "measure.needThree" */ });
test("escape clears points", () => {});
test("takeoff panel lists items and totals", () => { /* render 2 items, 1 total; row count 2; total text contains "15.000" */ });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test`
Expected: FAIL.

- [ ] **Step 3: Implement the reducer, hook, overlay and panel; wire into `App.tsx` with keyboard shortcuts (D distance, A area, C count, Esc, Enter, Ctrl+Z, Ctrl+Y)**

- [ ] **Step 4: Run tests and typecheck**

Run: `pnpm test && pnpm typecheck`
Expected: all pass.

- [ ] **Step 5: Run the app and measure the sample**

Run: `pnpm tauri dev`
Expected: distance 0,0 to 10,0 shows 10.000 m; area of the rectangle 12.000 m2; count DOOR shows 3 pcs; rows highlight in violet; undo removes the last row.

- [ ] **Step 6: Commit**

```bash
git add src && git commit -m "feat(ui): distance, area, count tools and the take-off panel"
```

---

### Task 10: Project file and Excel export

**Files:**
- Create: `crates/qiyas-core/src/project.rs`, `crates/qiyas-boq/{Cargo.toml,src/lib.rs,src/export.rs}`
- Modify: `crates/qiyas-core/src/session.rs` (load project on open, `save_project`), `crates/qiyas-desktop/src/commands.rs`, `src/api.ts`, `src/App.tsx` (Export button uses the save dialog with `.xlsx`)
- Test: `crates/qiyas-core/tests/project.rs`, `crates/qiyas-boq/tests/export.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize)] pub struct Project { pub version: u32 /* 1 */, pub drawing: String /* file name only */, pub takeoff: Takeoff }`; `pub fn project_path(drawing: &Path) -> PathBuf` = same folder, same stem, extension `qiyas` (`villa.dwg` → `villa.qiyas`).
  - `pub fn Session::save_project(&self) -> Result<PathBuf, CoreError>` writes pretty JSON atomically (temp file then rename). `Session::open` loads it when present; a project whose `drawing` name differs is still loaded but `problems` gets "Project file was saved for <name>".
  - `qiyas_boq::export_takeoff(path: &Path, items: &[Measurement], totals: &[Total], title: &str) -> Result<(), BoqError>`: sheet "Take-off" with header row `Description | Kind | Quantity | Unit | Layer | Sheet | Note | Id`, one row per item, a blank row, then totals; sheet "Totals" repeating totals. Numbers written as numbers with format `0.000`.
  - Tauri commands `save_project() -> Result<String, String>` and `export_takeoff(path: String) -> Result<(), String>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn project_round_trip() { /* measure twice, save_project, reopen drawing -> takeoff has 2 items with the same ids */ }
#[test] fn project_path_rule() { assert_eq!(project_path(Path::new("/x/villa.DWG")), PathBuf::from("/x/villa.qiyas")); }
#[test] fn export_is_readable_by_calamine() { /* export 2 items; open with calamine; sheet "Take-off" row 1 col 0 == "Description"; row 2 col 2 == 12.0 */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p qiyas-core -p qiyas-boq`
Expected: compile errors.

- [ ] **Step 3: Implement `project.rs`, `qiyas-boq`, the IPC commands and the Export button**

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates src && git commit -m "feat: .qiyas project file and Excel take-off export"
```

---

### Task 11: Save the drawing with measurements inside

**Files:**
- Create: `crates/qiyas-core/src/embed.rs`
- Modify: `crates/qiyas-core/src/session.rs` (`save_as`, `save`), `crates/qiyas-core/src/drawing.rs` (read embedded measurements on open), `crates/qiyas-desktop/src/commands.rs`, `src/api.ts`, `src/App.tsx` (Save, Save as with the dialog filter DWG and DXF)
- Test: `crates/qiyas-core/tests/save.rs`

**Interfaces:**
- Produces:
  - Layers `QIYAS-LENGTH` (colour index 6), `QIYAS-AREA` (index 5), `QIYAS-COUNT` (index 4), created on save when missing.
  - `pub fn embed_measurements(doc: &mut CadDocument, items: &[Measurement])`: removes every entity that carries xdata app `QIYAS`, then writes one `LwPolyline` per Length (open) and Area (closed) measurement from `points` (or from the flattened source entity when `points` is empty), and one `Point` per handle of a Count measurement at that insert's position; each gets `ExtendedDataRecord::new("QIYAS")` with values `String(id)`, `String(kind)`, `Real(value)` (or whichever `XDataValue` variant holds an f64 in the pinned revision), `String(description)`, `String(layer)`, `String(sheet)`, `String(block_name or "")`. The app id `QIYAS` is registered in `doc.app_ids`.
  - `pub fn read_embedded(doc: &CadDocument) -> Vec<Measurement>` rebuilds measurements from xdata (points from the polyline vertices, handles empty, `source = "user"`).
  - `Session::save_as(&mut self, path: &Path) -> Result<(), CoreError>`: embeds, writes with `DwgWriter::write_to_file` for `.dwg` and `DxfWriter::new(&doc).write_to_file` for `.dxf`, saves the project file next to the new path, updates `drawing.path`. `Session::save` uses the current path.
  - `Session::open`: when no project file exists, `takeoff.items = read_embedded(&doc)`.
  - Tauri commands `save_drawing() -> Result<(), String>`, `save_drawing_as(path: String) -> Result<(), String>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn save_dwg_reopen_without_project_rebuilds_measurements() { /* open sample dxf; measure distance and area and count; save_as out.dwg; delete out.qiyas; open out.dwg -> 3 items, same ids and kinds, values within 1e-6 */ }
#[test] fn save_dxf_from_dwg_too() { /* same from a dwg source to .dxf */ }
#[test] fn saving_twice_does_not_duplicate() { /* save_as, save again -> entities with QIYAS xdata count == items count (3 for the sample: 1 polyline + 1 polyline + 3 points = 5 entities) */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p qiyas-core`
Expected: compile errors.

- [ ] **Step 3: Implement `embed.rs` and the save paths**

- [ ] **Step 4: Run the full check**

Run: `scripts/check.sh`
Expected: exits 0.

- [ ] **Step 5: Run the app: open a real DWG from the founder's local folder, measure, save as a new DWG, reopen**

Expected: the new file opens with the measurements present; the file also opens in another CAD viewer with the QIYAS layers visible.

- [ ] **Step 6: Commit**

```bash
git add crates src && git commit -m "feat: save DWG and DXF with measurements embedded on QIYAS layers"
```

---

### Task 12: Brand files and app icon

**Files (workspace repo `qs-mind`, not `qiyas`):**
- Create: `brand/qiyas/mark/{qiyas-mark.svg,qiyas-mark-white.svg,qiyas-mark-small.svg,qiyas-mark-small-white.svg}`, `brand/qiyas/lockup/{qiyas-lockup.svg,qiyas-lockup-white.svg,qiyas-lockup-ar.svg,qiyas-lockup-ar-white.svg}`, `brand/qiyas/favicon/favicon.svg`
- Modify: `brand/README.md` (colour row `Anodised titanium | #C9A6E0 → #6E4A9E | Qiyas`, `<brand>` list), `brand/guide.html` (Qiyas section)

**Files (qiyas repo):**
- Create: `crates/qiyas-desktop/icons/*` generated from the mark, `public/favicon.svg`, `src/shell/About.tsx`

**Interfaces:**
- Consumes: `brand/quantix/*` as the geometric template: same sphere paths and class names, only the `.face` gradient stops and the `.wordmark`/`.product` shapes change.

- [ ] **Step 1: Build the Qiyas SVGs from the Quantix ones**: copy each Quantix file, replace the gradient `id` with `qiyas-face`, stops `#C9A6E0` and `#6E4A9E`, replace the wordmark shapes with "Qiyas" set in Instrument Sans Bold converted to paths, and the Arabic lockup with "قياس" in Thmanyah Sans Bold with `swsh`, converted to paths (use a local Inkscape or fontTools run; the font files never enter the repo).

- [ ] **Step 2: Check in the browser**: open `brand/guide.html` and each SVG; the face sits upper right, lines ink on light and white on dark, the small mark reads at 16 px.

- [ ] **Step 3: Generate app icons**: export `qiyas-mark.svg` on a transparent 1024 px PNG, run `pnpm tauri icon <png>` in the qiyas repo. The About box shows the lockup and the line "Qiyas, a QS Mind app" (Arabic: "قياس، تطبيق من كيو إس مايند").

- [ ] **Step 4: Commit (two repos)**

```bash
cd /home/kareem/Work/qs-mind && git add brand && git commit -m "brand: Qiyas mark, lockups and favicon in anodised titanium"
cd /home/kareem/Work/qs-mind/qiyas && git add crates/qiyas-desktop/icons public src && git commit -m "brand: Qiyas app icons and About box"
```

---

### Task 13: Final check and hand-over

- [ ] **Step 1: Run `scripts/check.sh`** — expected exit 0.
- [ ] **Step 2: Run the app on three real drawings** (one villa set, one with many xref-free blocks, one over 20 MB). Record open time and pan frame rate in `docs/2026-10-08-qiyas-foundation-check.md` in the workspace repo, with what broke.
- [ ] **Step 3: Tell the founder** what works, what was measured, and wait. No push, no release.
