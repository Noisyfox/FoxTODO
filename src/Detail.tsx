import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { useEffect, useState } from "react";
import { Star, Sun, Trash2, X, Plus, Paperclip, Download } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { dateKey, uid, type Task, type Attachment } from "./model";
import { arrayMove } from "@dnd-kit/sortable";
import { SortArea, SortRow } from "./SortArea";
import { report } from "./session";
export function Draft({
  value,
  onChange,
  multi = false,
  className,
  label,
}: {
  value: string;
  onChange: (s: string) => void;
  multi?: boolean;
  className?: string;
  label: string;
}) {
  const [text, set] = useState(value),
    [focused, focus] = useState(false);
  useEffect(() => {
    if (!focused) set(value);
  }, [value, focused]);
  const props = {
    className,
    "aria-label": label,
    placeholder: label,
    value: text,
    onFocus: () => focus(true),
    onBlur: () => focus(false),
    onChange: (e: { target: { value: string } }) => {
      set(e.target.value);
      onChange(e.target.value);
    },
  };
  return multi ? <Textarea {...props} /> : <Input {...props} />;
}
export function Detail({
  t,
  list,
  change,
  close,
  remove,
}: {
  t: Task;
  list: string;
  change: (f: (t: Task) => void) => void;
  close: () => void;
  remove: () => void;
}) {
  const [step, setStep] = useState("");
  const files = t.attachments;
  async function download(a: Attachment) {
    const path = await save({ defaultPath: a.name.replace(/[/\\]/g, "_") });
    if (path)
      await writeFile(
        path,
        Uint8Array.from(atob(a.contentBytes), (c) => c.charCodeAt(0)),
      );
  }
  const localTime = t.reminder
    ? new Date(
        Date.parse(t.reminder) -
          new Date(t.reminder).getTimezoneOffset() * 60000,
      )
        .toISOString()
        .slice(0, 16)
    : "";
  return (
    <div className="detail-content h-full overflow-auto">
      <div className="mb-4 flex items-center justify-between text-xs text-muted-foreground">
        <span>{list}</span>
        <Button
          variant="ghost"
          size="icon-sm"

          onClick={close}
          title="关闭任务详情"
        >
          <X size={18} />
        </Button>
      </div>
      <h2>
        <Draft
          className="border-0 px-0 text-xl font-semibold shadow-none md:text-xl"
          label="任务标题"
          value={t.title}
          onChange={(s) =>
            change((t) => {
              t.title = s;
            })
          }
        />
      </h2>
      <div className="my-4 flex flex-wrap gap-1.5 [&_[aria-pressed=true]]:bg-accent">
        <Button
          variant="outline"
          size="sm"
          aria-pressed={t.important}
          onClick={() =>
            change((t) => {
              t.important = !t.important;
            })
          }
        >
          <Star size={15} />
          重要
        </Button>
        <Button
          variant="outline"
          size="sm"
          aria-pressed={t.myDay === dateKey()}
          onClick={() =>
            change((t) => {
              t.myDay = t.myDay === dateKey() ? "" : dateKey();
            })
          }
        >
          <Sun size={15} />
          我的一天
        </Button>
      </div>
      <h3>步骤</h3>
      <SortArea
        ids={t.steps.map((s) => s.id)}
        disabled={t.steps.length < 2}
        onMove={(id, target) =>
          change((task) => {
            const from = task.steps.findIndex((step) => step.id === id);
            const to = task.steps.findIndex((step) => step.id === target);
            if (from >= 0 && to >= 0)
              task.steps = arrayMove(task.steps, from, to);
          })
        }
      >
        {t.steps.map((s) => (
          <SortRow
            className="step my-2 flex min-h-10 items-center gap-1.5 py-1"
            key={s.id}
            id={s.id}
            label={s.title || "步骤"}
          >
            <Checkbox
              aria-label={s.done ? "标记步骤为未完成" : "完成步骤"}
              checked={s.done}
              onCheckedChange={() =>
                change((t) => {
                  const a = t.steps.find((x) => x.id === s.id);
                  if (a) a.done = !a.done;
                })
              }
            />
            <Draft
              className="w-auto min-w-[6ch] max-w-[calc(100%-6rem)] border-0 px-1 shadow-none field-sizing-content"
              label="步骤名称"
              value={s.title}
              onChange={(v) =>
                change((t) => {
                  const a = t.steps.find((x) => x.id === s.id);
                  if (a) a.title = v;
                })
              }
            />
            <Button
              variant="ghost"
              size="icon-sm"
              title="删除步骤"
              className="ml-auto"
              onClick={() =>
                change((t) => {
                  t.steps = t.steps.filter((x) => x.id !== s.id);
                })
              }
            >
              <X size={13} />
            </Button>
          </SortRow>
        ))}
      </SortArea>
      <form
        className="my-2 flex items-center gap-1.5"
        onSubmit={(e) => {
          e.preventDefault();
          if (step.trim()) {
            const s = { id: uid(), title: step.trim(), done: false };
            change((t) => t.steps.push(s));
            setStep("");
          }
        }}
      >
        <Input
          aria-label="添加步骤"
          value={step}
          onChange={(e) => setStep(e.target.value)}
          placeholder="添加步骤"
        />
        <Button
          variant="ghost"
          size="icon-sm"

          title="添加步骤"
        >
          <Plus size={16} />
        </Button>
      </form>
      <label className="field">
        截止日期
        <Input
          type="date"
          value={t.due}
          onChange={(e) => {
            const s = e.target.value;
            change((t) => {
              t.due = s;
              t.repeatAnchor = "";
            });
          }}
        />
      </label>
      <label className="field">
        提醒
        <Input
          type="datetime-local"
          value={localTime}
          onChange={(e) => {
            const s = e.target.value;
            change((t) => {
              t.reminder = s ? new Date(s).toISOString() : "";
            });
          }}
        />
      </label>
      <label className="field">
        重复
        <NativeSelect
          value={t.repeat}
          onChange={(e) => {
            const s = e.target.value;
            change((t) => {
              t.repeat = s;
              t.repeatAnchor = "";
            });
          }}
        >
          <NativeSelectOption value="">不重复</NativeSelectOption>
          <NativeSelectOption value="daily">每天</NativeSelectOption>
          <NativeSelectOption value="weekdays">工作日</NativeSelectOption>
          <NativeSelectOption value="weekly">每周</NativeSelectOption>
          <NativeSelectOption value="monthly">每月</NativeSelectOption>
          <NativeSelectOption value="yearly">每年</NativeSelectOption>
          {t.repeat === "custom" && (
            <NativeSelectOption value="custom">
              导入的自定义规则
            </NativeSelectOption>
          )}
        </NativeSelect>
      </label>
      <label className="field">
        分类
        <Draft
          value={t.categories.join(", ")}
          label="用逗号分隔分类"
          onChange={(s) =>
            change((t) => {
              t.categories = s
                .split(/[,，]/)
                .map((x) => x.trim())
                .filter(Boolean);
            })
          }
        />
      </label>
      <label className="field">
        备注
        <Draft
          multi
          value={t.notes}
          label="添加备注"
          onChange={(s) =>
            change((t) => {
              t.notes = s;
            })
          }
        />
      </label>
      <div className="field">
        附件
        {files.map((f, i) => (
          <Button
            variant="ghost"
            className="w-full justify-start overflow-hidden"
            key={i}
            onClick={() => void download(f).catch(report)}
          >
            <Download size={14} />
            {f.name}
          </Button>
        ))}
        <label className="flex w-fit cursor-pointer items-center gap-2 rounded-md border px-3 py-2 text-sm [&_input]:hidden">
          <Paperclip size={14} />
          添加附件
          <input
            type="file"
            onChange={async (e) => {
              const f = e.target.files?.[0];
              if (!f) return;
              const r = new FileReader();
              r.onload = () =>
                change((t) => {
                  t.attachments.push({
                    name: f.name,
                    contentBytes: String(r.result).split(",")[1],
                  });
                });
              r.onerror = () => report("文件读取失败");
              r.readAsDataURL(f);
            }}
          />
        </label>
      </div>
      <Button
        variant="ghost"
        className="mt-3 text-destructive hover:text-destructive"
        onClick={remove}
      >
        <Trash2 size={15} />
        删除任务
      </Button>
    </div>
  );
}
