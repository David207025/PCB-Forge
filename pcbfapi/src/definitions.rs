//! Data model definitions and the in-memory Typst compiler world.
//!
//! This module contains:
//! - JSON-serializable structs that describe PCB Forge templates and projects
//!   ([`Template`], [`PageLayout`], [`PageConfig`], [`ProjectConfig`])
//! - [`InMemoryWorld`] — a lightweight implementation of Typst's [`World`]
//!   trait that compiles `.typ` scripts entirely in memory without touching
//!   the filesystem for package resolution (embedded packages are compiled
//!   into the binary via [`include_dir!`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration as TypstDuration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use chrono::{Datelike, Duration as ChronoDuration, Local};
use include_dir::{include_dir, Dir};

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::Tokenizer;

/// Typst packages embedded into the binary at compile time.
///
/// The directory at `pcbfapi/typst/packages/` is recursively compiled in so
/// that the process has no runtime dependency on the Typst package registry.
static EMBEDDED_PACKAGES: Dir = include_dir!("$CARGO_MANIFEST_DIR/typst/packages");

// ─────────────────────────────────────────────────────────────────────────────
// Configuration structs (serialized to / deserialized from JSON)
// ─────────────────────────────────────────────────────────────────────────────

/// A template definition stored in `~/.pcb-forge/templates/src/<name>/meta.json`.
///
/// `global_fields` are shared across all pages of a project (e.g. author name,
/// project title). `local_fields` are per-page overrides (e.g. document type,
/// sheet number).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Template {
  /// Optional `$schema` URI pointing to `template.schema.json`.
  /// Serialized as `"$schema"` in JSON.
  #[serde(rename = "$schema", default)]
  pub schema: Option<String>,
  /// Key → human-readable description pairs for project-wide fields.
  #[serde(default)]
  pub global_fields: HashMap<String, String>,
  /// Key → human-readable description pairs for per-page fields.
  #[serde(default)]
  pub local_fields: HashMap<String, String>,
}

/// Page size and orientation settings passed to the Typst layout function.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PageLayout {
  /// Paper size string recognized by Typst (e.g. `"a4"`, `"letter"`).
  pub size: String,
  /// `true` for landscape orientation, `false` for portrait.
  pub orientation: bool,
}

/// Configuration for a single page within a project.
///
/// Each page references one source asset (a KiCad schematic, PCB layout, or
/// Markdown document) along with per-page field values and an optional set of
/// extra CLI arguments forwarded to `kicad-cli`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PageConfig {
  /// Template layout variant name for this page (e.g. "default", "bom").
  pub variant: Option<String>,
  /// Layout dimensions for this page.
  pub layout: PageLayout,
  /// Per-page field values substituted into the Typst layout template.
  pub local_fields: HashMap<String, String>,
  /// Path to the source file for this page.
  pub path: String,
  /// Optional extra arguments passed verbatim to `kicad-cli` during export.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub extra_args: Option<Vec<String>>,
}

/// Top-level project configuration, deserialized from a user's `<name>.json` file.
///
/// The JSON file also carries a `$schema` field for IDE validation, but that
/// field is consumed by `forge::generate_project_pdf` before deserialization
/// and is not represented in this struct.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectConfig {
  /// Project-wide field values shared across all pages.
  pub global_fields: HashMap<String, String>,
  /// Ordered list of pages to render.
  pub pages: Vec<PageConfig>,
}

// Struct for the part payload
#[derive(Deserialize, Serialize, Debug)]
pub struct ElectronicPart {
  pub name: String,
  pub reference: String,
  pub value: String,
  pub description: String,
  pub link: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// In-memory Typst compiler world
// ─────────────────────────────────────────────────────────────────────────────

/// A self-contained implementation of Typst's [`World`] trait that compiles
/// scripts without requiring a Typst installation on the host system.
///
/// On construction it:
/// 1. Loads all system fonts via `fontdb`
/// 2. Registers an in-memory `main.typ` virtual source file
/// 3. Exposes embedded packages from the binary for `#import` resolution
///
/// Use [`InMemoryWorld::compile_pdf`] or [`InMemoryWorld::compile_pdf_with_root`]
/// as the primary entry point.
pub struct InMemoryWorld {
  library: LazyHash<Library>,
  book: LazyHash<FontBook>,
  fonts: Vec<Font>,
  main_id: FileId,
  sources: HashMap<FileId, Source>,
  root_dir: Option<PathBuf>,
}

impl InMemoryWorld {
  /// Creates a new world instance with `main_content` as the root Typst script.
  pub fn new(main_content: String) -> Self {
    Self::new_with_root(main_content, None)
  }
  
  /// Creates a new world instance specifying a base directory used to resolve
  /// relative asset paths (e.g., images or local template includes).
  pub fn new_with_root(main_content: String, root_dir: Option<&Path>) -> Self {
    let library = LazyHash::new(Library::default());
    
    let mut book = FontBook::new();
    let mut fonts = Vec::new();
    
    // Enumerate system fonts and register them with Typst's font book
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    
    for face in db.faces() {
      let path = match &face.source {
        fontdb::Source::File(p) => Some(p.as_path()),
        fontdb::Source::SharedFile(p, _) => Some(p.as_path()),
        _ => None,
      };
      
      if let Some(path) = path {
        if let Ok(data) = std::fs::read(path) {
          if let Some(font) = Font::new(Bytes::new(data), face.index) {
            book.push(font.info().clone());
            fonts.push(font);
          }
        }
      }
    }
    
    // Register the root script as the virtual "main.typ" file
    let main_id = FileId::new(RootedPath::new(
      VirtualRoot::Project,
      VirtualPath::new("main.typ").expect("Invalid virtual path"),
    ));
    let mut sources = HashMap::new();
    let source = Source::new(main_id, main_content);
    sources.insert(main_id, source);
    
    Self {
      library,
      book: LazyHash::new(book),
      fonts,
      main_id,
      sources,
      root_dir: root_dir.map(|p| p.to_path_buf()),
    }
  }
  
  /// Resolves a [`FileId`] to raw bytes:
  ///
  /// 1. **Embedded packages** — checks the [`EMBEDDED_PACKAGES`] binary blob
  ///    for package files referenced via Typst's `#import "@namespace/pkg:ver"` syntax.
  /// 2. **Relative filesystem paths** — resolves virtual relative paths strictly
  ///    against `root_dir` (or CWD if `root_dir` is omitted).
  fn read_bytes(&self, id: FileId) -> FileResult<Vec<u8>> {
    // 1. Check embedded packages
    if let Some(package) = id.package() {
      let rel_path = format!(
        "{}/{}/{}{}",
        package.namespace,
        package.name,
        package.version,
        id.vpath().as_rooted_path().display()
      );
      
      if let Some(file) = EMBEDDED_PACKAGES.get_file(&rel_path) {
        return Ok(file.contents().to_vec());
      }
    }
    
    // 2. Basic relative path resolution against root_dir
    let relative_path = id.vpath().as_rootless_path();
    let target_path = match &self.root_dir {
      Some(root) => root.join(relative_path),
      None => relative_path.to_path_buf(),
    };
    
    if let Ok(bytes) = std::fs::read(&target_path) {
      return Ok(bytes);
    }
    
    Err(FileError::NotFound(target_path))
  }
  
  /// Compiles a raw Typst script string into PDF bytes using the in-memory compiler engine.
  pub fn compile_pdf(main_content: String) -> Result<Vec<u8>, String> {
    Self::compile_pdf_with_root(main_content, None)
  }
  
  /// Compiles a raw Typst script string into PDF bytes with a root search path.
  pub fn compile_pdf_with_root(main_content: String, root_dir: Option<&Path>) -> Result<Vec<u8>, String> {
    let world = Self::new_with_root(main_content, root_dir);
    let output = typst::compile(&world);
    
    match output.output {
      Ok(document) => Ok(typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).unwrap()),
      Err(errors) => {
        let err_messages: Vec<String> = errors
          .into_iter()
          .map(|e| format!("span {:?}: {}", e.span, e.message))
          .collect();
        Err(format!("Typst in-memory compilation failed:\n{}", err_messages.join("\n")))
      }
    }
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// World trait implementation
// ─────────────────────────────────────────────────────────────────────────────

impl World for InMemoryWorld {
  /// Returns the standard Typst library (built-in functions and types).
  fn library(&self) -> &LazyHash<Library> {
    &self.library
  }
  
  /// Returns the font book used for font resolution during layout.
  fn book(&self) -> &LazyHash<FontBook> {
    &self.book
  }
  
  /// Returns the [`FileId`] of the root script (`main.typ`).
  fn main(&self) -> FileId {
    self.main_id
  }
  
  /// Looks up a source file by its [`FileId`], creating a new [`Source`] entry
  /// if it has not been seen before (e.g. an `#import`-ed file).
  fn source(&self, id: FileId) -> FileResult<Source> {
    if let Some(source) = self.sources.get(&id) {
      return Ok(source.clone());
    }
    
    // Resolve via read_bytes (handles embedded packages and disk files)
    let bytes = self.read_bytes(id)?;
    let text = std::str::from_utf8(&bytes)
      .map_err(|_| FileError::InvalidUtf8)?;
    
    Ok(Source::new(id, text.to_string()))
  }
  
  /// Returns the raw bytes for a binary asset (images, fonts) referenced from
  /// a Typst document.
  fn file(&self, id: FileId) -> FileResult<Bytes> {
    let bytes = self.read_bytes(id)?;
    Ok(Bytes::new(bytes))
  }
  
  /// Returns the font at the given index in the loaded font list.
  fn font(&self, index: usize) -> Option<Font> {
    self.fonts.get(index).cloned()
  }
  
  /// Returns today's date, optionally shifted by `offset` (used by Typst's
  /// `datetime.today()` function).
  fn today(&self, offset: Option<TypstDuration>) -> Option<Datetime> {
    let now = Local::now();
    let adjusted = match offset {
      Some(dur) => now + ChronoDuration::seconds(dur.seconds() as i64),
      None => now,
    };
    
    Datetime::from_ymd(
      adjusted.year(),
      adjusted.month() as u8,
      adjusted.day() as u8,
    )
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Embedding Model
// ─────────────────────────────────────────────────────────────────────────────

fn download_if_missing(url: &str, dest_path: &Path) -> anyhow::Result<()> {
  if !dest_path.exists() {
    let response = reqwest::blocking::get(url)?.error_for_status()?;
    let bytes = response.bytes()?;
    std::fs::write(dest_path, bytes)?;
  }
  Ok(())
}

pub struct EmbeddingModel {
  model: BertModel,
  tokenizer: Tokenizer,
  device: Device,
}

impl EmbeddingModel {
  pub fn new() -> anyhow::Result<Self> {
    let device = Device::Cpu;
    
    // Custom cache path: ~/.pcb-forge/cache/models
    let cache_dir = dirs::home_dir()
      .ok_or_else(|| anyhow::anyhow!("Could not determine user home directory"))?
      .join(".pcb-forge")
      .join("cache")
      .join("models");
    std::fs::create_dir_all(&cache_dir)?;
    
    let base_url = "https://huggingface.co/BAAI/bge-small-en-v1.5/tree/main";
    
    let config_filename = cache_dir.join("config.json");
    let tokenizer_filename = cache_dir.join("tokenizer.json");
    let weights_filename = cache_dir.join("model.safetensors");
    
    download_if_missing(&format!("{}/config.json", base_url), &config_filename)?;
    download_if_missing(&format!("{}/tokenizer.json", base_url), &tokenizer_filename)?;
    download_if_missing(&format!("{}/model.safetensors", base_url), &weights_filename)?;
    
    let config: Config = serde_json::from_str(&std::fs::read_to_string(&config_filename)?)?;
    let tokenizer = Tokenizer::from_file(&tokenizer_filename).map_err(anyhow::Error::msg)?;
    
    let vb = unsafe {
      VarBuilder::from_mmaped_safetensors(&[weights_filename], DTYPE, &device)?
    };
    let model = BertModel::load(vb, &config)?;
    
    Ok(Self {
      model,
      tokenizer,
      device,
    })
  }
  
  pub fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
    let tokens = self.tokenizer.encode(text, true).map_err(anyhow::Error::msg)?;
    let token_ids = Tensor::new(tokens.get_ids(), &self.device)?.unsqueeze(0)?;
    let token_type_ids = token_ids.zeros_like()?;
    
    let embeddings = self.model.forward(&token_ids, &token_type_ids, None)?;
    
    let (_n_sentence, n_tokens, _hidden_size) = embeddings.dims3()?;
    let mean_embeddings = (embeddings.sum(1)? / (n_tokens as f64))?;
    
    let norm = mean_embeddings.sqr()?.sum_keepdim(1)?.sqrt()?;
    let normalized = (mean_embeddings / norm)?;
    
    Ok(normalized.squeeze(0)?.to_vec1::<f32>()?)
  }
}