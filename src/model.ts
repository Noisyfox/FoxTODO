import { v5 } from "uuid";
export interface Attachment {
  name: string;
  contentBytes: string;
}
export interface Pattern {
  type: string;
  interval: number;
  daysOfWeek?: string[];
  firstDayOfWeek?: string;
  dayOfMonth?: number;
  month?: number;
  index?: string;
}
export interface Recurrence {
  pattern: Pattern;
  range: {
    type: string;
    startDate: string;
    endDate?: string;
    numberOfOccurrences?: number;
  };
}
export interface Task {
  id: string;
  title: string;
  done: boolean;
  important: boolean;
  notes: string;
  steps: { id: string; title: string; done: boolean }[];
  due: string;
  reminder: string;
  myDay: string;
  repeat: string;
  repeatAnchor: string;
  attachments: Attachment[];
  categories: string[];
  imported: null | { recurrence?: Recurrence };
}
export interface List {
  version: number;
  id: string;
  name: string;
  group: string;
  tasks: Task[];
}
export interface Binding {
  drive: string;
  folder: string;
  label: string;
  clientId: string;
}
export interface Row {
  doc: List;
  revision: number;
  dirty: boolean;
  deleted: boolean;
  conflict: null | { remote: null | { doc: List } };
}
export interface View {
  records: Row[];
  binding: Binding | null;
  oneDriveAvailable: boolean;
  platform: string;
}
export const uid = () => crypto.randomUUID();
export const dateKey = (d = new Date()) =>
  [
    d.getFullYear(),
    String(d.getMonth() + 1).padStart(2, "0"),
    String(d.getDate()).padStart(2, "0"),
  ].join("-");
export const newList = (name: string): List => ({
  id: uid(),
  version: 1,
  name,
  group: "",
  tasks: [],
});
export const newTask = (title: string): Task => ({
  id: uid(),
  title,
  done: false,
  important: false,
  notes: "",
  steps: [],
  due: "",
  reminder: "",
  myDay: "",
  repeat: "",
  repeatAnchor: "",
  attachments: [],
  categories: [],
  imported: null,
});
const weekdays = [
  "sunday",
  "monday",
  "tuesday",
  "wednesday",
  "thursday",
  "friday",
  "saturday",
];
function ordinal(d: Date) {
  return Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) / 86400000;
}
function matches(d: Date, base: Date, p: Pattern) {
  const days = ordinal(d) - ordinal(base),
    months =
      (d.getFullYear() - base.getFullYear()) * 12 +
      d.getMonth() -
      base.getMonth(),
    n = Math.max(1, p.interval);
  const last = new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate();
  if (p.type === "daily") return days % n === 0;
  const allowed = (
    p.daysOfWeek?.length ? p.daysOfWeek : [weekdays[base.getDay()]]
  ).includes(weekdays[d.getDay()]);
  if (p.type === "weekly") {
    const start = weekdays.indexOf(p.firstDayOfWeek || "sunday");
    return (
      Math.floor((days + ((base.getDay() - start + 7) % 7)) / 7) % n === 0 &&
      allowed
    );
  }
  if (
    p.type.endsWith("Yearly") &&
    (d.getMonth() + 1 !== (p.month || base.getMonth() + 1) ||
      (d.getFullYear() - base.getFullYear()) % n !== 0)
  )
    return false;
  if (p.type.endsWith("Monthly") && months % n !== 0) return false;
  if (p.type.startsWith("absolute"))
    return d.getDate() === Math.min(last, p.dayOfMonth || base.getDate());
  if (!p.type.startsWith("relative") || !allowed) return false;
  const possible: number[] = [];
  for (let i = 1; i <= last; i++)
    if (
      (p.daysOfWeek || []).includes(
        weekdays[new Date(d.getFullYear(), d.getMonth(), i).getDay()],
      )
    )
      possible.push(i);
  const index =
    p.index === "last"
      ? possible.length - 1
      : ["first", "second", "third", "fourth"].indexOf(p.index || "first");
  return possible[index] === d.getDate();
}
export function complete(list: List, id: string) {
  const task = list.tasks.find((t) => t.id === id);
  if (!task) return;
  task.done = !task.done;
  if (!task.done || !task.repeat) return;
  const start = task.due || dateKey(),
    base = new Date(start + "T12:00:00"),
    next = new Date(base);
  const rule = task.repeat === "custom" ? task.imported?.recurrence : undefined;
  const simple: Record<string, string> = {
    daily: "daily",
    weekly: "weekly",
    monthly: "absoluteMonthly",
    yearly: "absoluteYearly",
    weekdays: "weekly",
  };
  const p = rule?.pattern || {
    type: simple[task.repeat],
    interval: 1,
    daysOfWeek: task.repeat === "weekdays" ? weekdays.slice(1, 6) : undefined,
  };
  if (
    !p.type ||
    (rule?.range.type === "numbered" &&
      (rule.range.numberOfOccurrences || 0) <= 1)
  )
    return;
  const anchor = rule?.range.startDate
    ? new Date(rule.range.startDate + "T12:00:00")
    : task.repeatAnchor
      ? new Date(task.repeatAnchor + "T12:00:00")
      : base;
  if (!Number.isFinite(anchor.getTime()) || !Number.isFinite(base.getTime()))
    return;
  if (
    ![
      "daily",
      "weekly",
      "absoluteMonthly",
      "relativeMonthly",
      "absoluteYearly",
      "relativeYearly",
    ].includes(p.type)
  )
    return;
  do {
    next.setDate(next.getDate() + 1);
    if (
      next.getFullYear() > 9999 ||
      (rule?.range.type === "endDate" &&
        dateKey(next) > String(rule.range.endDate))
    )
      return;
  } while (!matches(next, anchor, p));
  const date = dateKey(next);
  // A repeatable occurrence has the same identity after undo/redo of completion.
  const successor = list.tasks.find((t) => t.id === occurrenceId(id, date));
  if (successor) return;
  const t = structuredClone(task);
  t.id = occurrenceId(id, date);
  t.due = date;
  t.repeatAnchor = dateKey(anchor);
  t.done = false;
  t.myDay = "";
  t.steps.forEach((s) => {
    s.done = false;
  });
  if (t.reminder) {
    const r = new Date(t.reminder);
    r.setDate(r.getDate() + ordinal(next) - ordinal(base));
    t.reminder = r.toISOString();
  }
  const range = t.imported?.recurrence?.range;
  if (range?.type === "numbered" && range.numberOfOccurrences)
    range.numberOfOccurrences -= 1;
  list.tasks.push(t);
}
function occurrenceId(id: string, date: string) {
  return v5(date, id);
}
