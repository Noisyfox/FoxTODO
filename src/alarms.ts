import { invoke } from "@tauri-apps/api/core";
import {
  cancel,
  pending,
  requestPermission,
  isPermissionGranted,
  Schedule,
} from "@tauri-apps/plugin-notification";
import { v5 } from "uuid";
import type { View } from "./model";
let last = "",
  work = Promise.resolve();
export function reminders(v: View, onError: (e: unknown) => void) {
  const alarms = v.records
    .filter((r) => !r.deleted)
    .flatMap((r) =>
      r.doc.tasks
        .filter((t) => !t.done && Date.parse(t.reminder) > Date.now())
        .map((t) => ({
          id: v5([r.doc.id, t.id, t.reminder, t.title].join("|"), v5.URL)
            .replaceAll("-", "")
            .slice(0, 16),
          title: t.title,
          at: Date.parse(t.reminder),
        })),
    )
    .sort((a, b) => a.at - b.at || a.id.localeCompare(b.id));
  const key = JSON.stringify([v.platform, alarms]);
  if (key === last) return;
  last = key;
  work = work
    .then(async () => {
      if (key !== last) return;
      if (v.platform !== "android") {
        await invoke("schedule_desktop", { alarms });
        return;
      }
      if (
        alarms.length &&
        !(await isPermissionGranted()) &&
        (await requestPermission()) !== "granted"
      )
        throw Error("请在 Android 设置中允许通知");
      const ids = alarms.map(
          (a) => parseInt(a.id.slice(0, 8), 16) & 0x7fffffff,
        ),
        scheduled = await pending();
      const remove = scheduled
        .map((x) => x.id)
        .filter((id) => !ids.includes(id));
      if (remove.length) await cancel(remove);
      const added = alarms.flatMap((alarm, i) => {
        if (scheduled.some((n) => n.id === ids[i])) return [];
        const payload = {
          id: ids[i],
          title: "FoxTODO",
          body: alarm.title,
          schedule: Schedule.at(new Date(alarm.at), false, true),
        };
        // The Android receiver and reboot restorer read the serialized notification.
        return [{ ...payload, sourceJson: JSON.stringify(payload) }];
      });
      if (added.length)
        await invoke("plugin:notification|batch", { notifications: added });
    })
    .catch((e) => {
      last = "";
      onError(e);
    });
}
