import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { OptionCombobox } from "@/components/option-combobox";
import { ListIconField } from "@/components/list-icon-field";
import { useEffect, useEffectEvent, useRef, useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft,
  House,
  Sun,
  Star,
  CalendarDays,
  ListChecks,
  List as ListIcon,
  Search,
  Plus,
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
  dismissError,
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
import { ItemConflicts } from "./ItemConflicts";
import { SortArea, SortRow } from "./SortArea";
import {
  compareOrder,
  listItem,
  movedPosition,
  taskItem,
  type OrderedItem,
} from "./listOrder";
import { Sidebar, SidebarLayout, useWorkspaceViewport } from "./Sidebar";
import { hasOpenOverlay, useWorkspaceNavigation } from "./workspaceNavigation";
import { Detail } from "./Detail";
import { reminders } from "./alarms";
import "./style.css";
const navButton =
  "h-10 w-full justify-start gap-3 text-left aria-[current=page]:bg-accent aria-[current=page]:text-accent-foreground";
const sortOptions = [
  { value: "list", label: "手动排序" },
  { value: "title", label: "按名称" },
  { value: "due", label: "按截止日期" },
  { value: "star", label: "重要优先" },
];
const views: { id: string; name: string; icon: LucideIcon }[] = [
  { id: "day", name: "我的一天", icon: Sun },
  { id: "important", name: "重要", icon: Star },
  { id: "plan", name: "已计划", icon: CalendarDays },
  { id: "all", name: "全部任务", icon: ListChecks },
];
function App() {
  const v = useSyncExternalStore(subscribe, current),
    viewport = useWorkspaceViewport(),
    {
      page: { active, task, nav, search },
      show,
      closePage,
    } = useWorkspaceNavigation(viewport.singlePage, v.platform === "android"),
    [modal, setModal] = useState(""),
    [input, setInput] = useState(""),
    [emoji, setEmoji] = useState(""),
    [group, setGroup] = useState(""),
    [entry, setEntry] = useState(""),
    [done, setDone] = useState(false),
    [sort, order] = useState("list"),
    [clock, setClock] = useState(() => new Date()),
    [conflict, setConflict] = useState(""),
    [online, setOnline] = useState(navigator.onLine);
  const addRef = useRef<HTMLInputElement>(null),
    findRef = useRef<HTMLInputElement>(null),
    appMenuTrigger = useRef<HTMLButtonElement>(null),
    listMenuTrigger = useRef<HTMLButtonElement>(null),
    dialogOpener = useRef<HTMLElement | null>(null);
  function openNavigation() {
    if (viewport.singlePage && !nav) show({ nav: true }, true);
  }
  const keys = useEffectEvent((e: KeyboardEvent) => {
    if (e.defaultPrevented || hasOpenOverlay()) return;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      openNavigation();
      requestAnimationFrame(() => findRef.current?.focus());
    } else if (e.key === "Escape" && (nav || task)) {
      closePage();
    }
  });
  useEffect(() => {
    const timer = setInterval(() => setClock(new Date()), 30000);
    const net = () => setOnline(navigator.onLine);
    window.addEventListener("offline", net);
    window.addEventListener("online", net);
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
    defaultList = rows.find((r) => r.doc.id === v.defaultListId),
    customLists = rows.filter((r) => r.doc.id !== v.defaultListId),
    special = views.find((t) => t.id === active),
    currentList = rows.find((r) => r.doc.id === active),
    needle = search.trim().toLowerCase(),
    today = dateKey(clock);
  const matches = (id: string, t: Task) =>
    id === "all" ||
    (id === "day" && t.myDay === today) ||
    (id === "important" && t.important) ||
    (id === "plan" && !!(t.due || t.reminder));
  const flat = rows.flatMap((r) =>
    r.doc.tasks.map((t, index) => ({ r, t, ...taskItem(r.doc, t, index) })),
  );
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
  if (sort === "list") visible.sort(compareOrder);
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
    show({ active: id, nav: false, search: "", task: null }, nav);

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
  function dialog(m: string, opener: Element | null = document.activeElement) {
    dialogOpener.current = opener instanceof HTMLElement ? opener : null;
    setModal(m);

    setInput(m === "rename" ? currentList?.doc.name || "" : "");
    setEmoji(m === "rename" ? currentList?.doc.emoji ?? "" : "");
    setGroup(m === "rename" ? currentList?.doc.group || "" : "");
  }
  function onMenuCloseAutoFocus(event: Event) {
    if (hasOpenOverlay()) event.preventDefault();
  }
  function add() {
    if (!entry.trim() || !v.ready) return;
    const seed = currentList?.doc ?? newList("任务", v.defaultListId),
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
    : active === v.defaultListId
      ? "任务"
      : currentList?.doc.name || special?.name || "我的清单";
  const HeadIcon = needle
    ? Search
    : active === v.defaultListId
      ? House
      : special?.icon || ListIcon;
  function taskRows(items: (OrderedItem & { r: Row; t: Task })[]) {
    return (
      <SortArea
        ids={items.map((item) => item.id)}
        disabled={sort !== "list" || items.length < 2}
        onMove={(id, target) => {
          const item = items.find((row) => row.id === id);
          const position = movedPosition(items, id, target);
          if (item && position)
            editTask(item.r, item.t.id, (task) => {
              task.position = position;
            });
        }}
      >
        {items.map(card)}
      </SortArea>
    );
  }
  function card({ r, t, id }: { r: Row; t: Task; id: string }) {
    return (
      <SortRow
        id={id}
        label={t.title || "未命名任务"}
        onOpen={() => show({ task: { list: r.doc.id, id: t.id }, nav: false }, true)}
        key={id}
        className={
          "task " +
          (t.done ? "finished " : "") +
          (task?.id === t.id ? "chosen" : "")
        }
      >
        <Checkbox
          className="check size-[19px] rounded-full"
          aria-label={t.done ? "标记为未完成" : "完成任务"}
          title={t.done ? "标记为未完成" : "完成任务"}
          checked={t.done}
          onCheckedChange={() => change(r.doc.id, (d) => complete(d, t.id))}
        />
        <div className="task-text">
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
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          title={t.important ? "取消重要" : "标记重要"}
          className={"star " + (t.important ? "set" : "")}
          onClick={() =>
            editTask(r, t.id, (x) => {
              x.important = !x.important;
            })
          }
        >
          <Star size={18} />
        </Button>
      </SortRow>
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
            : v.syncFailed
              ? "同步未完成 · 点击重试"
              : v.records.some((r) => r.dirty)
                ? "已保存到本机 · 待同步"
                : "已同步";
  const statusContent = (
    <>
      {v.busy ? (
        <RefreshCw size={14} className="animate-spin" />
      ) : online ? (
        <CheckCheck size={15} />
      ) : (
        <CloudOff size={15} />
      )}
      {status}
    </>
  );
  return (
    <SidebarLayout viewport={viewport} withDetail={!!detail}>
      <Sidebar open={nav}>
        <div className="@container flex items-center gap-1 pb-7">
          <Button
            variant="ghost"
            size="icon-sm"
            className="hidden single-page:inline-flex"
            title="返回"
            onClick={closePage}
          >
            <ArrowLeft />
          </Button>
          <DropdownMenu modal={false}>
            <DropdownMenuTrigger asChild>
              <Button
                ref={appMenuTrigger}
                variant="ghost"
                className="h-11 gap-1 text-lg font-bold tracking-tight text-primary has-[>svg]:px-1 @[11rem]:gap-2 @[11rem]:text-2xl @[11rem]:has-[>svg]:px-2.5"
                aria-label="FoxTODO 菜单"
              >
                <img className="size-7 @[11rem]:size-9" src="/fox.svg" alt="" />
                FoxTODO
                <ChevronDown className="size-3.5 text-muted-foreground" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent
              align="start"
              className="data-[state=closed]:hidden"
              onCloseAutoFocus={onMenuCloseAutoFocus}
            >
              <DropdownMenuItem
                onSelect={() => dialog("cloud", appMenuTrigger.current)}
              >
                <Cloud />
                OneDrive
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <div className="search mb-5 flex items-center gap-2 rounded-md border bg-background px-2.5 text-muted-foreground focus-within:ring-2 focus-within:ring-ring/50">
          <Search size={16} />
          <Input
            className="min-w-0 flex-1 border-0 px-0 shadow-none focus-visible:ring-0"
            ref={findRef}
            placeholder="搜索任务"
            aria-label="搜索任务"
            enterKeyHint="search"
            value={search}
            onChange={(e) => {
              show({ search: e.target.value, task: null });
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && nav) show({ nav: false }, true);
            }}
          />
          {search ? (
            <Button
              variant="ghost"
              size="icon-sm"

              title="清除搜索"
              onClick={() => show({ search: "" })}
            >
              <X size={13} />
            </Button>
          ) : (
            <Kbd>{v.platform === "macos" ? "⌘ K" : "Ctrl K"}</Kbd>
          )}
        </div>
        <div className="sidebar-scroll min-h-0 overflow-y-auto">
          <nav>
            {views.map((x) => (
              <Button
                variant="ghost"
                key={x.id}
                className={navButton}
                aria-current={active === x.id && !needle ? "page" : undefined}
                onClick={() => go(x.id)}
              >
                <x.icon size={18} />
                {x.name}
                <small>
                  {flat.filter(({ t }) => !t.done && matches(x.id, t)).length ||
                    ""}
                </small>
              </Button>
            ))}
            <Button
              variant="ghost"
              disabled={!v.ready}
              className={navButton}
              aria-current={
                active === v.defaultListId && !needle ? "page" : undefined
              }
              onClick={() => go(v.defaultListId)}
            >
              <House size={18} />
              任务
              <small>
                {defaultList?.doc.tasks.filter((t) => !t.done).length || ""}
              </small>
            </Button>
          </nav>
          <div className="mt-6 mb-1 flex items-center justify-between border-t px-2 pt-3 text-xs font-semibold text-muted-foreground">
            <h3>我的清单</h3>
            <Button
              variant="ghost"
              size="icon-sm"

              title="新建清单"
              onClick={() => dialog("new")}
            >
              <Plus size={17} />
            </Button>
          </div>
          <nav>
            {Array.from(new Set(customLists.map((r) => r.doc.group)))
              .sort()
              .map((g) => {
                const members = customLists
                  .filter((r) => r.doc.group === g)
                  .map((r) => ({ r, ...listItem(r.doc) }))
                  .sort(compareOrder);
                return (
                  <div key={g}>
                    {g && <h4>{g}</h4>}
                    <SortArea
                      ids={members.map((m) => m.id)}
                      disabled={members.length < 2}
                      onMove={(id, target) => {
                        const position = movedPosition(members, id, target);
                        if (position)
                          change(id, (doc) => {
                            doc.position = position;
                          });
                      }}
                    >
                      {members.map(({ r, id }) => (
                        <SortRow
                          key={id}
                          id={id}
                          label={r.doc.name}
                          className={
                            "sidebar-list-row" +
                            (active === id ? " active" : "")
                          }
                          onOpen={() => go(id)}
                          role="button"
                        >
                          {r.doc.emoji ? (
                            <span
                              aria-hidden="true"
                              className="shrink-0 text-base leading-none"
                            >
                              {r.doc.emoji}
                            </span>
                          ) : (
                            <ListIcon size={16} />
                          )}
                          <span>{r.doc.name}</span>
                          {r.conflict ? (
                            <AlertTriangle size={14} />
                          ) : (
                            <small>
                              {r.doc.tasks.filter((t) => !t.done).length || ""}
                            </small>
                          )}
                        </SortRow>
                      ))}
                    </SortArea>
                  </div>
                );
              })}
            <Button
              variant="ghost"
              className={navButton}
              onClick={() => dialog("new")}
            >
              <Plus size={16} />
              新建清单
            </Button>
          </nav>
        </div>
      </Sidebar>
      <main
        className={
          viewport.singlePage && (nav || detail) ? "hidden" : "flex min-w-0 flex-1 flex-col"
        }
      >
        <header className="workspace-header flex h-[65px] shrink-0 items-center justify-between gap-2.5 border-b px-[clamp(18px,3.7vw,54px)] single-page:h-[54px] single-page:px-3">
          <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
            <Button
              variant="ghost"
              size="icon-sm"
              className="hidden single-page:inline-flex"
              title="打开导航"
              onClick={openNavigation}
            >
              <Menu size={20} />
            </Button>
            工作区 <span>/</span> {title}
          </div>
          {v.binding ? (
            <Button
              variant="ghost"
              className="status text-xs text-muted-foreground"
              onClick={() => void sync()}
            >
              {statusContent}
            </Button>
          ) : (
            <span className="status inline-flex items-center gap-2 px-3 text-xs text-muted-foreground">
              {statusContent}
            </span>
          )}
        </header>
        <div className="content flex-1 overflow-auto">
          <div className="mb-7 flex items-center gap-4">
            <div className="grid size-14 shrink-0 place-items-center rounded-2xl bg-orange-100 text-amber-700">
              {!needle && currentList?.doc.emoji && active !== v.defaultListId ? (
                <span aria-hidden="true" className="text-[28px] leading-none">
                  {currentList.doc.emoji}
                </span>
              ) : (
                <HeadIcon size={28} strokeWidth={1.6} />
              )}
            </div>
            <div>
              <p className="mb-1 text-xs font-medium text-muted-foreground">
                {clock.toLocaleDateString("zh-CN", {
                  month: "long",
                  day: "numeric",
                  weekday: "long",
                })}
              </p>
              <h1 className="text-[clamp(28px,3vw,38px)] leading-tight font-bold tracking-tight text-primary">
                {title}
              </h1>
            </div>
          </div>
          {(v.error || v.syncError) && (
            <div
              role="alert"
              className="my-2 flex items-center justify-between gap-2 whitespace-pre-line rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900"
            >
              {v.error || v.syncError}
              <Button
                variant="ghost"
                size="icon-sm"

                title="关闭提示"
                onClick={dismissError}
              >
                <X size={14} />
              </Button>
            </div>
          )}
          {conflicts.map((r) => (
            <Button
              variant="ghost"
              key={r.doc.id}
              className="my-2 h-auto w-full justify-start whitespace-normal border border-amber-200 bg-amber-50 text-amber-900 [&_b]:ml-auto"
              onClick={(event) => {
                dialogOpener.current = event.currentTarget;
                setConflict(r.doc.id);
              }}
            >
              <AlertTriangle size={16} />「{r.doc.name}」存在同步冲突{" "}
              <b>查看</b>
            </Button>
          ))}
          {!needle && (
            <form
              className="composer flex min-h-[53px] items-center gap-2 rounded-lg border border-input bg-card px-3 shadow-xs focus-within:ring-2 focus-within:ring-ring/50"
              onSubmit={(e) => {
                e.preventDefault();
                add();
              }}
            >
              <Plus size={21} />
              <Input
                className="min-w-0 flex-1 border-0 px-0 shadow-none focus-visible:ring-0"
                ref={addRef}
                value={entry}
                onChange={(e) => setEntry(e.target.value)}
                placeholder="添加任务…"
                aria-label="添加任务"
              />
              <Button
                size="sm"
                disabled={!entry.trim() || v.saving > 0 || !v.ready}
                title="添加任务"
                type="submit"
              >
                添加
              </Button>
            </form>
          )}
          <div className="list-head mt-7 mb-3 flex items-center justify-between [&>div]:flex [&>div]:items-center [&_h3]:font-semibold [&_h3_span]:text-xs [&_h3_span]:text-muted-foreground">
            <h3>
              待办 <span>{open.length}</span>
            </h3>
            <div>
              <div className="flex items-center gap-1 text-muted-foreground">
                <ArrowUpDown size={14} />
                <OptionCombobox
                  compact
                  label="排序"
                  options={sortOptions}
                  value={sort}
                  onValueChange={order}
                />
              </div>
              {currentList && active !== v.defaultListId && (
                <DropdownMenu modal={false}>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      ref={listMenuTrigger}
                      title="更多选项"
                      aria-label="更多选项"
                    >
                      <Ellipsis size={18} />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent
                    align="end"
                    className="data-[state=closed]:hidden"
                    onCloseAutoFocus={onMenuCloseAutoFocus}
                  >
                    <DropdownMenuItem
                      onSelect={() => dialog("rename", listMenuTrigger.current)}
                    >
                      <Pencil />
                      编辑清单
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onSelect={() => dialog("delete-list", listMenuTrigger.current)}
                      variant="destructive"
                    >
                      <Trash2 />
                      删除清单
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              )}
            </div>
          </div>
          <div className="task-list">{taskRows(open)}</div>
          {!open.length && (
            <div className="grid min-h-40 content-center justify-items-center gap-2 rounded-lg border border-dashed text-center text-muted-foreground [&_h2]:font-semibold [&_p]:text-xs">
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
              <Button
                variant="ghost"
                className="fold my-3 text-muted-foreground"
                onClick={() => setDone(!done)}
              >
                <ChevronDown
                  size={15}
                  style={{ transform: done ? "" : "rotate(-90deg)" }}
                />
                已完成 <span>{completed.length}</span>
              </Button>
              {done && taskRows(completed)}
            </>
          )}
        </div>
      </main>
      {detail && cur && (
        <Sidebar side="right" open={!nav}>
          <Detail
            key={detail.id}
            t={detail}
            list={cur.doc.name}
            change={(fn) => editTask(cur, detail.id, fn)}
            close={closePage}
            remove={() => dialog("delete-task")}
          />
        </Sidebar>
      )}
      {(modal || c?.conflict) && (
        <Dialog
          open
          onOpenChange={(open) => {
            if (!open) {
              setModal("");
              setConflict("");
            }
          }}
        >
          <DialogContent
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              const opener = dialogOpener.current;
              if (opener?.isConnected && !opener.closest("[inert]"))
                opener.focus();
            }}
            showCloseButton={false}
            aria-describedby={undefined}
            className={c?.conflict?.issues ? "max-h-[94dvh] overflow-y-auto sm:max-w-2xl" : "max-h-[94dvh] overflow-y-auto sm:max-w-md"}
            onPointerDownOutside={(event) => event.preventDefault()}
          >
            {c?.conflict?.issues && c.conflict.remote ? (
              <ItemConflicts
                key={c.doc.id + ":" + c.revision}
                view={v}
                row={c}
                cloud={c.conflict.remote.doc}
                issues={c.conflict.issues}
                close={() => setConflict("")}
              />
            ) : c?.conflict ? (
              <section className="modal-card">
                <AlertTriangle className="modal-symbol" />
                <DialogTitle>清单修改冲突</DialogTitle>
                <p>「{c.doc.name}」在两端都发生了变化。</p>
                <div className="grid gap-1 [&_p]:flex [&_p]:justify-between">
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
                      ["both", c.conflict.remote ? "保留两份" : "另存本机副本并接受删除"],
                      ["cloud", c.conflict.remote ? "使用云端版本" : "接受云端删除"],
                      ["local", "使用本机版本"],
                    ]
                ).map(([choice, text], i) => (
                  <Button
                    variant={i ? "outline" : "default"}
                    key={choice}
                    className="w-full"
                    onClick={() =>
                      void invoke("resolve", {
                        id: c.doc.id,
                        revision: c.revision,
                        choice,
                        workspace: v.binding,
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
                  </Button>
                ))}
                <Button
                  variant="ghost"
                  className="w-full text-muted-foreground"
                  onClick={() => setConflict("")}
                >
                  稍后处理
                </Button>
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
                      if (emoji.trim()) d.emoji = emoji.trim();
                      d.group = group.trim();
                      await mutate(d.id, () => {}, d);
                      go(d.id);
                    }
                    if (modal === "rename" && currentList)
                      await mutate(active, (d) => {
                        d.name = input.trim();
                        d.emoji = emoji.trim();
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
                      closePage();
                    }
                    setModal("");
                  } catch (e) {
                    report(e);
                  }
                }}
              >
                <PanelRight className="modal-symbol" />
                <DialogTitle>
                  {
                    {
                      new: "新建清单",
                      rename: "编辑清单",
                      "delete-list": "删除清单？",
                      "delete-task": "删除任务？",
                    }[modal]
                  }
                </DialogTitle>
                {["new", "rename"].includes(modal) ? (
                  <>
                    <label className="field">
                      清单名称
                      <Input
                        autoFocus
                        required
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        placeholder="例如：购物清单"
                      />
                    </label>
                    <ListIconField value={emoji} onChange={setEmoji} />
                    <label className="field">
                      分组
                      <Input
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
                <Button
                  type="submit"
                  variant={
                    modal.startsWith("delete") ? "destructive" : "default"
                  }
                  className="w-full"
                  disabled={v.saving > 0}
                >
                  {modal.startsWith("delete") ? "确认删除" : "保存清单"}
                </Button>
                <Button
                  variant="ghost"
                  type="button"
                  className="w-full text-muted-foreground"
                  onClick={() => setModal("")}
                >
                  取消
                </Button>
              </form>
            )}
          </DialogContent>
        </Dialog>
      )}
    </SidebarLayout>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
void start().catch(report);
