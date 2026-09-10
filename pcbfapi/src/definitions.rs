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


static EMBEDDED_PACKAGES: Dir = include_dir!("$CARGO_MANIFEST_DIR/typst/packages");


#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Template {
  #[serde(rename = "$schema", default)]
  pub schema: Option<String>,
  #[serde(default)]
  pub global_fields: HashMap<String, String>,
  #[serde(default)]
  pub local_fields: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PageLayout {
  pub size: String,
  pub orientation: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PageConfig {
  pub layout: PageLayout,
  pub local_fields: HashMap<String, String>,
  pub content: String,
  pub path: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub extra_args: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectConfig {
  pub layout: String,
  pub global_fields: HashMap<String, String>,
  pub pages: Vec<PageConfig>,
}

pub struct InMemoryWorld {
  library: LazyHash<Library>,
  book: LazyHash<FontBook>,
  fonts: Vec<Font>,
  main_id: FileId,
  sources: HashMap<FileId, Source>,
}

impl InMemoryWorld {
  pub fn new(main_content: String) -> Self {
    let library = LazyHash::new(Library::default());
    
    let mut book = FontBook::new();
    let mut fonts = Vec::new();
    
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
    }
  }
  
  fn read_bytes(&self, id: FileId) -> FileResult<Vec<u8>> {
    // 1. Attempt lookup inside embedded packages if package metadata exists
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
    
    // 2. Disk fallback (resolves local non-package files and package calls to absolute disk paths)
    let rootless = id.vpath().as_rootless_path();
    
    if rootless.is_file() {
      if let Ok(bytes) = std::fs::read(rootless) {
        return Ok(bytes);
      }
    }
    
    let abs_path = Path::new("/").join(rootless);
    if abs_path.is_file() {
      if let Ok(bytes) = std::fs::read(&abs_path) {
        return Ok(bytes);
      }
    }
    
    let err_path = if let Some(package) = id.package() {
      PathBuf::from(format!(
        "{}/{}/{}{}",
        package.namespace,
        package.name,
        package.version,
        id.vpath().as_rooted_path().display()
      ))
    } else {
      abs_path
    };
    
    Err(FileError::NotFound(err_path))
  }
  
  /// Compiles a raw Typst script into PDF bytes using the in-memory compiler engine.
  pub fn compile_pdf(main_content: String) -> Result<Vec<u8>, String> {
    let world = Self::new(main_content);
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

impl World for InMemoryWorld {
  fn library(&self) -> &LazyHash<Library> {
    &self.library
  }
  
  fn book(&self) -> &LazyHash<FontBook> {
    &self.book
  }
  
  fn main(&self) -> FileId {
    self.main_id
  }
  
  fn source(&self, id: FileId) -> FileResult<Source> {
    if let Some(source) = self.sources.get(&id) {
      return Ok(source.clone());
    }
    
    // Resolve source content via read_bytes (handles both embedded packages and local files)
    let bytes = self.read_bytes(id)?;
    let text = std::str::from_utf8(&bytes)
      .map_err(|_| FileError::InvalidUtf8)?;
    
    Ok(Source::new(id, text.to_string()))
  }
  
  fn file(&self, id: FileId) -> FileResult<Bytes> {
    let bytes = self.read_bytes(id)?;
    Ok(Bytes::new(bytes))
  }
  
  fn font(&self, index: usize) -> Option<Font> {
    self.fonts.get(index).cloned()
  }
  
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