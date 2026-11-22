import { useEffect, useRef, useState, type ReactNode } from "react";

const widthKey = "foxtodo.sidebarWidth";

function readWidth() {
  try {
    const value = Number(localStorage.getItem(widthKey));
    return Number.isFinite(value) && value > 0 ? value : null;
  } catch {
    return null;
  }
}

export function Sidebar({
  open,
  withDetail,
  children,
}: {
  open: boolean;
  withDetail: boolean;
  children: ReactNode;
}) {
  const [preferred, setPreferred] = useState(readWidth);
  const [draft, setDraft] = useState<number | null>(null);
  const [viewport, setViewport] = useState(() => window.innerWidth);
  const drag = useRef<{ id: number; x: number; width: number } | null>(null);
  useEffect(() => {
    const resize = () => setViewport(window.innerWidth);
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);

  const drawer = viewport <= 720;
  const compact = viewport <= 900;
  const detailWidth = withDetail ? (compact ? 260 : 300) : 0;
  // Leave room to dismiss the drawer, or to use the task pane alongside it.
  const max = Math.max(
    1,
    Math.min(480, viewport - (drawer ? 40 : 320 + detailWidth)),
  );
  const min = Math.min(160, max);
  const clamp = (n: number) => Math.max(min, Math.min(max, Math.round(n)));
  const initial = drawer ? 250 : compact ? (withDetail ? 164 : 194) : 235;
  const width = clamp(draft ?? preferred ?? initial);

  function save(value: number) {
    const next = clamp(value);
    setPreferred(next);
    try {
      localStorage.setItem(widthKey, String(next));
    } catch {
      // A WebView with storage disabled can still resize for this session.
    }
  }

  return (
    <aside
      id="sidebar"
      className={"sidebar" + (open ? " shown" : "")}
      style={{ width }}
      inert={drawer && !open}
    >
      {children}
      <div
        className={"sidebar-divider" + (draft !== null ? " resizing" : "")}
        role="separator"
        aria-label="侧边栏宽度"
        aria-orientation="vertical"
        aria-controls="sidebar"
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={width}
        aria-valuetext={width + " 像素"}
        tabIndex={0}
        onPointerDown={(e) => {
          if (e.button !== 0 || !e.isPrimary || drag.current) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          drag.current = { id: e.pointerId, x: e.clientX, width };
          setDraft(width);
        }}
        onPointerMove={(e) => {
          const start = drag.current;
          if (start?.id === e.pointerId)
            setDraft(clamp(start.width + e.clientX - start.x));
        }}
        onPointerUp={(e) => {
          const start = drag.current;
          if (start?.id === e.pointerId && start.x !== e.clientX)
            save(start.width + e.clientX - start.x);
        }}
        onLostPointerCapture={() => {
          drag.current = null;
          setDraft(null);
        }}
        onKeyDown={(e) => {
          if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
          e.preventDefault();
          save(width + (e.key === "ArrowRight" ? 10 : -10));
        }}
      />
    </aside>
  );
}
