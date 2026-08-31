use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleManifest {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub status: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
}
