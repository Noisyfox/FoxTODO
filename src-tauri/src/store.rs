use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
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
    pub version: u32,
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub tasks: Vec<Task>,
}
impl Document {
    pub fn new(id: Uuid, name: String) -> Self {
        Self {
            version: 1,
            id,
            name,
            group: String::new(),
            tasks: vec![],
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub doc: Document,
    pub revision: u64,
    pub dirty: bool,
    pub deleted: bool,
    pub base: Option<Base>,
    pub conflict: Option<Conflict>,
    pub sending: Option<String>,
}
impl Record {
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            revision: 1,
            dirty: true,
            deleted: false,
            base: None,
            conflict: None,
            sending: None,
        }
    }
    pub fn adopt(remote: Remote) -> Self {
        Self {
            base: Some(remote.base),
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
    pub fn records(&self) -> Result<Vec<Record>> {
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
        let p = self.dir().join(format!("{id}.json"));
        if p.exists() {
            Ok(Some(read(&p)?))
        } else {
            Ok(None)
        }
    }
    pub fn save(&self, r: &Record) -> Result<()> {
        r.doc.validate(r.doc.id)?;
        atomic(&self.dir().join(format!("{}.json", r.doc.id)), r)
    }
    pub fn remove(&self, id: Uuid) -> Result<()> {
        fs::remove_file(self.dir().join(format!("{id}.json"))).map_err(err)
    }
    pub fn bind(&mut self, b: Binding) -> Result<()> {
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
        let mut r = match self.get(doc.id)? {
            Some(r) if revision == r.revision => r,
            None if revision == 0 => Record::new(doc.clone()),
            Some(_) => return Err("清单已更新，请重新打开后编辑".into()),
            None => return Err("清单已被移除，请重新打开".into()),
        };
        r.doc = doc;
        r.deleted = deleted;
        r.dirty = true;
        r.revision += 1;
        self.save(&r)
    }
    pub fn resolve(&self, id: Uuid, revision: u64, choice: &str) -> Result<()> {
        let mut r = self.get(id)?.ok_or("清单不存在")?;
        if r.revision != revision {
            return Err("冲突内容已更新，请重新查看".into());
        }
        let c = r.conflict.take().ok_or("清单没有冲突")?;
        if choice == "both" && !r.deleted {
            let fork_id = Uuid::new_v5(&id, format!("fork-{revision}").as_bytes());
            if self.get(fork_id)?.is_none() {
                let mut fork = r.doc.clone();
                fork.id = fork_id;
                fork.name.push_str("（本机副本）");
                self.save(&Record::new(fork))?;
            }
        }
        match choice {
            "both" | "cloud" => match c.remote {
                Some(v) => {
                    let mut next = Record::adopt(v);
                    next.revision = r.revision + 1;
                    self.save(&next)
                }
                None => self.remove(id),
            },
            "local" => {
                r.base = c.remote.map(|x| x.base);
                r.sending = None;
                r.dirty = true;
                r.revision += 1;
                self.save(&r)
            }
            _ => Err("无效的冲突处理方式".into()),
        }
    }
}
