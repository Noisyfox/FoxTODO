import { generateKeyBetween } from "fractional-indexing";
import type { List, Task } from "./model";

// Old documents have no position. Their original list name, ID and task index
// give them a stable place without rewriting other lists on the first drag.
export interface Position {
  listName: string;
  listId: string;
  index: number;
  rank: string;
}
export interface OrderedItem {
  id: string;
  position: Position;
}
export function listItem(doc: List): OrderedItem {
  return {
    id: doc.id,
    position: doc.position ?? {
      listName: doc.name,
      listId: doc.id,
      index: 0,
      rank: "a0",
    },
  };
}
export function taskItem(doc: List, task: Task, index: number): OrderedItem {
  return {
    id: doc.id + ":" + task.id,
    position: task.position ?? {
      listName: doc.name,
      listId: doc.id,
      index,
      rank: "a0",
    },
  };
}
function compareAnchor(a: Position, b: Position) {
  return (
    a.listName.localeCompare(b.listName, "zh") ||
    a.listId.localeCompare(b.listId) ||
    a.index - b.index
  );
}
// Include identity so simultaneous moves into the same gap remain sortable.
function key(item: OrderedItem) {
  return item.position.rank + item.id.replace(/[-:]/g, "") + "V";
}
export function compareOrder(a: OrderedItem, b: OrderedItem) {
  const anchor = compareAnchor(a.position, b.position);
  if (anchor) return anchor;
  const left = key(a),
    right = key(b);
  return left < right ? -1 : left > right ? 1 : 0;
}
export function movedPosition(
  items: OrderedItem[],
  id: string,
  target: string,
): Position | undefined {
  const from = items.findIndex((item) => item.id === id);
  const to = items.findIndex((item) => item.id === target);
  if (from < 0 || to < 0 || from === to) return;
  const anchor = items[to].position;
  const neighborKey = (index: number) => {
    const item = items[index];
    return item && !compareAnchor(anchor, item.position) ? key(item) : null;
  };
  const left = neighborKey(from < to ? to : to - 1);
  const right = neighborKey(from < to ? to + 1 : to);
  let rank = generateKeyBetween(left, right);
  // Leave room for the identity suffix even when the new key prefixes the upper bound.
  if (right?.startsWith(rank)) rank = rank.padEnd(right.length, "0");
  return { ...anchor, rank };
}

// Preserve implicit positions when another edit changes their source name or array index.
export function preserveOrder(before: List, after: List) {
  const renamed = before.name !== after.name;
  if (!renamed && before.tasks.length === after.tasks.length) return;
  if (renamed && !after.position) after.position = listItem(before).position;
  const oldIndices = new Map(before.tasks.map((task, i) => [task.id, i]));
  let last: OrderedItem | undefined;
  for (const [i, task] of after.tasks.entries()) {
    const old = oldIndices.get(task.id);
    if (old === undefined) continue;
    if (!task.position && (renamed || old !== i))
      task.position = taskItem(before, before.tasks[old], old).position;
    const item = taskItem(after, task, i);
    if (!last || compareOrder(last, item) < 0) last = item;
  }
  for (const [i, task] of after.tasks.entries()) {
    if (oldIndices.has(task.id)) continue;
    const item = taskItem(after, task, i);
    if (last && compareOrder(item, last) <= 0) {
      task.position = {
        ...last.position,
        rank: generateKeyBetween(key(last), null),
      };
    }
    last = taskItem(after, task, i);
  }
}
