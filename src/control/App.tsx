import { useCallback, useEffect, useRef, useState } from "react";
import { api, onError, onStatus } from "../api";
import type { DisplayInfo, MonitorConfig, Region, Status } from "../types";

const INTERVAL_PRESETS: { value: number; label: string }[] = [
  { value: 5, label: "5초" },
  { value: 10, label: "10초" },
  { value: 15, label: "15초" },
  { value: 30, label: "30초" },
  { value: 60, label: "1분" },
  { value: 300, label: "5분" },
  { value: 600, label: "10분" },
];

interface MonitorState {
  folder: string | null;
  /** Custom image list; when set it replaces the folder. */
  files: string[] | null;
  fullscreen: boolean;
  region: Region;
}

/** Turn pasted text into a path list: one per line, tolerating trailing commas and quotes. */
function parsePathList(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim().replace(/,+$/, "").trim().replace(/^["']|["']$/g, "").trim())
    .filter((line) => line.length > 0);
}

interface ListEditorProps {
  title: string;
  initial: string[];
  onSave: (files: string[]) => void;
  onClose: () => void;
}

function ListEditor({ title, initial, onSave, onClose }: ListEditorProps) {
  const [text, setText] = useState(initial.join("\n"));
  const parsed = parsePathList(text);

  const addFiles = async () => {
    const picked = await api.pickFiles();
    if (picked.length === 0) return;
    setText((t) => (t.trim() ? `${t.replace(/\s+$/, "")}\n` : "") + picked.join("\n"));
  };

  return (
    <div className="dialog-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true">
        <h3>{title}</h3>
        <p className="dialog-help">
          이미지 경로를 한 줄에 하나씩 적거나 붙여넣으세요. 위에서부터 순서대로 표시되며, 줄 끝의
          쉼표와 따옴표는 무시됩니다. 비워서 저장하면 폴더 방식으로 돌아갑니다.
        </p>
        <textarea
          autoFocus
          spellCheck={false}
          wrap="off"
          value={text}
          placeholder={"D:\\Pictures\\image1.jpg\nD:\\Pictures\\image2.gif"}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && onClose()}
        />
        <div className="dialog-actions">
          <button onClick={addFiles}>파일 추가…</button>
          <span className="dialog-count">{parsed.length}줄</span>
          <button onClick={onClose}>취소</button>
          <button className="primary" onClick={() => onSave(parsed)}>
            저장
          </button>
        </div>
      </div>
    </div>
  );
}

export default function ControlApp() {
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [monitorState, setMonitorState] = useState<Record<string, MonitorState>>({});
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [intervalSec, setIntervalSec] = useState(600);
  const [shuffle, setShuffle] = useState(false);
  const [statusMsg, setStatusMsg] = useState("대기 중");
  const [isRunning, setIsRunning] = useState(false);

  // Cosmetic countdown state.
  const currentIntervalRef = useRef(600);
  const nextAtRef = useRef(0);
  const countdownRef = useRef<number | null>(null);

  const [editingId, setEditingId] = useState<string | null>(null);

  const refreshCount = useCallback(
    async (id: string, folder: string | null, files: string[] | null) => {
      let n = 0;
      if (files && files.length > 0) n = await api.countFiles(files);
      else if (folder) n = await api.countImages(folder);
      setCounts((c) => ({ ...c, [id]: n }));
    },
    [],
  );

  // --- init ---
  useEffect(() => {
    (async () => {
      const [ds, cfg, status] = await Promise.all([
        api.getDisplays(),
        api.getConfig(),
        api.getStatus(),
      ]);
      setDisplays(ds);

      const cfgInt = cfg.intervalSec || 600;
      if (INTERVAL_PRESETS.some((p) => p.value === cfgInt)) setIntervalSec(cfgInt);
      setShuffle(!!cfg.shuffle);

      const ms: Record<string, MonitorState> = {};
      for (const d of ds) {
        const cfgM = cfg.monitors?.[d.id] ?? {};
        ms[d.id] = {
          folder: cfgM.folder ?? null,
          files: cfgM.files && cfgM.files.length > 0 ? cfgM.files : null,
          fullscreen: !cfgM.region,
          region: cfgM.region ?? {
            x: 0,
            y: 0,
            width: d.bounds.width,
            height: d.bounds.height,
          },
        };
      }
      setMonitorState(ms);
      for (const d of ds) refreshCount(d.id, ms[d.id].folder, ms[d.id].files);

      applyStatus(status);
    })();

    const unStatus = onStatus((s) => applyStatus(s));
    const unError = onError((msg) => setStatusMsg(`오류: ${msg}`));
    return () => {
      unStatus.then((f) => f());
      unError.then((f) => f());
      stopCountdown();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function applyStatus(data: Status) {
    setIsRunning(!!data.active);
    if (data.intervalSec) currentIntervalRef.current = data.intervalSec;
    if (data.active) {
      nextAtRef.current = Date.now() + currentIntervalRef.current * 1000;
      startCountdown();
    } else {
      stopCountdown();
      setStatusMsg("중지됨");
    }
  }

  function startCountdown() {
    stopCountdown();
    tickCountdown();
    countdownRef.current = window.setInterval(tickCountdown, 1000);
  }
  function stopCountdown() {
    if (countdownRef.current) window.clearInterval(countdownRef.current);
    countdownRef.current = null;
  }
  function tickCountdown() {
    let left = Math.max(0, Math.round((nextAtRef.current - Date.now()) / 1000));
    if (left <= 0) {
      nextAtRef.current = Date.now() + currentIntervalRef.current * 1000;
      left = currentIntervalRef.current;
    }
    const m = Math.floor(left / 60);
    const s = left % 60;
    setStatusMsg(`실행 중 — 다음 변경까지 ${m}:${String(s).padStart(2, "0")}`);
  }

  // --- monitor card handlers ---
  const setMs = (id: string, patch: Partial<MonitorState>) =>
    setMonitorState((prev) => ({ ...prev, [id]: { ...prev[id], ...patch } }));

  const pickFolder = async (id: string) => {
    const folder = await api.pickFolder();
    if (folder) {
      // Choosing a folder switches back from a custom list.
      setMs(id, { folder, files: null });
      refreshCount(id, folder, null);
    }
  };

  /** Apply a custom list (empty = back to folder mode); restart if already running. */
  const applyFiles = (id: string, files: string[]) => {
    const next: Record<string, MonitorState> = {
      ...monitorState,
      [id]: { ...monitorState[id], files: files.length > 0 ? files : null },
    };
    setMonitorState(next);
    refreshCount(id, next[id].folder, next[id].files);
    if (isRunning) start(next);
  };

  const pickFiles = async (id: string) => {
    const files = await api.pickFiles();
    if (files.length > 0) applyFiles(id, files);
  };

  const clearFolder = (id: string) => {
    setMs(id, { folder: null, files: null });
    setCounts((c) => ({ ...c, [id]: 0 }));
  };

  const setRegionField = (id: string, key: keyof Region, value: number) =>
    setMonitorState((prev) => ({
      ...prev,
      [id]: { ...prev[id], region: { ...prev[id].region, [key]: value } },
    }));

  const dragRegion = async (id: string) => {
    const region = await api.pickRegion(id);
    if (!region) return; // cancelled
    setMs(id, { region, fullscreen: false });
  };

  // --- controls ---
  const start = async (state: Record<string, MonitorState> = monitorState) => {
    const monitors: Record<string, MonitorConfig> = {};
    for (const [id, ms] of Object.entries(state)) {
      if (!ms.folder && !ms.files) continue;
      monitors[id] = {
        folder: ms.folder,
        files: ms.files,
        region: ms.fullscreen ? null : { ...ms.region },
      };
    }
    if (Object.keys(monitors).length === 0) {
      setStatusMsg("모니터 하나 이상에 폴더나 이미지 목록을 지정하세요.");
      return;
    }
    setStatusMsg("시작 중…");
    const res = await api.start({ intervalSec, shuffle, monitors });
    if (!res.active) {
      setStatusMsg("표시할 이미지가 없습니다. 폴더나 목록의 경로를 확인하세요.");
    }
  };

  const stop = () => api.stop();

  const next = async () => {
    if (!isRunning) return;
    await api.next();
    nextAtRef.current = Date.now() + currentIntervalRef.current * 1000;
  };

  return (
    <div className="control-root">
      <header className="appbar">
        <div className="brand-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="4" width="18" height="16" rx="3" />
            <circle cx="8.5" cy="9.5" r="1.5" />
            <path d="M21 15.5l-4.5-4.5L8 19.5" />
          </svg>
        </div>
        <div>
          <h1>Move Wallpapers</h1>
          <p className="sub">
            바탕화면 아이콘 뒤에 사진·GIF를 배경화면처럼 띄워 주기적으로 교체합니다. 시스템
            배경화면 설정은 바꾸지 않으며, 화면비가 다른 이미지는 검은 여백으로 중앙 정렬됩니다.
          </p>
        </div>
      </header>

      <section className="settings">
        <label>
          <span>변경 주기</span>
          <select
            value={intervalSec}
            onChange={(e) => setIntervalSec(parseInt(e.target.value, 10))}
          >
            {INTERVAL_PRESETS.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
        </label>
        <label className="row">
          <input
            type="checkbox"
            checked={shuffle}
            onChange={(e) => setShuffle(e.target.checked)}
          />{" "}
          셔플
        </label>
      </section>

      <section className="monitors">
        <h2>모니터</h2>
        <div>
          {displays.map((d) => {
            const ms = monitorState[d.id];
            if (!ms) return null;
            return (
              <div className="monitor-card" key={d.id}>
                <div className="m-head">
                  <div className="m-title">
                    {d.label}
                    {d.primary && <span className="primary-tag">기본</span>}
                  </div>
                  {(ms.folder || ms.files) && (
                    <button className="btn-clear" onClick={() => clearFolder(d.id)}>
                      해제
                    </button>
                  )}
                </div>
                <div className="m-body">
                  <div className="folder-info">
                    <div className={`folder-path ${ms.folder || ms.files ? "" : "empty"}`}>
                      {ms.files
                        ? `직접 지정한 목록 · ${ms.files[0]}`
                        : ms.folder || "폴더나 이미지 목록을 지정하세요"}
                    </div>
                    <div className="img-count">
                      {ms.files
                        ? `목록 ${ms.files.length}줄 중 사용 가능한 이미지 ${counts[d.id] ?? 0}장`
                        : ms.folder
                          ? `이미지 ${counts[d.id] ?? 0}장`
                          : ""}
                    </div>
                  </div>
                </div>
                <div className="m-sources">
                  <button className="btn-pick" onClick={() => pickFolder(d.id)}>
                    폴더 선택
                  </button>
                  <button onClick={() => pickFiles(d.id)}>파일 선택</button>
                  <button onClick={() => setEditingId(d.id)}>목록 편집</button>
                </div>
                <div className="m-region">
                  <div className="region-head">
                    <label className="row">
                      <input
                        type="checkbox"
                        checked={ms.fullscreen}
                        onChange={(e) => setMs(d.id, { fullscreen: e.target.checked })}
                      />
                      <span>전체 모니터 사용</span>
                    </label>
                    <button className="btn-drag" onClick={() => dragRegion(d.id)}>
                      드래그로 영역 지정
                    </button>
                  </div>
                  {!ms.fullscreen && (
                    <div className="region-fields">
                      {(
                        [
                          ["x", "X"],
                          ["y", "Y"],
                          ["width", "너비"],
                          ["height", "높이"],
                        ] as [keyof Region, string][]
                      ).map(([key, label]) => (
                        <label key={key}>
                          {label}
                          <input
                            type="number"
                            value={ms.region[key]}
                            onChange={(e) =>
                              setRegionField(d.id, key, parseInt(e.target.value || "0", 10) || 0)
                            }
                          />
                        </label>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section className="controls">
        <button className="primary" onClick={() => start()}>
          시작
        </button>
        <button className="tonal" onClick={stop}>
          중지
        </button>
        <button onClick={next} disabled={!isRunning}>
          즉시 다음
        </button>
      </section>

      <section className={`status ${isRunning ? "running" : ""}`}>
        <span className="status-dot" />
        <span>{statusMsg}</span>
      </section>

      {editingId && monitorState[editingId] && (
        <ListEditor
          title={`이미지 목록 — ${displays.find((d) => d.id === editingId)?.label ?? ""}`}
          initial={monitorState[editingId].files ?? []}
          onClose={() => setEditingId(null)}
          onSave={(files) => {
            applyFiles(editingId, files);
            setEditingId(null);
          }}
        />
      )}
    </div>
  );
}
