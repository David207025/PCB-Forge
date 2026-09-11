# PCB-Forge API (pcbfapi) Documentation

> Auto-generated from Rust source doc comments.

## Module: `main.rs`

PCB Forge API — main entry point.

This binary starts two concurrent runtimes on the same thread:

- **Axum HTTP server** (async, on a Tokio worker pool) — exposes a local
  REST API on `127.0.0.1:47210` that the VS Code extension calls.
- **tao event loop** (synchronous, on the main thread) — drives the macOS /
  Windows system tray icon and processes menu events.

The two halves communicate via a `tao` [`EventLoopProxy`] that the Axum
handlers use to fire [`UserEvent`]s into the event loop (e.g. to update
the tray progress percentage).

## API endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/init-template` | Create a new template scaffold |
| `POST` | `/init-project` | Create a project JSON from a template |
| `POST` | `/gen-templates` | Regenerate schemas for all templates |
| `POST` | `/preview-template` | Generate a single-page preview PDF |
| `POST` | `/gen-project` | Compile the full project PDF |

### Declarations & Functions

#### `pub struct ApiResponse<T = serde_json::Value>`

Standardized envelope returned by every API endpoint.

```json
{ "status": "success", "message": "…", "data": { … } }
{ "status": "error",   "message": "…" }
```

The `data` field is omitted from the serialized JSON when it is `None`.

---

#### `pub fn success( message: impl Into<String>, data: Option<T>, ) -> (StatusCode, Json<ApiResponse<T>>)`

Constructs a `200 OK` response with an optional data payload.

---

#### `pub fn error( status_code: StatusCode, message: impl Into<String>, ) -> (StatusCode, Json<ApiResponse<serde_json::Value>>)`

Constructs an error response with a given HTTP status code and message.
The `data` field is always `None` for error responses.

---

#### `pub enum UserEvent`

Custom events sent from Axum handler threads into the tao event loop via
[`EventLoopProxy::send_event`].

---

#### `struct InitTemplatePayload`

Payload for `POST /init-template`.

---

#### `struct InitProjectPayload`

Payload for `POST /init-project`.

---

#### `struct PreviewTemplatePayload`

Payload for `POST /preview-template`.

---

#### `struct GenProjectPayload`

Payload for `POST /gen-project`.

---

#### `struct AppState`

Axum application state, cloned into every request handler.

Currently only carries the tao event loop proxy used to push progress
updates to the tray icon from async contexts.

---

## Module: `forge.rs`

Forge engine — file resolution, asset export, and PDF generation.

This is the core module of the PCB Forge API. It provides:

- **Directory helpers** — canonical paths for the `~/.pcb-forge/` directory
  tree (cache, schemas, template sources and generated schemas).
- **KiCad CLI discovery** — locates `kicad-cli` on the host system across
  macOS, Windows, and Linux.
- **Asset resolution** — converts project paths (`.kicad_sch`, `.kicad_pcb`,
  `.md`, images) into relative paths, running KiCad exports to SVG as needed
  and caching the results to avoid redundant re-exports.
- **SVG post-processing** — removes background rectangles and recomputes
  tight viewBox bounds for cleaner embedded images.
- **Typst compilation** — builds the script that drives the in-memory Typst
  engine and produces per-page and full-project PDFs.
- **Schema generation** — produces JSON Schema files used by IDEs for
  `meta.json` and project JSON validation.

### Declarations & Functions

#### `pub fn make_relative_path(path: &Path, base_dir: &Path) -> String`

Helper to convert a path to a relative path string with respect to `base_dir` if possible.

---

#### `fn escape_typst_string(s: &str) -> String`

Escapes a Rust string for safe embedding inside a Typst string literal.

---

#### `pub fn json_to_typst(val: &serde_json::Value) -> String`

Recursively converts a [`serde_json::Value`] into a Typst expression literal.

---

#### `pub fn get_home_dir() -> PathBuf`

Returns the root `~/.pcb-forge/` directory path.

---

#### `pub fn get_cache_dir() -> PathBuf`

Returns `~/.pcb-forge/cache/`, creating it if it does not exist.

---

#### `pub fn get_schemas_dir() -> PathBuf`

Returns `~/.pcb-forge/schemas/`, creating it if it does not exist.

---

#### `pub fn get_templates_src_dir() -> PathBuf`

Returns `~/.pcb-forge/templates/src/`, creating it if it does not exist.

---

#### `pub fn get_templates_generated_dir() -> PathBuf`

Returns `~/.pcb-forge/templates/generated/`, creating it if it does not exist.

---

#### `pub fn get_kicad_cli_path() -> Result<PathBuf, String>`

Discovers the absolute path to the `kicad-cli` executable.

---

#### `pub fn resolve_project_path(raw_path_str: &str, base_dir: &Path) -> String`

Resolves a raw path string into a relative path string relative to `base_dir`.

---

#### `pub fn copy_template_assets(template_name: &str, target_dir: &Path) -> Result<Vec<PathBuf>, String>`

Copies all template assets (meta.json, schema.json, bom/, default/, etc.)
into `target_dir` and returns a list of every top-level path created.

---

#### `pub fn preprocess_markdown_and_copy_dependencies( md_content: &str, md_file_path: &Path, out_dir: &Path, ) -> String`

Preprocesses Markdown content, resolves and copies all file dependencies (like images)
into the output directory, and updates link references for Typst compilation.

---

#### `pub fn generate_page_pdf( template_name: &str, global_fields: &std::collections::HashMap<String, String>, page: &PageConfig, project_dir: &Path, output_pdf_path: &Path, ) -> Result<(), String>`

Compiles a single page into a PDF.

Takes project-wide global fields, a single page configuration, the root project
directory, and the target output PDF path. It:
1. Locates and copies all template assets into a temporary workspace.
2. Resolves and compiles/recreates the required page source asset (SVGs, Markdown, BOMs).
3. Generates and executes the Typst compilation script via in-memory world.
4. Cleans up all intermediate files and generated assets (including exported SVGs).

---

#### `fn build_single_page_script( template_name: &str, project: &ProjectConfig, project_dir: &Path, ) -> Result<String, String>`

Builds the standalone Typst script for rendering a single page.

---

## Module: `definitions.rs`

Data model definitions and the in-memory Typst compiler world.

This module contains:
- JSON-serializable structs that describe PCB Forge templates and projects
  ([`Template`], [`PageLayout`], [`PageConfig`], [`ProjectConfig`])
- [`InMemoryWorld`] — a lightweight implementation of Typst's [`World`]
  trait that compiles `.typ` scripts entirely in memory without touching
  the filesystem for package resolution (embedded packages are compiled
  into the binary via [`include_dir!`]).

### Declarations & Functions

#### `pub struct Template`

A template definition stored in `~/.pcb-forge/templates/src/<name>/meta.json`.

`global_fields` are shared across all pages of a project (e.g. author name,
project title). `local_fields` are per-page overrides (e.g. document type,
sheet number).

---

#### `pub struct PageLayout`

Page size and orientation settings passed to the Typst layout function.

---

#### `pub struct PageConfig`

Configuration for a single page within a project.

Each page references one source asset (a KiCad schematic, PCB layout, or
Markdown document) along with per-page field values and an optional set of
extra CLI arguments forwarded to `kicad-cli`.

---

#### `pub struct ProjectConfig`

Top-level project configuration, deserialized from a user's `<name>.json` file.

The JSON file also carries a `$schema` field for IDE validation, but that
field is consumed by `forge::generate_project_pdf` before deserialization
and is not represented in this struct.

---

#### `pub struct InMemoryWorld`

A self-contained implementation of Typst's [`World`] trait that compiles
scripts without requiring a Typst installation on the host system.

On construction it:
1. Loads all system fonts via `fontdb`
2. Registers an in-memory `main.typ` virtual source file
3. Exposes embedded packages from the binary for `#import` resolution

Use [`InMemoryWorld::compile_pdf`] or [`InMemoryWorld::compile_pdf_with_root`]
as the primary entry point.

---

#### `pub fn new(main_content: String) -> Self`

Creates a new world instance with `main_content` as the root Typst script.

---

#### `pub fn new_with_root(main_content: String, root_dir: Option<&Path>) -> Self`

Creates a new world instance specifying a base directory used to resolve
relative asset paths (e.g., images or local template includes).

---

#### `fn read_bytes(&self, id: FileId) -> FileResult<Vec<u8>>`

Resolves a [`FileId`] to raw bytes:

1. **Embedded packages** — checks the [`EMBEDDED_PACKAGES`] binary blob
   for package files referenced via Typst's `#import "@namespace/pkg:ver"` syntax.
2. **Relative filesystem paths** — resolves virtual relative paths strictly
   against `root_dir` (or CWD if `root_dir` is omitted).

---

#### `pub fn compile_pdf(main_content: String) -> Result<Vec<u8>, String>`

Compiles a raw Typst script string into PDF bytes using the in-memory compiler engine.

---

#### `pub fn compile_pdf_with_root(main_content: String, root_dir: Option<&Path>) -> Result<Vec<u8>, String>`

Compiles a raw Typst script string into PDF bytes with a root search path.

---

#### `fn library(&self) -> &LazyHash<Library>`

Returns the standard Typst library (built-in functions and types).

---

#### `fn book(&self) -> &LazyHash<FontBook>`

Returns the font book used for font resolution during layout.

---

#### `fn main(&self) -> FileId`

Returns the [`FileId`] of the root script (`main.typ`).

---

#### `fn source(&self, id: FileId) -> FileResult<Source>`

Looks up a source file by its [`FileId`], creating a new [`Source`] entry
if it has not been seen before (e.g. an `#import`-ed file).

---

#### `fn file(&self, id: FileId) -> FileResult<Bytes>`

Returns the raw bytes for a binary asset (images, fonts) referenced from
a Typst document.

---

#### `fn font(&self, index: usize) -> Option<Font>`

Returns the font at the given index in the loaded font list.

---

#### `fn today(&self, offset: Option<TypstDuration>) -> Option<Datetime>`

Returns today's date, optionally shifted by `offset` (used by Typst's
`datetime.today()` function).

---

## Module: `tray.rs`

Tray icon module for PCB Forge.

Manages the system tray icon, its tooltip, and the status menu item
that reflects background processing progress (e.g. PDF generation,
schema batch runs).  All state is stored in `static mut` globals because
the tray-icon / tao crates require the icon to be created on the main
thread and kept alive for the lifetime of the process.

### Declarations & Functions

#### `pub fn set_event_proxy(proxy: EventLoopProxy<UserEvent>)`

Registers the event loop proxy so worker threads can safely dispatch
status updates to the main thread event loop.

---

#### `pub fn load_icon() -> Icon`

Loads the embedded `res/icon.png` and converts it to a tray-icon [`Icon`].

The icon bytes are compiled into the binary at build time via
`include_bytes!`.  Near-black pixels (R < 15, G < 15, B < 15) are made
fully transparent so the icon renders cleanly on dark menu bars.

---

#### `pub fn initialize_tray(menu: Menu)`

Creates the system tray icon with the provided context menu and stores it
in [`GLOBAL_TRAY`].

This function is idempotent — it does nothing if the tray is already active
([`TRAY_ACTIVE`] is `true`).

# Panics
Panics if the underlying tray icon builder fails (e.g. unsupported platform
or missing icon data).

---

#### `pub fn update_process_status(status_percent: u8)`

Thread-safe: dispatches a status update event to the main-thread event loop.

Safe to call from any Tokio worker or background thread.

---

#### `pub fn reset_process_status()`

Thread-safe: dispatches a reset event to the main-thread event loop.

Safe to call from any Tokio worker or background thread.

---

#### `pub fn apply_process_status(status_percent: u8)`

Actually updates the tray status label and tooltip on macOS AppKit main thread.

# Safety
Must only be called from the main thread inside `event_loop.run`.

---

#### `pub fn apply_reset_status()`

Actually resets the tray status label and tooltip on macOS AppKit main thread.

# Safety
Must only be called from the main thread inside `event_loop.run`.

---

