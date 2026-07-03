import { useEffect, useRef, useState } from "react";
import { api, onNext, toAssetUrl } from "../api";

interface Props {
  displayId: string;
}

export default function Overlay({ displayId }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [banner, setBanner] = useState<{ text: string; error: boolean } | null>({
    text: "overlay ready, waiting for config…",
    error: false,
  });

  const imagesRef = useRef<string[]>([]);
  const indexRef = useRef(0);
  const intervalRef = useRef(60);
  const timerRef = useRef<number | null>(null);

  const log = (m: string) => {
    api.overlayLog(`[${displayId}] ${m}`).catch(() => {});
  };

  useEffect(() => {
    const stopTimer = () => {
      if (timerRef.current) window.clearInterval(timerRef.current);
      timerRef.current = null;
    };

    const showAt = (i: number) => {
      const url = imagesRef.current[i];
      if (!url) return;
      setSrc(url);
    };

    const advance = () => {
      const imgs = imagesRef.current;
      if (imgs.length === 0) return;
      indexRef.current = (indexRef.current + 1) % imgs.length;
      showAt(indexRef.current);
    };

    const applyConfig = (images: string[], intervalSec: number) => {
      imagesRef.current = images.map(toAssetUrl);
      intervalRef.current = Math.max(1, intervalSec | 0);
      indexRef.current = 0;
      log(`config: ${imagesRef.current.length} images, interval=${intervalRef.current}s`);

      stopTimer();
      if (imagesRef.current.length === 0) {
        setBanner({ text: "NO IMAGES", error: true });
        setSrc(null);
        return;
      }
      setBanner({
        text: `${imagesRef.current.length} images @ ${intervalRef.current}s`,
        error: false,
      });
      showAt(0);
      if (imagesRef.current.length > 1) {
        timerRef.current = window.setInterval(advance, intervalRef.current * 1000);
      }
    };

    // Pull this window's config from the backend once mounted.
    api
      .getOverlayConfig(displayId)
      .then((cfg) => {
        if (cfg) applyConfig(cfg.images, cfg.intervalSec);
        else setBanner({ text: "NO CONFIG", error: true });
      })
      .catch((e) => setBanner({ text: `CONFIG ERROR\n${String(e)}`, error: true }));

    const unNext = onNext(() => {
      if (imagesRef.current.length < 2) return;
      advance();
      if (timerRef.current) {
        window.clearInterval(timerRef.current);
        timerRef.current = window.setInterval(advance, intervalRef.current * 1000);
      }
    });

    return () => {
      unNext.then((f) => f());
      stopTimer();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="overlay-root">
      {src && (
        <img
          className="overlay-img"
          src={src}
          alt=""
          onLoad={(e) => {
            const img = e.currentTarget;
            log(`loaded: ${img.naturalWidth}×${img.naturalHeight}`);
            // Fade the banner out on the first successful load.
            setBanner((b) => (b && !b.error ? null : b));
          }}
          onError={() => {
            log(`load FAILED: ${src}`);
            setBanner({ text: `LOAD FAIL\n${(src || "").slice(0, 80)}`, error: true });
          }}
        />
      )}
      {banner && (
        <div className={`overlay-banner ${banner.error ? "error" : ""}`}>
          {banner.text}
        </div>
      )}
    </div>
  );
}
