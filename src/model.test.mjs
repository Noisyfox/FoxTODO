import test from "node:test";
import assert from "node:assert/strict";
import { complete, newList, newTask } from "./model.ts";
test("repeated completion produces exactly one next occurrence even after undo", () => {
  const list = newList("Tasks"),
    task = newTask("Read");
  Object.assign(task, { due: "2026-01-31", repeat: "monthly" });
  list.tasks = [task];
  complete(list, task.id);
  assert.equal(list.tasks[1].due, "2026-02-28");
  complete(list, task.id);
  complete(list, task.id);
  assert.equal(list.tasks.length, 2);
  complete(list, list.tasks[1].id);
  assert.equal(list.tasks[2].due, "2026-03-31");
});
test("imported recurring tasks respect the date range and allowed weekdays", () => {
  const list = newList("Tasks"),
    task = newTask("Read");
  Object.assign(task, {
    due: "2026-11-20",
    repeat: "custom",
    imported: {
      recurrence: {
        pattern: {
          type: "weekly",
          interval: 1,
          daysOfWeek: ["friday", "monday"],
        },
        range: {
          type: "endDate",
          startDate: "2026-11-20",
          endDate: "2026-11-24",
        },
      },
    },
  });
  list.tasks = [task];
  complete(list, task.id);
  assert.equal(list.tasks[1].due, "2026-11-23");
  complete(list, list.tasks[1].id);
  assert.equal(list.tasks.length, 2);
});
