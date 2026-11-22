import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Cloud,
  Folder,
  FolderPlus,
  ChevronRight,
  ArrowLeft,
  Download,
  LogIn,
} from "lucide-react";
import { refresh, sync } from "./session";
import { uid, type View } from "./model";
interface Dir {
  id: string;
  name: string;
  children: { id: string; name: string }[];
}
export function CloudPanel({ view, close }: { view: View; close: () => void }) {
  const [signingIn, setSigningIn] = useState(false),
    [importDrive, setImportDrive] = useState(""),
    [dir, setDir] = useState<Dir | null>(null),
    [folderName, setFolderName] = useState<string | null>(null),
    [parents, setParents] = useState<string[]>([]),
    [busy, setBusy] = useState(false),
    [msg, setMsg] = useState(""),
    [lists, setLists] = useState<{ id: string; displayName: string }[] | null>(
      null,
    ),
    [selected, setSelected] = useState<string[]>([]);
  const live = useRef(true);
  const attempt = useRef<string | null>(null);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
      if (attempt.current)
        void invoke("cancel_login", { attempt: attempt.current }).catch(
          () => {},
        );
    };
  }, []);
  async function act(fn: () => Promise<void>) {
    setBusy(true);
    setMsg("");
    try {
      await fn();
    } catch (e) {
      setMsg(String(e));
    } finally {
      if (live.current) setBusy(false);
    }
  }
  async function directory(id: string | null) {
    setDir(await invoke<Dir>("folders", { parent: id }));
  }
  async function signin(tasks: boolean) {
    const id = uid();
    attempt.current = id;
    setSigningIn(true);
    try {
      const drive = await invoke<string>("login", {
        importTasks: tasks,
        attempt: id,
      });
      if (!live.current) return;
      setSigningIn(false);
      if (tasks) {
        const imported = await invoke<{ id: string; displayName: string }[]>(
          "import_lists",
          {
            drive,
            workspace: view.binding,
          },
        );
        setImportDrive(drive);
        setLists(imported);
        setSelected([]);
      } else {
        setParents([]);
        await directory(null);
      }
    } finally {
      attempt.current = null;
      if (live.current) setSigningIn(false);
    }
  }
  return (
    <section className="modal-card">
      <Cloud className="modal-symbol" />
      <DialogTitle>OneDrive</DialogTitle>
      {view.binding && (
        <p className="hint">当前数据目录 · {view.binding.label}</p>
      )}
      {signingIn ? (
        <p role="status">请在微软登录页完成授权，FoxTODO 会自动继续。</p>
      ) : dir ? (
        <>
          <h3>选择数据目录</h3>
          <div className="dir-head">
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={!parents.length || busy || folderName !== null}

              title="上一级"
              onClick={() =>
                void act(async () => {
                  await directory(parents.at(-1)!);
                  setParents(parents.slice(0, -1));
                })
              }
            >
              <ArrowLeft size={17} />
            </Button>
            {dir.name}
          </div>
          <div className="choices">
            {dir.children.map((f) => (
              <Button
                variant="ghost"
                className="folder w-full justify-start [&_svg:last-child]:ml-auto"
                key={f.id}
                disabled={busy || folderName !== null}
                onClick={() =>
                  void act(async () => {
                    await directory(f.id);
                    setParents([...parents, dir.id]);
                  })
                }
              >
                <Folder size={17} />
                {f.name}
                <ChevronRight size={16} />
              </Button>
            ))}
          </div>
          {folderName === null ? (
            <>
              <Button
                variant="outline"
                className="w-full"
                disabled={busy}
                onClick={() => {
                  setMsg("");
                  setFolderName("");
                }}
              >
                <FolderPlus size={16} />
                新建文件夹
              </Button>
              <Button
                disabled={busy}
                className="w-full"
                onClick={() =>
                  void act(async () => {
                    await invoke("bind", { folder: dir.id });
                    await refresh();
                    void sync();
                    close();
                  })
                }
              >
                使用此目录
              </Button>
            </>
          ) : (
            <form
              className="grid gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (busy || !folderName.trim()) return;
                void act(async () => {
                  const folder = await invoke<{ id: string; name: string }>(
                    "create_folder",
                    {
                      parent: dir.id,
                      name: folderName.trim(),
                    },
                  );
                  setParents([...parents, dir.id]);
                  setDir({ id: folder.id, name: folder.name, children: [] });
                  setFolderName(null);
                });
              }}
            >
              <label htmlFor="cloud-folder-name">新文件夹名称</label>
              <Input
                id="cloud-folder-name"
                autoFocus
                autoComplete="off"
                value={folderName}
                disabled={busy}
                onChange={(event) => setFolderName(event.target.value)}
              />
              <div className="grid grid-cols-2 gap-2">
                <Button
                  variant="outline"
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    setFolderName(null);
                    setMsg("");
                  }}
                >
                  取消
                </Button>
                <Button
                  type="submit"
                  disabled={busy || !folderName.trim()}
                >
                  创建
                </Button>
              </div>
            </form>
          )}
        </>
      ) : lists ? (
        <>
          <h3>选择要导入的 Microsoft To Do 清单</h3>
          <label className="select-all-lists">
            <Checkbox
              disabled={busy || !lists.length}
              checked={
                selected.length === 0
                  ? false
                  : selected.length === lists.length
                    ? true
                    : "indeterminate"
              }
              onCheckedChange={(checked) =>
                setSelected(
                  checked === true ? lists.map((list) => list.id) : [],
                )
              }
            />
            全选
          </label>
          <div className="choices">
            {lists.map((l) => (
              <label key={l.id}>
                <Checkbox
                  disabled={busy}
                  checked={selected.includes(l.id)}
                  onCheckedChange={(checked) =>
                    setSelected(
                      checked === true
                        ? [...selected, l.id]
                        : selected.filter((i) => i !== l.id),
                    )
                  }
                />
                {l.displayName}
              </label>
            ))}
          </div>
          <Button
            disabled={busy || !selected.length}
            className="w-full"
            onClick={() =>
              void act(async () => {
                const r = await invoke<{
                  imported: number;
                  skipped: number;
                  errors: string[];
                }>("import_selected", {
                  ids: selected,
                  drive: importDrive,
                  workspace: view.binding,
                });
                await refresh();
                void sync();
                setLists(null);
                setMsg(
                  "已导入 " +
                    r.imported +
                    " 个清单，跳过 " +
                    r.skipped +
                    " 个已导入清单。 " +
                    r.errors.join("；"),
                );
              })
            }
          >
            导入所选清单
          </Button>
        </>
      ) : (
        <>
          <p className="hint">
            在每台设备选择同一目录。每个清单保存为一个 JSON 文件。
          </p>
          {!view.oneDriveAvailable && (
            <p className="message">
              此版本尚未启用 OneDrive 登录，清单仍可保存在本机。
            </p>
          )}
          <Button
            disabled={busy || !view.oneDriveAvailable}
            className="w-full"
            onClick={() => void act(() => signin(false))}
          >
            <LogIn size={16} />
            {view.binding ? "登录并选择目录" : "连接个人 OneDrive"}
          </Button>
          {view.oneDriveAvailable && (
            <Button
              variant="outline"
              disabled={busy}
              className="w-full"
              onClick={() => void act(() => signin(true))}
            >
              <Download size={16} />从 Microsoft To Do 导入
            </Button>
          )}
        </>
      )}
      {busy && !signingIn && <p role="status">处理中…</p>}
      {msg && (
        <p className="message" role="status">
          {msg}
        </p>
      )}
      <Button
        variant="ghost"
        className="w-full text-muted-foreground"
        onClick={close}
      >
        关闭
      </Button>
    </section>
  );
}
