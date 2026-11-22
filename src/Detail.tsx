import { TaskDatePicker } from "@/components/task-date-picker";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import { OptionCombobox } from "@/components/option-combobox";
import { useEffect, useState } from "react";
import { ArrowLeft, Star, Sun, Trash2, X, Plus, Paperclip, Download } from "lucide-react";
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
const repeatOptions = [
  { value: "", label: "不重复" },
  { value: "daily", label: "每天" },
  { value: "weekdays", label: "工作日" },
  { value: "weekly", label: "每周" },
  { value: "monthly", label: "每月" },
  { value: "yearly", label: "每年" },
];
const importedRepeatOptions = [
  ...repeatOptions,
  { value: "custom", label: "导入的自定义规则" },
];

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
  return (
    <div className="detail-content h-full overflow-auto">
      <div className="mb-4 flex items-center justify-between text-xs text-muted-foreground">
        <Button
          variant="ghost"
          size="icon-sm"
          className="hidden single-page:inline-flex"
          title="返回任务列表"
          onClick={close}
        >
          <ArrowLeft />
        </Button>
        <span className="single-page:mr-auto single-page:ml-2">{list}</span>
        <Button
          variant="ghost"
          size="icon-sm"
          className="single-page:hidden"
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
      <TaskDatePicker
        kind="due"
        value={t.due}
        onChange={(due) =>
          change((task) => {
            task.due = due;
            task.repeatAnchor = "";
          })
        }
      />
      <TaskDatePicker
        kind="reminder"
        value={t.reminder}
        onChange={(reminder) =>
          change((task) => {
            task.reminder = reminder;
          })
        }
      />
      <div className="field">
        <label htmlFor="task-repeat">重复</label>
        <OptionCombobox
          id="task-repeat"
          label="重复"
          options={
            t.repeat === "custom" ? importedRepeatOptions : repeatOptions
          }
          value={t.repeat}
          onValueChange={(repeat) =>
            change((task) => {
              task.repeat = repeat;
              task.repeatAnchor = "";
            })
          }
        />
      </div>
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
