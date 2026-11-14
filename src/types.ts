export interface Bounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DisplayInfo {
  id: string;
  label: string;
  bounds: Bounds;
  scaleFactor: number;
  primary: boolean;
}

export interface MonitorConfig {
  folder: string | null;
  /** Explicit image list; when non-empty it replaces `folder`. */
  files?: string[] | null;
  region: Region | null;
}

export interface Config {
  intervalSec: number;
  shuffle: boolean;
  monitors: Record<string, MonitorConfig>;
}

export interface StartPayload {
  intervalSec: number;
  shuffle: boolean;
  monitors: Record<string, MonitorConfig>;
}

export interface StartResult {
  ok: boolean;
  active: boolean;
}

export interface Status {
  active: boolean;
  intervalSec: number | null;
  monitorCount: number;
}

/** Payload the backend emits to each overlay window. */
export interface OverlayConfig {
  images: string[];
  intervalSec: number;
}
