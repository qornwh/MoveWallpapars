import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { Region } from "../types";

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

// The picker window spans the monitor in physical pixels, but the webview
// reports CSS pixels. Regions are stored in physical pixels, so scale by DPR.
const toPhysical = (r: Region): Region => {
  const dpr = window.devicePixelRatio || 1;
  return {
    x: Math.round(r.x * dpr),
    y: Math.round(r.y * dpr),
    width: Math.round(r.width * dpr),
    height: Math.round(r.height * dpr),
  };
};

export default function RegionPicker() {
  const [rect, setRect] = useState<Region | null>(null);
  const startRef = useRef<{ x: number; y: number } | null>(null);
  const settledRef = useRef(false);

  const complete = (region: Region | null) => {
    if (settledRef.current) return;
    settledRef.current = true;
    api.regionPicked(region ? toPhysical(region) : null).catch(() => {});
  };

  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      startRef.current = { x: e.clientX, y: e.clientY };
    };

    const onMouseMove = (e: MouseEvent) => {
      const s = startRef.current;
      if (!s) return;
      const curX = clamp(e.clientX, 0, window.innerWidth);
      const curY = clamp(e.clientY, 0, window.innerHeight);
      setRect({
        x: Math.min(s.x, curX),
        y: Math.min(s.y, curY),
        width: Math.abs(curX - s.x),
        height: Math.abs(curY - s.y),
      });
    };

    const onMouseUp = (e: MouseEvent) => {
      const s = startRef.current;
      if (!s) return;
      startRef.current = null;
      const endX = clamp(e.clientX, 0, window.innerWidth);
      const endY = clamp(e.clientY, 0, window.innerHeight);
      const x = Math.min(s.x, endX);
      const y = Math.min(s.y, endY);
      const w = Math.abs(endX - s.x);
      const h = Math.abs(endY - s.y);
      if (w < 20 || h < 20) {
        complete(null); // too small → cancel
        return;
      }
      complete({ x, y, width: w, height: h });
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") complete(null);
    };

    const onContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      complete(null);
    };

    window.addEventListener("mousedown", onMouseDown);
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("contextmenu", onContextMenu);
    return () => {
      window.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("contextmenu", onContextMenu);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="picker-root">
      <div className="picker-hint">
        드래그해서 영역을 지정하세요 · Esc 또는 우클릭으로 취소
      </div>
      {rect && (
        <>
          <div
            className="picker-selection"
            style={{
              left: rect.x,
              top: rect.y,
              width: rect.width,
              height: rect.height,
            }}
          />
          <div
            className="picker-info"
            style={{ left: rect.x + 4, top: rect.y + rect.height + 6 }}
          >
            {(() => {
              const p = toPhysical(rect);
              return `${p.x}, ${p.y} · ${p.width} × ${p.height} px`;
            })()}
          </div>
        </>
      )}
    </div>
  );
}
