use crate::models::{Config, OverlayConfig, Region};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

#[derive(Default)]
pub struct Inner {
    pub is_running: bool,
    pub last_config: Option<Config>,
    /// overlay key (sanitized display id) -> config served to that window.
    pub overlays: HashMap<String, OverlayConfig>,
    /// Tauri window labels of live overlay windows.
    pub overlay_labels: Vec<String>,
}

#[derive(Default)]
pub struct AppState {
    pub inner: Mutex<Inner>,
    /// Pending region-picker result channel (one picker at a time).
    pub region_tx: Mutex<Option<oneshot::Sender<Option<Region>>>>,
}
