use crate::store::{Document, Position, Task};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Key {
    Name,
    Emoji,
    Group,
    ListPosition,
    Task(Uuid),
    TaskPosition(Uuid),
    Recurrence(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Local,
    Cloud,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Issue {
    pub key: Key,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<Uuid>,
}

pub struct Merge {
    pub doc: Document,
    pub issues: Vec<Issue>,
}

struct Choices<F> {
    choose: F,
    issues: Vec<Issue>,
}

impl<F: Fn(Key) -> Side> Choices<F> {
    fn conflict(&mut self, key: Key, members: Vec<Uuid>) -> Side {
        self.issues.push(Issue { key, members });
        (self.choose)(key)
    }

    fn value<T: PartialEq>(&mut self, key: Key, base: T, local: T, cloud: T) -> T {
        if local == base {
            return cloud;
        }
        if cloud == base || cloud == local {
            return local;
        }
        match self.conflict(key, vec![]) {
            Side::Local => local,
            Side::Cloud => cloud,
        }
    }
}

fn anchor(doc: &Document, index: usize) -> Position {
    Position {
        list_name: doc.name.clone(),
        list_id: doc.id,
        index,
        rank: "a0".into(),
    }
}

#[derive(Clone, PartialEq)]
struct PlacedTask {
    content: Task,
    position: Position,
}

type Tasks = BTreeMap<Uuid, PlacedTask>;
type Children = BTreeMap<Uuid, BTreeSet<Uuid>>;

fn tasks(doc: &Document) -> Tasks {
    doc.tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut content = t.clone();
            let position = content.position.take().unwrap_or_else(|| anchor(doc, i));
            (t.id, PlacedTask { content, position })
        })
        .collect()
}

// Old clients identify a successor only by UUIDv5(parent, due date). New clients
// also retain the parent when a user reschedules the successor.
fn children(tasks: &mut Tasks, base: &Tasks) -> Children {
    let parents: BTreeSet<_> = base
        .values()
        .chain(tasks.values())
        .filter(|t| !t.content.repeat.is_empty())
        .map(|t| t.content.id)
        .collect();
    let mut children = Children::new();
    for (&id, task) in tasks.iter_mut().filter(|(id, _)| !base.contains_key(id)) {
        let t = &mut task.content;
        if t.occurrence_of.is_none() && t.id.get_version_num() == 5 && !t.due.is_empty() {
            t.occurrence_of = parents
                .iter()
                .copied()
                .find(|p| Uuid::new_v5(p, t.due.as_bytes()) == id);
        }
        if let Some(parent) = t.occurrence_of {
            children.entry(parent).or_default().insert(id);
        }
    }
    children
}

fn descendants(parent: Uuid, local: &Children, cloud: &Children) -> BTreeSet<Uuid> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![parent];
    while let Some(id) = queue.pop() {
        for child in local.get(&id).into_iter().chain(cloud.get(&id)).flatten() {
            if *child != parent && seen.insert(*child) {
                queue.push(*child);
            }
        }
    }
    seen
}

fn recurrence_choices<F: Fn(Key) -> Side>(
    base: &Tasks,
    local: &mut Tasks,
    cloud: &mut Tasks,
    choices: &mut Choices<F>,
) -> BTreeMap<Uuid, Side> {
    let lc = children(local, base);
    let rc = children(cloud, base);
    let parents: BTreeSet<_> = lc.keys().chain(rc.keys()).copied().collect();
    let mut groups = Vec::new();
    let mut nested = BTreeSet::new();
    for id in parents {
        let b = base.get(&id).map(|t| &t.content);
        let l = local.get(&id).map(|t| &t.content);
        let r = cloud.get(&id).map(|t| &t.content);
        let different_dates = lc.contains_key(&id) && rc.contains_key(&id) && lc[&id] != rc[&id];
        if different_dates || (l != b && r != b && l != r) {
            let members = descendants(id, &lc, &rc);
            nested.extend(members.iter().copied());
            groups.push((id, members));
        }
    }
    let mut sides = BTreeMap::new();
    for (id, members) in groups {
        if !nested.contains(&id) {
            let side = choices.conflict(Key::Recurrence(id), members.iter().copied().collect());
            sides.insert(id, side);
            sides.extend(members.into_iter().map(|child| (child, side)));
        }
    }
    sides
}

fn merge_task<F: Fn(Key) -> Side>(
    id: Uuid,
    base: Option<&PlacedTask>,
    local: Option<&PlacedTask>,
    cloud: Option<&PlacedTask>,
    family: Option<Side>,
    choices: &mut Choices<F>,
) -> Option<PlacedTask> {
    // Positions and content are independent for tasks present on all sides.
    if let (Some(b), Some(l), Some(r)) = (base, local, cloud) {
        let content = match family {
            Some(Side::Local) => &l.content,
            Some(Side::Cloud) => &r.content,
            None => choices.value(Key::Task(id), &b.content, &l.content, &r.content),
        };
        let position = choices.value(Key::TaskPosition(id), &b.position, &l.position, &r.position);
        return Some(PlacedTask {
            content: content.clone(),
            position: position.clone(),
        });
    }
    match family {
        Some(Side::Local) => local,
        Some(Side::Cloud) => cloud,
        None => choices.value(Key::Task(id), base, local, cloud),
    }
    .cloned()
}

pub fn lists_equivalent(a: &Document, b: &Document) -> bool {
    a == b
        || (a.id == b.id
            && a.version == b.version
            && a.name == b.name
            && a.emoji == b.emoji
            && a.group == b.group
            && a.default_imports == b.default_imports
            && a.position.clone().unwrap_or_else(|| anchor(a, 0))
                == b.position.clone().unwrap_or_else(|| anchor(b, 0))
            && tasks(a) == tasks(b))
}

pub fn merge(
    base: &Document,
    local: &Document,
    cloud: &Document,
    choose: impl Fn(Key) -> Side,
) -> Merge {
    let mut choices = Choices {
        choose,
        issues: vec![],
    };
    let name = choices.value(Key::Name, &base.name, &local.name, &cloud.name);
    let mut doc = Document::new(base.id, name.clone());
    doc.emoji = choices
        .value(Key::Emoji, &base.emoji, &local.emoji, &cloud.emoji)
        .clone();
    doc.group = choices
        .value(Key::Group, &base.group, &local.group, &cloud.group)
        .clone();
    let pos = |d: &Document| d.position.clone().unwrap_or_else(|| anchor(d, 0));
    let position = choices.value(Key::ListPosition, pos(base), pos(local), pos(cloud));
    if position != anchor(&doc, 0) {
        doc.position = Some(position);
    }
    // The import ledger is append-only. Preserve markers from either device.
    doc.default_imports = local
        .default_imports
        .iter()
        .chain(&cloud.default_imports)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let b = tasks(base);
    let mut l = tasks(local);
    let mut r = tasks(cloud);
    // Concurrent imports and recurring completions can insert the same ID at
    // different array indices. Neither device explicitly dragged that task.
    let implicit_local: BTreeSet<_> = local
        .tasks
        .iter()
        .filter(|t| t.position.is_none() && !b.contains_key(&t.id))
        .map(|t| t.id)
        .collect();
    for t in cloud
        .tasks
        .iter()
        .filter(|t| t.position.is_none() && implicit_local.contains(&t.id))
    {
        let left = l.get_mut(&t.id).unwrap();
        let right = r.get_mut(&t.id).unwrap();
        let pos = (&left.position).min(&right.position).clone();
        left.position = pos.clone();
        right.position = pos;
    }
    let families = recurrence_choices(&b, &mut l, &mut r, &mut choices);
    let added: BTreeSet<_> = l
        .keys()
        .chain(r.keys())
        .filter(|id| !b.contains_key(id))
        .copied()
        .collect();
    // Preserve base array order and make concurrent insertions deterministic.
    // The UI uses position + ID, independent of this serialization order.
    for id in base.tasks.iter().map(|t| t.id).chain(added) {
        if let Some(mut task) = merge_task(
            id,
            b.get(&id),
            l.get(&id),
            r.get(&id),
            families.get(&id).copied(),
            &mut choices,
        ) {
            if task.position != anchor(&doc, doc.tasks.len()) {
                task.content.position = Some(task.position);
            }
            doc.tasks.push(task.content);
        }
    }
    Merge {
        doc,
        issues: choices.issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(names: &[&str]) -> Document {
        let mut doc = Document::new(Uuid::new_v4(), "清单".into());
        for &name in names {
            doc.tasks.push(Task {
                title: name.into(),
                ..Default::default()
            });
        }
        doc
    }
    fn automatic(base: &Document, local: &Document, cloud: &Document) -> Document {
        let result = merge(base, local, cloud, |_| panic!("unexpected conflict"));
        assert!(result.issues.is_empty());
        assert_eq!(
            result.doc,
            merge(base, cloud, local, |_| panic!("asymmetric conflict")).doc
        );
        result.doc.validate(base.id).unwrap();
        result.doc
    }
    #[test]
    fn disjoint_contents_insertions_deletions_and_same_named_tasks() {
        let base = list(&["A", "B", "C", "D"]);
        let mut l = base.clone();
        let mut r = base.clone();
        l.tasks[0].notes = "本机".into();
        r.tasks[1].done = true;
        let c = base.tasks[2].id;
        l.tasks.remove(2);
        l.tasks[2].position = Some(anchor(&base, 3));
        r.tasks.pop();
        l.tasks.push(Task {
            title: "新任务".into(),
            ..Default::default()
        });
        r.tasks.push(Task {
            title: "新任务".into(),
            ..Default::default()
        });
        let doc = automatic(&base, &l, &r);
        assert_eq!(doc.tasks.len(), 4);
        assert_eq!(doc.tasks[0].notes, "本机");
        assert!(doc.tasks[1].done);
        assert!(!doc.tasks.iter().any(|t| t.id == c || t.title == "D"));
        assert_eq!(doc.tasks.iter().filter(|t| t.title == "新任务").count(), 2);
    }
    #[test]
    fn independent_content_and_order_survive_conflict_choices() {
        let base = list(&["A", "B", "C"]);
        let (mut l, mut r) = (base.clone(), base.clone());
        l.name = "新名字".into();
        l.position = Some(anchor(&base, 0));
        for (i, t) in l.tasks.iter_mut().enumerate() {
            t.position = Some(anchor(&base, i));
        }
        r.emoji = Some("🐺".into());
        r.tasks[1].notes = "保留".into();
        l.tasks[0].due = "2026-11-30".into();
        r.tasks[0].important = true;
        r.tasks[0].position = Some(anchor(&base, 8));
        l.tasks.pop();
        for side in [Side::Local, Side::Cloud] {
            let result = merge(&base, &l, &r, |_| side);
            assert_eq!(
                result.issues,
                vec![Issue {
                    key: Key::Task(base.tasks[0].id),
                    members: vec![]
                }]
            );
            let doc = result.doc;
            assert_eq!(doc.name, l.name);
            assert_eq!(doc.emoji, r.emoji);
            assert_eq!(doc.tasks.len(), 2);
            assert_eq!(doc.tasks[0].important, side == Side::Cloud);
            assert_eq!(doc.tasks[0].position, r.tasks[0].position);
            assert_eq!(doc.tasks[1].notes, "保留");
        }
    }
    #[test]
    fn same_edits_and_deletes_coalesce_and_move_vs_delete_conflicts() {
        let base = list(&["A", "B"]);
        let mut l = base.clone();
        l.tasks.pop();
        l.tasks[0].done = true;
        assert_eq!(automatic(&base, &l, &l), l);
        let mut r = l.clone();
        r.tasks[0].position = Some(anchor(&base, 9));
        assert_eq!(automatic(&base, &l, &r), r);
        l.tasks.clear();
        let result = merge(&base, &l, &r, |_| Side::Local);
        assert_eq!(result.issues[0].key, Key::Task(base.tasks[0].id));
        assert!(result.doc.tasks.is_empty());
    }
    #[test]
    fn moved_tasks_keep_coincident_anchors_after_neighbor_deletion() {
        let base = list(&["A", "B", "C"]);
        let (mut l, mut r) = (base.clone(), base.clone());
        l.tasks[0].position = Some(anchor(&base, 2));
        r.tasks[1].position = Some(anchor(&base, 2));
        r.tasks.pop();
        let doc = automatic(&base, &l, &r);
        assert_eq!(doc.tasks.len(), 2);
        assert_eq!(doc.tasks[0].position, doc.tasks[1].position);
        l.tasks[1].position = Some(anchor(&base, 8));
        let result = merge(&base, &l, &r, |_| Side::Local);
        assert_eq!(result.issues[0].key, Key::TaskPosition(base.tasks[1].id));
        assert_eq!(result.doc.tasks[1].position, l.tasks[1].position);
        let mut explicit = base.clone();
        explicit.tasks[0].position = Some(anchor(&base, 0));
        assert!(lists_equivalent(&base, &explicit));
    }
    #[test]
    fn independent_attribute_choices_and_matching_import_ids() {
        let base = list(&[]);
        let (mut l, mut r) = (base.clone(), base.clone());
        l.name = "本机".into();
        r.name = "云端".into();
        l.group = "一".into();
        r.group = "二".into();
        l.default_imports.push(Uuid::new_v4());
        r.default_imports.push(Uuid::new_v4());
        l.tasks.push(Task::default());
        r.tasks = l.tasks.clone();
        r.tasks[0].title = "不同内容".into();
        let result = merge(&base, &l, &r, |key| {
            if key == Key::Name {
                Side::Cloud
            } else {
                Side::Local
            }
        });
        assert_eq!(result.doc.name, r.name);
        assert_eq!(result.doc.group, l.group);
        assert_eq!(result.doc.tasks[0].title, l.tasks[0].title);
        assert_eq!(result.doc.default_imports.len(), 2);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.key == Key::Task(l.tasks[0].id)));
    }
    fn occurrence(parent: Uuid, date: &str) -> Task {
        Task {
            id: Uuid::new_v5(&parent, date.as_bytes()),
            due: date.into(),
            repeat: "daily".into(),
            ..Default::default()
        }
    }
    #[test]
    fn recurring_candidates_and_descendants_follow_one_decision() {
        let mut base = list(&["重复"]);
        let parent = base.tasks[0].id;
        base.tasks[0].repeat = "daily".into();
        let mut l = base.clone();
        l.tasks[0].done = true;
        l.tasks.push(occurrence(parent, "2026-11-25"));
        let mut r = l.clone();
        r.tasks.insert(1, Task::default());
        r.tasks[2].occurrence_of = Some(parent);
        assert_eq!(automatic(&base, &l, &r).tasks.len(), 3);
        let sibling = r.tasks[1].id;
        r.tasks[2] = occurrence(parent, "2026-11-26");
        r.tasks.push(occurrence(r.tasks[2].id, "2026-11-27"));
        for side in [Side::Local, Side::Cloud] {
            let result = merge(&base, &l, &r, |_| side);
            assert_eq!(result.issues.len(), 1);
            assert_eq!(result.issues[0].key, Key::Recurrence(parent));
            assert_eq!(result.issues[0].members.len(), 3);
            assert!(result.doc.tasks.iter().any(|t| t.id == sibling));
            assert_eq!(
                result.doc.tasks.len(),
                if side == Side::Local { 3 } else { 4 }
            );
        }
        l.tasks.clear();
        assert_eq!(merge(&base, &l, &r, |_| Side::Local).doc.tasks.len(), 1);
    }
}
