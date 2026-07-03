import { useCallback, useEffect, useRef, useState } from "react";
import { api, onError, onStatus } from "../api";
import type { DisplayInfo, Region, Status } from "../types";

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
  fullscreen: boolean;
  region: Region;
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

  const refreshCount = useCallback(async (id: string, folder: string | null) => {
    if (!folder) {
      setCounts((c) => ({ ...c, [id]: 0 }));
      return;
    }
    const n = await api.countImages(folder);
    setCounts((c) => ({ ...c, [id]: n }));
  }, []);

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
      for (const d of ds) refreshCount(d.id, ms[d.id].folder);

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
      setMs(id, { folder });
      refreshCount(id, folder);
    }
  };

  const clearFolder = (id: string) => {
    setMs(id, { folder: null });
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
  const start = async () => {
    const monitors: Record<string, { folder: string; region: Region | null }> = {};
    for (const [id, ms] of Object.entries(monitorState)) {
      if (!ms.folder) continue;
      monitors[id] = {
        folder: ms.folder,
        region: ms.fullscreen ? null : { ...ms.region },
      };
    }
    if (Object.keys(monitors).length === 0) {
      setStatusMsg("적어도 하나의 모니터에 폴더를 지정하세요.");
      return;
    }
    setStatusMsg("시작 중…");
    const res = await api.start({ intervalSec, shuffle, monitors });
    if (!res.active) {
      setStatusMsg("실행할 수 없습니다. 폴더에 이미지가 있는지 확인하세요.");
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
      <header>
        <h1>Move Wallpapers</h1>
        <p className="sub">
          데스크톱 아이콘 뒤에 사진을 진짜 배경화면(WorkerW)으로 띄워 주기적으로 교체합니다.
          시스템 배경화면 설정은 변경하지 않습니다. 화면비가 맞지 않는 영역은 검은 여백으로
          채워 중앙 정렬됩니다.
        </p>
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
                  {ms.folder && (
                    <button className="btn-clear" onClick={() => clearFolder(d.id)}>
                      해제
                    </button>
                  )}
                </div>
                <div className="m-body">
                  <div className="folder-info">
                    <div className={`folder-path ${ms.folder ? "" : "empty"}`}>
                      {ms.folder || "폴더 미지정"}
                    </div>
                    <div className="img-count">
                      {ms.folder ? `이미지 ${counts[d.id] ?? 0}장` : ""}
                    </div>
                  </div>
                  <button className="btn-pick" onClick={() => pickFolder(d.id)}>
                    {ms.folder ? "폴더 변경" : "폴더 선택"}
                  </button>
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
        <button className="primary" onClick={start}>
          시작
        </button>
        <button onClick={stop}>중지</button>
        <button onClick={next} disabled={!isRunning}>
          즉시 다음
        </button>
      </section>

      <section className="status">{statusMsg}</section>
    </div>
  );
}
