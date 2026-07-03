use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Physical-pixel rectangle. Regions are stored relative to a monitor's
/// top-left; bounds are absolute in the virtual desktop.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: String,
    pub label: String,
    pub bounds: Bounds,
    #[serde(rename = "scaleFactor")]
    pub scale_factor: f64,
    pub primary: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MonitorConfig {
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub region: Option<Region>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "intervalSec")]
    pub interval_sec: u32,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub monitors: HashMap<String, MonitorConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            interval_sec: 600,
            shuffle: false,
            monitors: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartPayload {
    #[serde(rename = "intervalSec")]
    pub interval_sec: u32,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub monitors: HashMap<String, MonitorConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartResult {
    pub ok: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub active: bool,
    #[serde(rename = "intervalSec")]
    pub interval_sec: Option<u32>,
    #[serde(rename = "monitorCount")]
    pub monitor_count: usize,
}

/// Config delivered to an overlay window on mount. `images` are absolute file
/// paths; the frontend converts them with `convertFileSrc`.
#[derive(Debug, Clone, Serialize)]
pub struct OverlayConfig {
    pub images: Vec<String>,
    #[serde(rename = "intervalSec")]
    pub interval_sec: u32,
}
