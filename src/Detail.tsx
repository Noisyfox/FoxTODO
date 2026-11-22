import { useEffect, useState } from "react";
import { Star, Sun, Trash2, X, Plus, Paperclip, Download } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { dateKey, uid, type Task, type Attachment } from "./model";
import { report } from "./session";
export function Draft({
  value,
  onChange,
  multi = false,
  label,
}: {
  value: string;
  onChange: (s: string) => void;
  multi?: boolean;
  label: string;
}) {
  const [text, set] = useState(value),
    [focused, focus] = useState(false);
  useEffect(() => {
    if (!focused) set(value);
  }, [value, focused]);
  const props = {
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
  return multi ? <textarea {...props} /> : <input {...props} />;
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
    <aside className="detail">
      <div className="detail-head">
        <span>{list}</span>
        <button className="icon" onClick={close} title="关闭任务详情">
          <X size={18} />
        </button>
      </div>
      <h2>
        <Draft
          label="任务标题"
          value={t.title}
          onChange={(s) =>
            change((t) => {
              t.title = s;
            })
          }
        />
      </h2>
      <div className="chips">
        <button
          className={t.important ? "selected" : ""}
          onClick={() =>
            change((t) => {
              t.important = !t.important;
            })
          }
        >
          <Star size={15} />
          重要
        </button>
        <button
          className={t.myDay === dateKey() ? "selected" : ""}
          onClick={() =>
            change((t) => {
              t.myDay = t.myDay === dateKey() ? "" : dateKey();
            })
          }
        >
          <Sun size={15} />
          我的一天
        </button>
      </div>
      <h3>步骤</h3>
      {t.steps.map((s) => (
        <div className="step" key={s.id}>
          <input
            type="checkbox"
            checked={s.done}
            onChange={() =>
              change((t) => {
                const a = t.steps.find((x) => x.id === s.id);
                if (a) a.done = !a.done;
              })
            }
          />
          <Draft
            label="步骤名称"
            value={s.title}
            onChange={(v) =>
              change((t) => {
                const a = t.steps.find((x) => x.id === s.id);
                if (a) a.title = v;
              })
            }
          />
          <button
            title="删除步骤"
            className="icon"
            onClick={() =>
              change((t) => {
                t.steps = t.steps.filter((x) => x.id !== s.id);
              })
            }
          >
            <X size={13} />
          </button>
        </div>
      ))}
      <form
        className="inline"
        onSubmit={(e) => {
          e.preventDefault();
          if (step.trim()) {
            const s = { id: uid(), title: step.trim(), done: false };
            change((t) => t.steps.push(s));
            setStep("");
          }
        }}
      >
        <input
          aria-label="添加步骤"
          value={step}
          onChange={(e) => setStep(e.target.value)}
          placeholder="添加步骤"
        />
        <button className="icon" title="添加步骤">
          <Plus size={16} />
        </button>
      </form>
      <label className="field">
        截止日期
        <input
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
        <input
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
        <select
          value={t.repeat}
          onChange={(e) => {
            const s = e.target.value;
            change((t) => {
              t.repeat = s;
              t.repeatAnchor = "";
            });
          }}
        >
          <option value="">不重复</option>
          <option value="daily">每天</option>
          <option value="weekdays">工作日</option>
          <option value="weekly">每周</option>
          <option value="monthly">每月</option>
          <option value="yearly">每年</option>
          {t.repeat === "custom" && (
            <option value="custom">导入的自定义规则</option>
          )}
        </select>
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
          <button
            className="folder"
            key={i}
            onClick={() => void download(f).catch(report)}
          >
            <Download size={14} />
            {f.name}
          </button>
        ))}
        <label className="file-input">
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
      <button className="danger" onClick={remove}>
        <Trash2 size={15} />
        删除任务
      </button>
    </aside>
  );
}
