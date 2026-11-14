import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Config,
  DisplayInfo,
  OverlayConfig,
  Region,
  StartPayload,
  StartResult,
  Status,
} from "./types";

// --- Commands (mirror the old Electron IPC handlers) ---

export const api = {
  getDisplays: () => invoke<DisplayInfo[]>("get_displays"),
  getConfig: () => invoke<Config>("get_config"),
  getStatus: () => invoke<Status>("get_status"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  countImages: (folder: string) => invoke<number>("count_images", { folder }),
  pickFiles: () => invoke<string[]>("pick_files"),
  countFiles: (files: string[]) => invoke<number>("count_files", { files }),
  pickRegion: (displayId: string) =>
    invoke<Region | null>("pick_region", { displayId }),
  regionPicked: (region: Region | null) =>
    invoke<void>("region_picked", { region }),
  start: (payload: StartPayload) => invoke<StartResult>("start", { payload }),
  stop: () => invoke<{ ok: boolean }>("stop"),
  next: () => invoke<{ ok: boolean }>("next"),
  overlayLog: (message: string) => invoke<void>("overlay_log", { message }),
  getOverlayConfig: (display: string) =>
    invoke<OverlayConfig | null>("get_overlay_config", { display }),
};

/** Convert an absolute file path into a webview-loadable asset URL. */
export const toAssetUrl = (path: string): string => convertFileSrc(path);

// --- Event subscriptions ---

export const onStatus = (cb: (s: Status) => void): Promise<UnlistenFn> =>
  listen<Status>("status", (e) => cb(e.payload));

export const onError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
  listen<{ message: string }>("error", (e) => cb(e.payload.message));

export const onNext = (cb: () => void): Promise<UnlistenFn> =>
  listen("next", () => cb());
