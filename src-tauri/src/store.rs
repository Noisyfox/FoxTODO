use crate::merge::{self, Key, Merge, Side};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
// A workspace has one Tasks JSON, even when two devices first use it offline.
pub const DEFAULT_LIST_ID: Uuid = uuid::uuid!("3d430186-0ef4-5a7c-9eb6-6a4d69e92b49");
pub type Result<T> = std::result::Result<T, String>;
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn hash(data: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(data))
}
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&fs::read(path).map_err(err)?)
        .map_err(|e| format!("{}：{e}", path.display()))
}
pub fn atomic<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    let dir = path.parent().ok_or("存储路径无效")?;
    fs::create_dir_all(dir).map_err(err)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(err)?;
    serde_json::to_writer_pretty(&mut tmp, data).map_err(err)?;
    tmp.write_all(b"\n").map_err(err)?;
    tmp.as_file().sync_all().map_err(err)?;
    tmp.persist(path).map_err(err)?;
    #[cfg(unix)]
    fs::File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(err)?;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: Uuid,
    pub title: String,
    pub done: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub name: String,
    pub content_bytes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub list_name: String,
    pub list_id: Uuid,
    pub index: usize,
    pub rank: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_of: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub important: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub reminder: String,
    #[serde(default)]
    pub my_day: String,
    #[serde(default)]
    pub repeat: String,
    #[serde(default)]
    pub repeat_anchor: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub imported: Option<serde_json::Value>,
}
impl Default for Task {
    fn default() -> Self {
        Self {
            occurrence_of: None,
            position: None,
            id: Uuid::new_v4(),
            title: String::new(),
            done: false,
            important: false,
            notes: String::new(),
            steps: vec![],
            due: String::new(),
            reminder: String::new(),
            my_day: String::new(),
            repeat: String::new(),
            repeat_anchor: String::new(),
            attachments: vec![],
            categories: vec![],
            imported: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub version: u32,
    pub id: Uuid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(default)]
    pub group: String,
    pub tasks: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_imports: Vec<Uuid>,
}
impl Document {
    pub fn new(id: Uuid, name: String) -> Self {
        Self {
            version: 1,
            position: None,
            id,
            name,
            emoji: None,
            group: String::new(),
            tasks: vec![],
            default_imports: vec![],
        }
    }
    pub fn validate(&self, id: Uuid) -> Result<()> {
        if self.version != 1 || self.id != id || self.name.trim().is_empty() {
            return Err("清单格式、标识或名称无效".into());
        }
        let mut ids = std::collections::HashSet::new();
        for t in &self.tasks {
            if !t.due.is_empty() && chrono::NaiveDate::parse_from_str(&t.due, "%Y-%m-%d").is_err()
                || !t.reminder.is_empty()
                    && chrono::DateTime::parse_from_rfc3339(&t.reminder).is_err()
            {
                return Err("清单中的日期或提醒格式无效".into());
            }
            if !ids.insert(t.id) {
                return Err("清单存在重复任务 ID".into());
            }
        }
        Ok(())
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(self).map_err(err)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Base {
    pub id: String,
    pub etag: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remote {
    pub base: Base,
    pub doc: Document,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub remote: Option<Remote>,
}
#[derive(Debug, Deserialize)]
pub struct Decision {
    pub key: Key,
    pub side: Side,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub doc: Document,
    pub revision: u64,
    pub dirty: bool,
    pub deleted: bool,
    pub base: Option<Base>,
    // When clean, doc itself is the acknowledged version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_doc: Option<Document>,
    pub conflict: Option<Conflict>,
    pub sending: Option<String>,
}
impl Record {
    pub fn new(doc: Document) -> Self {
        let base_doc =
            (doc.id == DEFAULT_LIST_ID).then(|| Document::new(DEFAULT_LIST_ID, "任务".into()));
        Self {
            doc,
            base_doc,
            revision: 1,
            dirty: true,
            deleted: false,
            base: None,
            conflict: None,
            sending: None,
        }
    }
    pub fn begin_edit(&mut self) {
        if !self.dirty && self.base.is_some() {
            self.base_doc = Some(self.doc.clone());
        }
        self.dirty = true;
    }
    pub fn acknowledge(&mut self, base: Base, doc: Document) {
        self.dirty = self.deleted || self.doc != doc;
        self.base = Some(base);
        self.base_doc = self.dirty.then_some(doc);
        self.sending = None;
        self.conflict = None;
    }
    pub fn ancestor(&self) -> Option<&Document> {
        let base = self.base_doc.as_ref()?;
        // If an upload may have committed and both sides subsequently changed,
        // the last acknowledged document may predate a local edit and its undo.
        if let Some(sent) = &self.sending {
            if hash(self.doc.bytes().ok()?) != *sent {
                return None;
            }
        }
        Some(base)
    }

    pub fn merge_with(&self, remote: &Remote, choose: impl Fn(Key) -> Side) -> Option<Merge> {
        if self.deleted
            || self
                .base
                .as_ref()
                .is_some_and(|base| base.id != remote.base.id)
        {
            return None;
        }
        Some(merge::merge(
            self.ancestor()?,
            &self.doc,
            &remote.doc,
            choose,
        ))
    }
    pub fn accept_merge(&mut self, merged: Document, remote: Remote) {
        self.doc = merged;
        self.acknowledge(remote.base, remote.doc);
        self.revision += 1;
    }
    pub fn adopt(remote: Remote) -> Self {
        Self {
            base: Some(remote.base),
            base_doc: None,
            dirty: false,
            ..Record::new(remote.doc)
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub drive: String,
    pub folder: String,
    pub label: String,
    pub client_id: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub binding: Option<Binding>,
}
pub struct Store {
    root: PathBuf,
    pub settings: Settings,
    _lock: fs::File,
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root).map_err(err)?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("instance.lock"))
            .map_err(err)?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| "FoxTODO 数据目录正在使用")?;
        let path = root.join("settings.json");
        let settings = if path.exists() {
            read(&path)?
        } else {
            Settings::default()
        };
        Ok(Self {
            root,
            settings,
            _lock: lock,
        })
    }
    fn directory(&self, b: Option<&Binding>) -> PathBuf {
        self.root.join("workspaces").join(
            b.map(|b| hash(format!("{}\n{}", b.drive, b.folder)))
                .unwrap_or_else(|| "local".into()),
        )
    }
    fn dir(&self) -> PathBuf {
        self.directory(self.settings.binding.as_ref())
    }
    // Both list files must be visible together to edits and sync. Replay the
    // durable pair if a previous write was interrupted, including on restart.
    fn finish_move(&self, records: &[Record; 2]) -> Result<()> {
        let dir = self.dir();
        for r in records {
            atomic(&dir.join(format!("{}.json", r.doc.id)), r)?;
        }
        fs::remove_file(dir.join("pending-move")).map_err(err)
    }

    fn recover_move(&self) -> Result<()> {
        let journal = self.dir().join("pending-move");
        if journal.try_exists().map_err(err)? {
            self.finish_move(&read(&journal)?)?;
        }
        Ok(())
    }

    pub fn move_task(
        &self,
        source: Uuid,
        destination: Uuid,
        task: Uuid,
        destination_revision: u64,
        position: Option<Position>,
    ) -> Result<[Record; 2]> {
        if source == destination {
            return Err("任务已在此清单中".into());
        }
        let mut from = self
            .get(source)?
            .filter(|r| !r.deleted)
            .ok_or("来源清单已不存在")?;
        let mut to = match self.get(destination)? {
            Some(r) if !r.deleted && r.revision == destination_revision => r,
            None if destination == DEFAULT_LIST_ID && destination_revision == 0 => {
                Record::new(Document::new(destination, "任务".into()))
            }
            _ => return Err("目标清单已更新，请重试".into()),
        };
        let index = from
            .doc
            .tasks
            .iter()
            .position(|t| t.id == task)
            .ok_or("任务已被移除")?;
        if to.doc.tasks.iter().any(|t| t.id == task) {
            return Err("目标清单中已有此任务".into());
        }

        from.begin_edit();
        to.begin_edit();
        // Apply the same legacy import normalization the editor sees, before
        // moving the last task that identifies an imported list.
        for r in [&mut from, &mut to] {
            if r.conflict.is_none() {
                crate::import::show_legacy_icon(&mut r.doc);
            }
        }
        let mut moved = from.doc.tasks.remove(index);
        // Legacy tasks derive their order from their original array index.
        for (i, t) in from.doc.tasks.iter_mut().enumerate().skip(index) {
            t.position.get_or_insert_with(|| Position {
                list_name: from.doc.name.clone(),
                list_id: source,
                index: i + 1,
                rank: "a0".into(),
            });
        }
        moved.position = position;
        to.doc.tasks.push(moved);
        from.revision += 1;
        to.revision += 1;
        let records = [from, to];
        for r in &records {
            r.doc.validate(r.doc.id)?;
        }
        atomic(&self.dir().join("pending-move"), &records)?;
        self.finish_move(&records)?;
        Ok(records)
    }

    pub fn records(&self) -> Result<Vec<Record>> {
        self.recover_move()?;
        let dir = self.dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        fs::read_dir(dir)
            .map_err(err)?
            .filter_map(|entry| match entry {
                Ok(e) if e.path().extension().is_some_and(|x| x == "json") => Some(read(&e.path())),
                Ok(_) => None,
                Err(e) => Some(Err(err(e))),
            })
            .collect()
    }
    pub fn get(&self, id: Uuid) -> Result<Option<Record>> {
        self.recover_move()?;
        let p = self.dir().join(format!("{id}.json"));
        if p.exists() {
            Ok(Some(read(&p)?))
        } else {
            Ok(None)
        }
    }
    pub fn save(&self, r: &Record) -> Result<()> {
        self.recover_move()?;
        r.doc.validate(r.doc.id)?;
        atomic(&self.dir().join(format!("{}.json", r.doc.id)), r)
    }
    pub fn remove(&self, id: Uuid) -> Result<()> {
        self.recover_move()?;
        fs::remove_file(self.dir().join(format!("{id}.json"))).map_err(err)
    }
    pub fn bind(&mut self, b: Binding) -> Result<()> {
        self.recover_move()?;
        let dir = self.directory(Some(&b));
        if self.settings.binding.is_none() {
            for record in self.records()? {
                let p = dir.join(format!("{}.json", record.doc.id));
                // Retrying first-time binding must include edits made since a partial copy.
                atomic(&p, &record)?;
            }
        }
        let settings = Settings { binding: Some(b) };
        atomic(&self.root.join("settings.json"), &settings)?;
        self.settings = settings;
        Ok(())
    }
    pub fn edit(&self, doc: Document, revision: u64, deleted: bool) -> Result<()> {
        if doc.id == DEFAULT_LIST_ID && (deleted || doc.name != "任务" || !doc.group.is_empty()) {
            return Err("固定任务清单无法删除、重命名或移动".into());
        }
        let mut r = match self.get(doc.id)? {
            Some(r) if revision == r.revision => r,
            None if revision == 0 => Record::new(doc.clone()),
            Some(_) => return Err("清单已更新，请重新打开后编辑".into()),
            None => return Err("清单已被移除，请重新打开".into()),
        };
        r.begin_edit();
        r.doc = doc;
        r.deleted = deleted;
        r.revision += 1;
        self.save(&r)
    }
    fn pending_conflict(&self, id: Uuid, revision: u64) -> Result<(Record, Conflict)> {
        let mut r = self.get(id)?.ok_or("清单不存在")?;
        if r.revision != revision {
            return Err("冲突内容已更新，请重新查看".into());
        }
        let c = r.conflict.take().ok_or("清单没有冲突")?;
        Ok((r, c))
    }

    pub fn resolve_items(&self, id: Uuid, revision: u64, decisions: &[Decision]) -> Result<()> {
        let (mut r, conflict) = self.pending_conflict(id, revision)?;
        let remote = conflict.remote.ok_or("请重新查看清单冲突")?;
        let answers: std::collections::BTreeMap<_, _> =
            decisions.iter().map(|d| (d.key, d.side)).collect();
        let result = r
            .merge_with(&remote, |key| {
                answers.get(&key).copied().unwrap_or(Side::Local)
            })
            .ok_or("请重新查看清单冲突")?;
        if answers.len() != decisions.len()
            || answers.len() != result.issues.len()
            || !result
                .issues
                .iter()
                .all(|issue| answers.contains_key(&issue.key))
        {
            return Err("请为每项冲突选择一个版本".into());
        }
        r.accept_merge(result.doc, remote);
        self.save(&r)
    }

    pub fn resolve(&self, id: Uuid, revision: u64, choice: &str) -> Result<()> {
        let (mut r, c) = self.pending_conflict(id, revision)?;
        if choice == "both" && !r.deleted {
            let fork_id = Uuid::new_v5(&id, format!("fork-{revision}").as_bytes());
            if self.get(fork_id)?.is_none() {
                let mut fork = c
                    .remote
                    .as_ref()
                    .and_then(|remote| r.merge_with(remote, |_| Side::Local))
                    .map(|m| m.doc)
                    .unwrap_or_else(|| r.doc.clone());
                fork.id = fork_id;
                fork.name.push_str("（本机副本）");
                self.save(&Record::new(fork))?;
            }
        }
        let side = match choice {
            "local" => Side::Local,
            "both" | "cloud" => Side::Cloud,
            _ => return Err("无效的冲突处理方式".into()),
        };
        match c.remote {
            Some(remote) => {
                if let Some(result) = r.merge_with(&remote, |_| side) {
                    r.accept_merge(result.doc, remote);
                } else {
                    if side == Side::Cloud {
                        r.doc = remote.doc.clone();
                        r.deleted = false;
                    }
                    r.acknowledge(remote.base, remote.doc);
                    r.revision += 1;
                }
            }
            None if side == Side::Cloud => return self.remove(id),
            None => {
                r.base = None;
                r.base_doc = None;
                r.sending = None;
                r.dirty = true;
                r.revision += 1;
            }
        }
        self.save(&r)
    }
}
