import React from "react";
import ReactDOM from "react-dom/client";
import ControlApp from "./control/App";
import Overlay from "./overlay/Overlay";
import RegionPicker from "./picker/RegionPicker";
import "./styles.css";

// A single Vite entry serves three window roles. Tauri tags each window via an
// injected init script (globals) with a `?window=` query as fallback.
declare global {
  interface Window {
    __MW_ROLE__?: string;
    __MW_DISPLAY__?: string;
  }
}

const params = new URLSearchParams(window.location.search);
const role = window.__MW_ROLE__ ?? params.get("window") ?? "control";
const displayId = window.__MW_DISPLAY__ ?? params.get("display") ?? "";

function pickApp(): React.ReactElement {
  switch (role) {
    case "overlay":
      return <Overlay displayId={displayId} />;
    case "picker":
      return <RegionPicker />;
    default:
      return <ControlApp />;
  }
}

document.documentElement.dataset.window = role;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{pickApp()}</React.StrictMode>,
);
