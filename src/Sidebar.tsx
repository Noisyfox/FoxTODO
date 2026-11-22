import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

function usePanelWidth(
  key: string,
  initial: number,
  minimum: number,
  maximum: number,
) {
  const [preferred, setPreferred] = useState<number | null>(() => {
    try {
      const value = Number(localStorage.getItem(key));
      return Number.isFinite(value) && value > 0 ? value : null;
    } catch {
      return null;
    }
  });
  const [draft, setDraft] = useState<number | null>(null);
  const max = Math.max(1, maximum);
  const min = Math.min(minimum, max);
  const clamp = (n: number) => Math.max(min, Math.min(max, Math.round(n)));
  const width = clamp(draft ?? preferred ?? initial);
  function save(value: number) {
    const next = clamp(value);
    setPreferred(next);
    try {
      localStorage.setItem(key, String(next));
    } catch {
      // Resizing still works for this session if storage is unavailable.
    }
  }
  return { width, min, max, draft, setDraft, save };
}

type PanelWidth = ReturnType<typeof usePanelWidth>;
const Layout = createContext<{
  drawer: boolean;
  left: PanelWidth;
  right: PanelWidth;
} | null>(null);

export function SidebarLayout({
  withDetail,
  children,
}: {
  withDetail: boolean;
  children: ReactNode;
}) {
  const [viewport, setViewport] = useState(() => window.innerWidth);
  useEffect(() => {
    const resize = () => setViewport(window.innerWidth);
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);

  const drawer = viewport <= 720;
  const compact = viewport <= 900;
  // Docked panels share the space left after reserving the task list.
  const available = viewport - 320;
  const defaultDetail = compact ? 260 : 300;
  const reservedLeft = Math.min(160, Math.max(0, available - defaultDetail));
  const right = usePanelWidth(
    "foxtodo.detailWidth",
    drawer ? 360 : defaultDetail,
    240,
    Math.min(600, drawer ? viewport : available - reservedLeft),
  );
  const left = usePanelWidth(
    "foxtodo.sidebarWidth",
    drawer ? 250 : compact ? (withDetail ? 164 : 194) : 235,
    160,
    Math.min(
      480,
      drawer ? viewport - 40 : available - (withDetail ? right.width : 0),
    ),
  );
  return (
    <Layout.Provider value={{ drawer, left, right }}>
      <div className={"app" + (withDetail ? " with-detail" : "")}>
        {children}
      </div>
    </Layout.Provider>
  );
}

export function Sidebar({
  side = "left",
  open = true,
  children,
}: {
  side?: "left" | "right";
  open?: boolean;
  children: ReactNode;
}) {
  const layout = useContext(Layout)!;
  const { width, min, max, draft, setDraft, save } = layout[side];
  const drag = useRef<{ id: number; x: number; width: number } | null>(null);
  // Closing a pane can remove the pointer capture target mid-drag.
  useEffect(() => () => setDraft(null), [setDraft]);
  const isLeft = side === "left";
  const direction = isLeft ? 1 : -1;
  const id = isLeft ? "sidebar" : "detail";
  return (
    <aside
      id={id}
      className={id + (isLeft && open ? " shown" : "")}
      style={{ width }}
      inert={layout.drawer && !open}
    >
      {children}
      <div
        className={"sidebar-divider" + (draft !== null ? " resizing" : "")}
        role="separator"
        aria-label={isLeft ? "侧边栏宽度" : "任务详情栏宽度"}
        aria-orientation="vertical"
        aria-controls={id}
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
            setDraft(start.width + direction * (e.clientX - start.x));
        }}
        onPointerUp={(e) => {
          const start = drag.current;
          if (start?.id === e.pointerId && start.x !== e.clientX)
            save(start.width + direction * (e.clientX - start.x));
        }}
        onLostPointerCapture={() => {
          drag.current = null;
          setDraft(null);
        }}
        onKeyDown={(e) => {
          if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
          e.preventDefault();
          save(width + direction * (e.key === "ArrowRight" ? 10 : -10));
        }}
      />
    </aside>
  );
}
