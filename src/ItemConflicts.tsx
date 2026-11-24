import { useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { DialogTitle } from "@/components/ui/dialog";
import { downloadAttachment, importedRepeatOptions } from "./Detail";
import {
  compareOrder,
  listItem,
  taskItem,
  type OrderedItem,
} from "./listOrder";
import { refresh, sync } from "./session";
import type {
  ConflictIssue,
  ConflictKey,
  List,
  Row,
  Task,
  View,
} from "./model";

type Side = "local" | "cloud";

const attributes = {
  name: "清单名称",
  emoji: "清单图标",
  group: "清单分组",
  listPosition: "清单排序",
};

function heading(key: ConflictKey, local: List, cloud: List) {
  if (!("id" in key)) return attributes[key.kind];
  const task =
    local.tasks.find((t) => t.id === key.id) ??
    cloud.tasks.find((t) => t.id === key.id);
  const label =
    key.kind === "taskPosition"
      ? "任务排序"
      : key.kind === "recurrence"
        ? "重复任务"
        : "任务";
  return label + "「" + (task?.title || "无标题") + "」";
}

function PositionPreview({
  items,
  id,
}: {
  items: (OrderedItem & { title: string })[];
  id: string;
}) {
  const ordered = items.sort(compareOrder);
  const index = ordered.findIndex((t) => t.id === id);
  const item = ordered[index];
  if (!item) return <p>已删除</p>;
  return (
    <>
      <p>
        第 {index + 1} 项
        {ordered[index - 1] && "，在「" + ordered[index - 1].title + "」之后"}
        {ordered[index + 1] && "，在「" + ordered[index + 1].title + "」之前"}
      </p>
      <details>
        <summary>排序位置</summary>
        <small>
          {item.position.listName} · {item.position.index + 1} ·{" "}
          {item.position.rank}
        </small>
      </details>
    </>
  );
}

function TaskPreview({
  task: t,
  other,
  onError,
}: {
  task: Task;
  other?: Task;
  onError: (message: string) => void;
}) {
  const fields: [string, ReactNode][] = [];
  function field(key: keyof Task, label: string, value: ReactNode) {
    let different = !!value;
    if (other) {
      if (key === "attachments") {
        different =
          t.attachments.length !== other.attachments.length ||
          t.attachments.some(
            (f, i) =>
              f.name !== other.attachments[i].name ||
              f.contentBytes !== other.attachments[i].contentBytes,
          );
      } else {
        different =
          typeof t[key] === "object"
            ? JSON.stringify(t[key]) !== JSON.stringify(other[key])
            : t[key] !== other[key];
      }
    }
    if (different) fields.push([label, value || "无"]);
  }
  field("title", "标题", t.title);
  field("done", "状态", t.done ? "已完成" : "待办");
  field("important", "重要", t.important ? "已标星" : "未标星");
  field("notes", "备注", t.notes);
  field("due", "截止日期", t.due);
  field(
    "reminder",
    "提醒",
    t.reminder && new Date(t.reminder).toLocaleString(),
  );
  field("myDay", "我的一天", t.myDay);
  field(
    "repeat",
    "重复",
    importedRepeatOptions.find((o) => o.value === t.repeat)?.label ?? t.repeat,
  );
  field("repeatAnchor", "重复起始日期", t.repeatAnchor);
  if (other && t.occurrenceOf !== other.occurrenceOf)
    field("occurrenceOf", "上一期任务 ID", t.occurrenceOf);
  field("categories", "分类", t.categories.join("、"));
  field(
    "steps",
    "步骤",
    t.steps.length > 0 && (
      <ol>
        {t.steps.map((s) => (
          <li key={s.id}>
            {s.done ? "✓" : "○"} {s.title}
          </li>
        ))}
      </ol>
    ),
  );
  field(
    "attachments",
    "附件",
    t.attachments.length > 0 &&
      t.attachments.map((f, i) => (
        <button
          key={i}
          type="button"
          className="block text-left text-primary underline"
          onClick={() =>
            void downloadAttachment(f).catch((e) => onError(String(e)))
          }
        >
          保存 {f.name}
        </button>
      )),
  );
  if (t.imported || other?.imported)
    field(
      "imported",
      "导入信息",
      t.imported && (
        <details>
          <summary>查看原始数据</summary>
          <pre className="whitespace-pre-wrap">
            {JSON.stringify(t.imported, null, 2)}
          </pre>
        </details>
      ),
    );
  return (
    <dl className="grid gap-1.5">
      {fields.map(([label, value]) => (
        <div key={label}>
          <dt className="text-xs text-muted-foreground">{label}</dt>
          <dd className="whitespace-pre-wrap">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function Preview({
  issue,
  doc,
  other,
  view,
  onError,
}: {
  issue: ConflictIssue;
  doc: List;
  other: List;
  view: View;
  onError: (message: string) => void;
}) {
  const { key } = issue;
  const docs = view.records
    .filter((r) => !r.deleted)
    .map((r) => (r.doc.id === doc.id ? doc : r.doc));
  switch (key.kind) {
    case "name":
      return <p>{doc.name}</p>;
    case "emoji":
      return <p>{doc.emoji || "无图标"}</p>;
    case "group":
      return <p>{doc.group || "未分组"}</p>;
    case "listPosition":
      return (
        <PositionPreview
          id={doc.id}
          items={docs
            .filter((d) => d.id !== view.defaultListId && d.group === doc.group)
            .map((d) => ({ ...listItem(d), title: d.name }))}
        />
      );
    case "taskPosition": {
      const selected = doc.tasks.find((t) => t.id === key.id);
      const items = docs.flatMap((d) =>
        d.tasks.map((t, i) => ({
          ...taskItem(d, t, i),
          title: t.title,
          done: t.done,
        })),
      );
      return (
        <PositionPreview
          id={doc.id + ":" + key.id}
          items={items.filter((t) => t.done === selected?.done)}
        />
      );
    }
    case "task":
    case "recurrence": {
      const t = doc.tasks.find((t) => t.id === key.id);
      const counterpart = other.tasks.find((t) => t.id === key.id);
      const children = doc.tasks.filter((t) => issue.members?.includes(t.id));
      return (
        <>
          {t ? (
            <TaskPreview task={t} other={counterpart} onError={onError} />
          ) : (
            <p>已删除</p>
          )}
          {key.kind === "task" &&
            t &&
            (!counterpart ||
              JSON.stringify(t.position) !==
                JSON.stringify(counterpart.position)) && (
              <details>
                <summary>查看此版本的排序</summary>
                <PositionPreview
                  id={doc.id + ":" + t.id}
                  items={doc.tasks.map((item, i) => ({
                    ...taskItem(doc, item, i),
                    title: item.title,
                  }))}
                />
              </details>
            )}
          {key.kind === "recurrence" && (
            <div>
              <p className="text-xs text-muted-foreground">后续任务</p>
              {children.length === 0
                ? "无"
                : children.map((child) => (
                    <details key={child.id}>
                      <summary>
                        {child.due || "无截止日期"} · {child.title}
                      </summary>
                      <TaskPreview task={child} onError={onError} />
                    </details>
                  ))}
            </div>
          )}
        </>
      );
    }
  }
}

export function ItemConflicts({
  view,
  row,
  cloud,
  issues,
  close,
}: {
  view: View;
  row: Row;
  cloud: List;
  issues: ConflictIssue[];
  close: () => void;
}) {
  const [answers, setAnswers] = useState<Record<string, Side>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function apply(keepBoth = false) {
    setBusy(true);
    try {
      const record = {
        id: row.doc.id,
        revision: row.revision,
        workspace: view.binding,
      };
      if (keepBoth) {
        await invoke("resolve", { ...record, choice: "both" });
      } else {
        await invoke("resolve_items", {
          ...record,
          decisions: issues.map(({ key }) => ({
            key,
            side: answers[JSON.stringify(key)],
          })),
        });
      }
      await refresh();
      close();
      void sync();
    } catch (e) {
      setError(String(e));
      await refresh().catch((err) => setError(String(err)));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="modal-card">
      <DialogTitle>合并清单修改</DialogTitle>
      <p>
        「{row.doc.name}」
        {issues.length
          ? "有 " + issues.length + " 项需要选择，其他修改会自动保留。"
          : "的所有修改均可自动合并。"}
      </p>
      {issues.map((issue) => {
        const id = JSON.stringify(issue.key);
        return (
          <fieldset
            key={id}
            className="min-w-0 space-y-2 border-t pt-2"
            disabled={busy}
          >
            <legend className="pr-2 font-medium">
              {heading(issue.key, row.doc, cloud)}
            </legend>
            <div className="grid gap-2 sm:grid-cols-2">
              {(["local", "cloud"] as const).map((side) => (
                <div
                  key={side}
                  className="min-w-0 rounded-md border p-3 text-sm [overflow-wrap:anywhere] has-checked:border-primary"
                >
                  <label className="mb-2 flex cursor-pointer items-center gap-2 font-medium">
                    <input
                      type="radio"
                      name={id}
                      value={side}
                      checked={answers[id] === side}
                      onChange={() => setAnswers((a) => ({ ...a, [id]: side }))}
                    />
                    {side === "local" ? "这台设备" : "OneDrive"}
                  </label>
                  <Preview
                    issue={issue}
                    doc={side === "local" ? row.doc : cloud}
                    other={side === "local" ? cloud : row.doc}
                    view={view}
                    onError={setError}
                  />
                </div>
              ))}
            </div>
          </fieldset>
        );
      })}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="flex flex-wrap justify-end gap-2">
        {issues.length > 0 && (
          <Button
            variant="outline"
            className="mr-auto"
            disabled={busy}
            onClick={() => void apply(true)}
          >
            保留两份清单
          </Button>
        )}
        <Button variant="ghost" disabled={busy} onClick={close}>
          稍后处理
        </Button>
        <Button
          disabled={
            busy || issues.some(({ key }) => !answers[JSON.stringify(key)])
          }
          onClick={() => void apply()}
        >
          应用合并
        </Button>
      </div>
    </section>
  );
}
