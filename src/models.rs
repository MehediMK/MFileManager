use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: String,
    pub created: String,
    pub permissions: String,
    pub extension: String,
    pub mime_type: String,
    pub hidden: bool,
}

#[allow(clippy::too_many_arguments)]
impl FileEntry {
    pub fn new(
        name: String,
        path: String,
        is_dir: bool,
        is_symlink: bool,
        size: u64,
        modified: String,
        created: String,
        permissions: String,
        extension: String,
        mime_type: String,
        hidden: bool,
    ) -> Self {
        Self {
            name,
            path,
            is_dir,
            is_symlink,
            size,
            modified,
            created,
            permissions,
            extension,
            mime_type,
            hidden,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub size: u64,
    pub hash: String,
    pub files: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub device: String,
    pub mount_point: String,
    pub total: u64,
    pub free: u64,
    pub used: u64,
    pub used_percent: u8,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecentFile {
    pub path: String,
    pub name: String,
    pub last_opened: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInfo {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub mime_type: String,
    pub text: Option<String>,
    pub data_url: Option<String>,
}
