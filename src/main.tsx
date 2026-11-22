import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import {
  Sun,
  Star,
  CalendarDays,
  ListChecks,
  List as ListIcon,
  Search,
  Plus,
  Check,
  Cloud,
  CloudOff,
  RefreshCw,
  Menu,
  X,
  ChevronDown,
  Ellipsis,
  ArrowUpDown,
  CheckCheck,
  AlertTriangle,
  PanelRight,
  ArrowUpRight,
  Calendar,
  Trash2,
  Pencil,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import {
  current,
  subscribe,
  mutate,
  refresh,
  sync,
  start,
  report,
} from "./session";
import {
  newTask,
  newList,
  dateKey,
  complete,
  type Task,
  type List,
  type Row,
} from "./model";
import { CloudPanel } from "./CloudPanel";
import { Detail } from "./Detail";
import { reminders } from "./alarms";
import "./style.css";
const views: { id: string; name: string; icon: LucideIcon }[] = [
  { id: "day", name: "我的一天", icon: Sun },
  { id: "important", name: "重要", icon: Star },
  { id: "plan", name: "已计划", icon: CalendarDays },
  { id: "all", name: "全部任务", icon: ListChecks },
];
function App() {
  const v = useSyncExternalStore(subscribe, current),
    [active, select] = useState("day"),
    [task, pick] = useState<{ list: string; id: string } | null>(null),
    [modal, setModal] = useState(""),
    [input, setInput] = useState(""),
    [group, setGroup] = useState(""),
    [entry, setEntry] = useState(""),
    [search, setSearch] = useState(""),
    [nav, setNav] = useState(false),
    [menu, setMenu] = useState(false),
    [done, setDone] = useState(false),
    [sort, order] = useState("list"),
    [clock, setClock] = useState(() => new Date()),
    [conflict, setConflict] = useState(""),
    [online, setOnline] = useState(navigator.onLine);
  const addRef = useRef<HTMLInputElement>(null),
    findRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const timer = setInterval(() => setClock(new Date()), 30000);
    const net = () => setOnline(navigator.onLine);
    window.addEventListener("offline", net);
    window.addEventListener("online", net);
    const keys = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setNav(true);
        setTimeout(() => findRef.current?.focus(), 0);
      }
      if (e.key === "Escape") {
        setNav(false);
        setMenu(false);
        setModal("");
        setConflict("");
        pick(null);
      }
    };
    window.addEventListener("keydown", keys);
    return () => {
      clearInterval(timer);
      window.removeEventListener("keydown", keys);
      window.removeEventListener("online", net);
      window.removeEventListener("offline", net);
    };
  }, []);
  useEffect(() => {
    if (v.ready) reminders(v, report);
  }, [v.records, v.ready, v.platform]);
  const rows = v.records
      .filter((r) => !r.deleted)
      .sort((a, b) => a.doc.name.localeCompare(b.doc.name, "zh")),
    special = views.find((t) => t.id === active),
    currentList = rows.find((r) => r.doc.id === active),
    needle = search.trim().toLowerCase(),
    today = dateKey(clock);
  const matches = (id: string, t: Task) =>
    id === "all" ||
    (id === "day" && t.myDay === today) ||
    (id === "important" && t.important) ||
    (id === "plan" && !!(t.due || t.reminder));
  const flat = rows.flatMap((r) => r.doc.tasks.map((t) => ({ r, t })));
  const visible = flat.filter(({ r, t }) =>
    needle
      ? [
          t.title,
          t.notes,
          r.doc.name,
          ...t.categories,
          ...t.steps.map((s) => s.title),
        ]
          .join(" ")
          .toLowerCase()
          .includes(needle)
      : r.doc.id === active || matches(active, t),
  );
  if (sort === "title")
    visible.sort((a, b) => a.t.title.localeCompare(b.t.title, "zh"));
  if (sort === "due")
    visible.sort((a, b) =>
      (a.t.due || "9999").localeCompare(b.t.due || "9999"),
    );
  if (sort === "star")
    visible.sort((a, b) => Number(b.t.important) - Number(a.t.important));
  const completed = visible.filter((x) => x.t.done),
    open = visible.filter((x) => !x.t.done),
    cur = rows.find((r) => r.doc.id === task?.list),
    detail = cur?.doc.tasks.find((t) => t.id === task?.id),
    c = v.records.find((r) => r.doc.id === conflict),
    conflicts = v.records.filter((r) => r.conflict);
  function go(id: string) {
    select(id);
    setNav(false);
    setSearch("");
    pick(null);
    setMenu(false);
    setDone(false);
  }
  function change(id: string, fn: (d: List) => void) {
    void mutate(id, fn).catch(report);
  }
  function editTask(r: Row, id: string, fn: (t: Task) => void) {
    change(r.doc.id, (d) => {
      const t = d.tasks.find((x) => x.id === id);
      if (t) fn(t);
    });
  }
  function dialog(m: string) {
    setModal(m);
    setNav(false);
    setMenu(false);
    setInput(m === "rename" ? currentList?.doc.name || "" : "");
    setGroup(m === "rename" ? currentList?.doc.group || "" : "");
  }
  function add() {
    if (!entry.trim()) return;
    const seed =
        currentList?.doc ||
        rows.find((r) => r.doc.name === "任务")?.doc ||
        newList("任务"),
      t = newTask(entry.trim());
    if (active === "day") t.myDay = today;
    if (active === "important") t.important = true;
    if (active === "plan") t.due = today;
    void mutate(seed.id, (d) => d.tasks.push(t), seed)
      .then(() => {
        setEntry((old) => (old.trim() === t.title ? "" : old));
        addRef.current?.focus();
      })
      .catch(report);
  }
  const title = needle
    ? "搜索结果"
    : currentList?.doc.name || special?.name || "我的清单";
  const HeadIcon = needle ? Search : special?.icon || ListIcon;
  function card({ r, t }: { r: Row; t: Task }) {
    return (
      <div
        key={r.doc.id + t.id}
        className={
          "task " +
          (t.done ? "finished " : "") +
          (task?.id === t.id ? "chosen" : "")
        }
      >
        <button
          className="check"
          title={t.done ? "标记为未完成" : "完成任务"}
          onClick={() => change(r.doc.id, (d) => complete(d, t.id))}
        >
          {t.done && <Check size={14} />}
        </button>
        <button
          className="task-text"
          onClick={() => pick({ list: r.doc.id, id: t.id })}
        >
          <strong>{t.title || "未命名任务"}</strong>
          <small>
            {!currentList && <span>{r.doc.name}</span>}
            {t.due && (
              <span>
                <Calendar size={12} />
                {t.due}
              </span>
            )}
            {t.steps.length > 0 && (
              <span>
                <ListChecks size={13} />
                {t.steps.filter((s) => s.done).length}/{t.steps.length}
              </span>
            )}
            {t.categories.map((s) => (
              <span key={s}>#{s}</span>
            ))}
          </small>
        </button>
        <button
          title={t.important ? "取消重要" : "标记重要"}
          className={"icon star " + (t.important ? "set" : "")}
          onClick={() =>
            editTask(r, t.id, (x) => {
              x.important = !x.important;
            })
          }
        >
          <Star size={18} />
        </button>
      </div>
    );
  }
  const status = v.saveFailed
    ? "上次本机保存失败"
    : v.saving
      ? "正在保存…"
      : v.busy
        ? "正在同步…"
        : !v.binding
          ? v.records.length
            ? "已保存到本机"
            : "本机工作区"
          : !online
            ? "离线 · 已保存到本机"
            : v.records.some((r) => r.dirty)
              ? "已保存到本机 · 待同步"
              : "已同步";
  return (
    <div className={"app " + (detail ? "with-detail" : "")}>
      {nav && (
        <button
          aria-label="关闭导航"
          className="scrim"
          onClick={() => setNav(false)}
        />
      )}
      <aside className={"sidebar " + (nav ? "shown" : "")}>
        <div className="brand">
          <img src="/fox.svg" alt="" />
          FoxTODO
        </div>
        <div className="search">
          <Search size={16} />
          <input
            ref={findRef}
            placeholder="搜索任务"
            aria-label="搜索任务"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              pick(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") setNav(false);
            }}
          />
          {search ? (
            <button
              className="icon"
              title="清除搜索"
              onClick={() => setSearch("")}
            >
              <X size={13} />
            </button>
          ) : (
            <kbd>{v.platform === "macos" ? "⌘ K" : "Ctrl K"}</kbd>
          )}
        </div>
        <div className="sidebar-scroll">
          <nav>
            {views.map((x) => (
              <button
                key={x.id}
                className={active === x.id && !needle ? "active" : ""}
                onClick={() => go(x.id)}
              >
                <x.icon size={18} />
                {x.name}
                <small>
                  {flat.filter(({ t }) => !t.done && matches(x.id, t)).length ||
                    ""}
                </small>
              </button>
            ))}
          </nav>
          <div className="lists-header">
            <h3>我的清单</h3>
            <button
              className="icon"
              title="新建清单"
              onClick={() => dialog("new")}
            >
              <Plus size={17} />
            </button>
          </div>
          <nav>
            {Array.from(new Set(rows.map((r) => r.doc.group)))
              .sort()
              .map((g) => (
                <div key={g}>
                  {g && <h4>{g}</h4>}
                  {rows
                    .filter((r) => r.doc.group === g)
                    .map((r) => (
                      <button
                        key={r.doc.id}
                        onClick={() => go(r.doc.id)}
                        className={active === r.doc.id ? "active" : ""}
                      >
                        <ListIcon size={16} />
                        <span>{r.doc.name}</span>
                        {r.conflict ? (
                          <AlertTriangle size={14} />
                        ) : (
                          <small>
                            {r.doc.tasks.filter((t) => !t.done).length || ""}
                          </small>
                        )}
                      </button>
                    ))}
                </div>
              ))}
            <button className="new-list" onClick={() => dialog("new")}>
              <Plus size={16} />
              新建清单
            </button>
          </nav>
        </div>
        <div className="account">
          <button onClick={() => dialog("cloud")}>
            <Cloud size={19} />
            <span>
              <strong>OneDrive</strong>
              <small>{v.binding?.label || "连接个人账号"}</small>
            </span>
            <ArrowUpRight size={14} />
          </button>
        </div>
      </aside>
      <main>
        <header>
          <div className="breadcrumb">
            <button
              className="mobile icon"
              title="打开导航"
              onClick={() => setNav(true)}
            >
              <Menu size={20} />
            </button>
            工作区 <span>/</span> {title}
          </div>
          <button
            className="status"
            onClick={() => (v.binding ? void sync() : dialog("cloud"))}
          >
            {v.busy ? (
              <RefreshCw size={14} className="spin" />
            ) : online ? (
              <CheckCheck size={15} />
            ) : (
              <CloudOff size={15} />
            )}{" "}
            {status}
          </button>
        </header>
        <div className="content">
          <div className="hero">
            <div className="hero-symbol">
              <HeadIcon size={28} strokeWidth={1.6} />
            </div>
            <div>
              <p className="date">
                {clock.toLocaleDateString("zh-CN", {
                  month: "long",
                  day: "numeric",
                  weekday: "long",
                })}
              </p>
              <h1>{title}</h1>
            </div>
          </div>
          {v.error && (
            <div role="alert" className="banner">
              {v.error}
              <button
                className="icon"
                title="关闭提示"
                onClick={() => report("")}
              >
                <X size={14} />
              </button>
            </div>
          )}
          {conflicts.map((r) => (
            <button
              key={r.doc.id}
              className="banner conflict"
              onClick={() => setConflict(r.doc.id)}
            >
              <AlertTriangle size={16} />「{r.doc.name}」存在同步冲突{" "}
              <b>查看</b>
            </button>
          ))}
          {!needle && (
            <form
              className="composer"
              onSubmit={(e) => {
                e.preventDefault();
                add();
              }}
            >
              <Plus size={21} />
              <input
                ref={addRef}
                value={entry}
                onChange={(e) => setEntry(e.target.value)}
                placeholder="添加任务…"
                aria-label="添加任务"
              />
              <button
                disabled={!entry.trim() || v.saving > 0 || !v.ready}
                title="添加任务"
                type="submit"
              >
                添加
              </button>
            </form>
          )}
          <div className="list-head">
            <h3>
              待办 <span>{open.length}</span>
            </h3>
            <div>
              <label className="sort">
                <ArrowUpDown size={14} />
                <select
                  aria-label="排序"
                  value={sort}
                  onChange={(e) => order(e.target.value)}
                >
                  <option value="list">默认排序</option>
                  <option value="title">按名称</option>
                  <option value="due">按截止日期</option>
                  <option value="star">重要优先</option>
                </select>
              </label>
              {currentList && (
                <button
                  title="清单选项"
                  className="icon"
                  onClick={() => setMenu(!menu)}
                >
                  <Ellipsis size={18} />
                </button>
              )}
            </div>
            {menu && (
              <div className="popover">
                <button onClick={() => dialog("rename")}>
                  <Pencil size={15} />
                  编辑清单
                </button>
                <button onClick={() => dialog("delete-list")}>
                  <Trash2 size={15} />
                  删除清单
                </button>
              </div>
            )}
          </div>
          <div className="task-list">{open.map(card)}</div>
          {!open.length && (
            <div className="empty">
              <HeadIcon size={38} strokeWidth={1.2} />
              <h2>
                {needle
                  ? "没有匹配的待办"
                  : completed.length
                    ? "待办已全部完成"
                    : "还没有待办任务"}
              </h2>
              <p>
                {needle
                  ? "试试其他关键词"
                  : active === "day"
                    ? "在上方记录今天要做的事，或从清单中加入任务。"
                    : "在上方添加你的任务。"}
              </p>
            </div>
          )}
          {!!completed.length && (
            <>
              <button className="fold" onClick={() => setDone(!done)}>
                <ChevronDown
                  size={15}
                  style={{ transform: done ? "" : "rotate(-90deg)" }}
                />
                已完成 <span>{completed.length}</span>
              </button>
              {done && completed.map(card)}
            </>
          )}
        </div>
      </main>
      {detail && cur && (
        <Detail
          key={detail.id}
          t={detail}
          list={cur.doc.name}
          change={(fn) => editTask(cur, detail.id, fn)}
          close={() => pick(null)}
          remove={() => dialog("delete-task")}
        />
      )}
      {(modal || c?.conflict) && (
        <div className="backdrop" role="presentation">
          <div
            role="dialog"
            aria-modal="true"
            aria-label={c ? "同步冲突" : "设置"}
          >
            {c?.conflict ? (
              <section className="modal-card">
                <AlertTriangle className="modal-symbol" />
                <h2>清单修改冲突</h2>
                <p>「{c.doc.name}」在两端都发生了变化。</p>
                <div className="versions">
                  <p>
                    这台设备{" "}
                    <b>
                      {c.deleted ? "已删除" : c.doc.tasks.length + " 项任务"}
                    </b>
                  </p>
                  <p>
                    OneDrive{" "}
                    <b>
                      {c.conflict.remote
                        ? c.conflict.remote.doc.tasks.length + " 项任务"
                        : "已删除"}
                    </b>
                  </p>
                </div>
                {[
                  { label: "查看本机任务", doc: c.deleted ? null : c.doc },
                  { label: "查看云端任务", doc: c.conflict.remote?.doc },
                ].map(
                  ({ label, doc }) =>
                    doc && (
                      <details key={label} className="compare">
                        <summary>{label}</summary>
                        <ul>
                          {doc.tasks.map((task) => (
                            <li key={task.id}>
                              <strong>
                                {task.done ? "✓ " : ""}
                                {task.title}
                              </strong>
                              {task.due && <small>{task.due}</small>}
                              {task.notes && <p>{task.notes}</p>}
                              {task.steps.map((s) => (
                                <small key={s.id}>
                                  {s.done ? "✓ " : "○ "}
                                  {s.title}
                                </small>
                              ))}
                            </li>
                          ))}
                        </ul>
                      </details>
                    ),
                )}
                {(c.deleted
                  ? [
                      ["cloud", "保留云端版本"],
                      ["local", "确认删除云端清单"],
                    ]
                  : [
                      ["both", "保留两份"],
                      ["cloud", "使用云端版本"],
                      ["local", "使用本机版本"],
                    ]
                ).map(([choice, text], i) => (
                  <button
                    key={choice}
                    className={i ? "secondary" : "primary"}
                    onClick={() =>
                      void invoke("resolve", {
                        id: c.doc.id,
                        revision: c.revision,
                        choice,
                      })
                        .then(async () => {
                          setConflict("");
                          await refresh();
                          void sync();
                        })
                        .catch(report)
                    }
                  >
                    {text}
                  </button>
                ))}
                <button className="text-button" onClick={() => setConflict("")}>
                  稍后处理
                </button>
              </section>
            ) : modal === "cloud" ? (
              <CloudPanel view={v} close={() => setModal("")} />
            ) : (
              <form
                className="modal-card"
                onSubmit={async (e) => {
                  e.preventDefault();
                  try {
                    if (modal === "new") {
                      const d = newList(input.trim());
                      d.group = group.trim();
                      await mutate(d.id, () => {}, d);
                      go(d.id);
                    }
                    if (modal === "rename" && currentList)
                      await mutate(active, (d) => {
                        d.name = input.trim();
                        d.group = group.trim();
                      });
                    if (modal === "delete-list" && currentList) {
                      await mutate(active, () => {}, undefined, true);
                      go("all");
                    }
                    if (modal === "delete-task" && detail && cur) {
                      await mutate(cur.doc.id, (d) => {
                        d.tasks = d.tasks.filter((t) => t.id !== detail.id);
                      });
                      pick(null);
                    }
                    setModal("");
                  } catch (e) {
                    report(e);
                  }
                }}
              >
                <PanelRight className="modal-symbol" />
                <h2>
                  {
                    {
                      new: "新建清单",
                      rename: "编辑清单",
                      "delete-list": "删除清单？",
                      "delete-task": "删除任务？",
                    }[modal]
                  }
                </h2>
                {["new", "rename"].includes(modal) ? (
                  <>
                    <label className="field">
                      清单名称
                      <input
                        autoFocus
                        required
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        placeholder="例如：购物清单"
                      />
                    </label>
                    <label className="field">
                      分组
                      <input
                        value={group}
                        onChange={(e) => setGroup(e.target.value)}
                        placeholder="可选"
                      />
                    </label>
                  </>
                ) : (
                  <p>
                    {modal === "delete-list"
                      ? currentList?.doc.name
                      : detail?.title}
                  </p>
                )}
                <button className="primary" disabled={v.saving > 0}>
                  {modal.startsWith("delete") ? "确认删除" : "保存清单"}
                </button>
                <button
                  type="button"
                  className="text-button"
                  onClick={() => setModal("")}
                >
                  取消
                </button>
              </form>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
void start().catch(report);
