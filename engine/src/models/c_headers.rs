use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct HeaderModuleItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_url: String,
    pub downloaded: bool,
    pub selected: bool,
    pub local_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadHeaderRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectHeaderRequest {
    pub id: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelectedHeaderMetadata {
    pub id: String,
    pub include_hint: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeaderModuleState {
    pub headers: Vec<HeaderModuleItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeaderSelectionMetadata {
    pub selected_headers: Vec<SelectedHeaderMetadata>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleActionResponse {
    pub ok: bool,
    pub message: String,
}
