import { invoke } from "@tauri-apps/api/core";
import { preserveOrder } from "./listOrder";
import type { List, Row, View } from "./model";
type LocalState = View & {
  error: string;
  busy: boolean;
  saving: number;
  ready: boolean;
  saveFailed: boolean;
};
let state: LocalState = {
  defaultListId: "",
  records: [],
  binding: null,
  oneDriveAvailable: false,
  platform: "",
  error: "",
  busy: false,
  saving: 0,
  ready: false,
  saveFailed: false,
};
const listeners = new Set<() => void>();
export const subscribe = (cb: () => void) => {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
};
export const current = () => state;
function update(p: Partial<LocalState>) {
  state = { ...state, ...p };
  listeners.forEach((f) => f());
}
export const report = (e: unknown) => update({ error: String(e) });
let queue = Promise.resolve<unknown>(undefined),
  timer: ReturnType<typeof setTimeout>;
function serial<T>(f: () => Promise<T>): Promise<T> {
  const p = queue.then(f);
  queue = p.catch(report);
  return p;
}
export function refresh() {
  return serial(async () =>
    update({ ...(await invoke<View>("snapshot")), ready: true }),
  );
}
function later() {
  clearTimeout(timer);
  timer = setTimeout(() => void sync(), 950);
}
export function mutate(
  id: string,
  fn: (doc: List) => void,
  seed?: List,
  deleted = false,
) {
  const workspace = state.binding;
  update({ saving: state.saving + 1 });
  return serial(async () => {
    try {
      const r = state.records.find((r) => r.doc.id === id),
        doc = structuredClone(r?.doc ?? seed);
      if (!doc) throw Error("此清单已不存在");
      fn(doc);
      if (r) preserveOrder(r.doc, doc);
      const saved = await invoke<Row>("save_list", {
        doc,
        revision: r?.revision ?? 0,
        deleted,
        workspace,
      });
      update({
        records: [...state.records.filter((r) => r.doc.id !== id), saved],
        saveFailed: false,
      });
      later();
    } catch (e) {
      update({ saveFailed: true, ...(await invoke<View>("snapshot")) });
      throw e;
    } finally {
      update({ saving: state.saving - 1 });
    }
  });
}
let again = false;
export async function sync() {
  if (!state.binding || !navigator.onLine) return;
  if (state.busy) {
    again = true;
    return;
  }
  update({ busy: true });
  try {
    const errors = await invoke<string[]>("synchronize");
    if (errors.length) report(errors.join("\n"));
  } catch (e) {
    report(e);
  } finally {
    await refresh().catch(report);
    update({ busy: false });
    if (again) {
      again = false;
      later();
    }
  }
}
export async function start() {
  await refresh();
  void sync();
  window.addEventListener("online", sync);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) void sync();
  });
  setInterval(() => {
    if (!document.hidden) void sync();
  }, 60000);
}
