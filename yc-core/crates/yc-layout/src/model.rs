use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LayoutYaml {
    pub layout_id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub rows: Vec<Vec<KeyYaml>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct KeyYaml {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default = "default_width")]
    pub width: f32,
}

fn default_width() -> f32 {
    1.0
}
