use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

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