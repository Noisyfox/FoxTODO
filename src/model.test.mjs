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

test("manual order survives repeated single-document moves and coincident offline positions", async () => {
  const { listItem, movedPosition, compareOrder } =
    await import("./listOrder.ts");
  const docs = Array.from({ length: 8 }, (_, i) => newList(String(i)));
  const ids = () => docs.map(listItem).sort(compareOrder);
  let expected = ids().map((i) => i.id);
  const same = movedPosition(ids(), expected[0], expected[4]);
  docs[0].position = same;
  docs[1].position = structuredClone(same);
  expected = ids().map((i) => i.id);
  for (let n = 0; n < 180; n++) {
    const from = n % 8,
      to = (n * 7 + 3) % 8;
    const [id, target] = [expected[from], expected[to]];
    docs.find((d) => d.id === id).position = movedPosition(ids(), id, target);
    expected.splice(to, 0, ...expected.splice(from, 1));
    assert.deepEqual(
      ids().map((i) => i.id),
      expected,
    );
  }
});

test("manual task order survives deleting a neighbor, renaming and appending", async () => {
  const { taskItem, movedPosition, compareOrder, preserveOrder } =
    await import("./listOrder.ts");
  const list = newList("Alpha");
  list.tasks = ["A", "B", "C"].map(newTask);
  const items = list.tasks.map((t, i) => taskItem(list, t, i));
  list.tasks[2].position = movedPosition(items, items[2].id, items[1].id);
  const edited = structuredClone(list);
  edited.tasks.shift();
  edited.name = "Beta";
  edited.tasks.push(newTask("D"));
  preserveOrder(list, edited);
  const sorted = edited.tasks
    .map((t, i) => ({ ...taskItem(edited, t, i), title: t.title }))
    .sort(compareOrder);
  assert.deepEqual(
    sorted.map((i) => i.title),
    ["C", "B", "D"],
  );
});
