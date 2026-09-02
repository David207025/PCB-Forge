use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration as TypstDuration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use chrono::{Datelike, Duration as ChronoDuration, Local};

fn default_thickness() -> f32 {
  1.0
}

fn default_font_size() -> f32 {
  12.0
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct FontConfig {
  #[serde(default)]
  pub family: Option<String>,
  #[serde(default = "default_font_size")]
  pub size: f32,
  #[serde(default)]
  pub weight: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Dimensions {
  pub width: f32,
  pub height: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Node {
  Container {
    id: String,
    ml: f32,
    mt: f32,
    mr: f32,
    mb: f32,
    #[serde(default = "default_thickness")]
    thickness: f32,
    #[serde(default)]
    children: Vec<Node>,
  },
  Text {
    id: String,
    ml: f32,
    mt: f32,
    mr: f32,
    mb: f32,
    text: String,
    #[serde(default)]
    font: Option<FontConfig>,
  },
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Template {
  #[serde(rename = "$schema", default)]
  pub schema: Option<String>,
  pub dimensions: Dimensions,
  pub root: Vec<Node>,
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

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PageConfig {
  pub layout: PageLayout,
  #[serde(rename = "localFields")]
  pub local_fields: HashMap<String, String>,
  pub content: String,
  pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct ProjectConfig {
  #[serde(rename = "globalFields")]
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
      Ok(source.clone())
    } else {
      let path = id.vpath().as_rootless_path();
      Err(FileError::NotFound(path.to_path_buf()))
    }
  }
  
  fn file(&self, id: FileId) -> FileResult<Bytes> {
    let path = id.vpath().as_rootless_path();
    std::fs::read(path)
      .map(Bytes::new)
      .map_err(|_| FileError::NotFound(path.to_path_buf()))
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