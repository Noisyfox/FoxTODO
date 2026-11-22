import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Cloud,
  Folder,
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
      <h2>OneDrive</h2>
      {view.binding && (
        <p className="hint">当前数据目录 · {view.binding.label}</p>
      )}
      {signingIn ? (
        <p role="status">请在微软登录页完成授权，FoxTODO 会自动继续。</p>
      ) : dir ? (
        <>
          <h3>选择数据目录</h3>
          <div className="dir-head">
            <button
              disabled={!parents.length || busy}
              className="icon"
              title="上一级"
              onClick={() =>
                void act(async () => {
                  await directory(parents.at(-1)!);
                  setParents(parents.slice(0, -1));
                })
              }
            >
              <ArrowLeft size={17} />
            </button>
            {dir.name}
          </div>
          <div className="choices">
            {dir.children.map((f) => (
              <button
                className="folder"
                key={f.id}
                disabled={busy}
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
              </button>
            ))}
          </div>
          <button
            disabled={busy}
            className="primary"
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
          </button>
        </>
      ) : lists ? (
        <>
          <h3>选择要导入的 Microsoft To Do 清单</h3>
          <div className="choices">
            {lists.map((l) => (
              <label key={l.id}>
                <input
                  type="checkbox"
                  checked={selected.includes(l.id)}
                  onChange={(e) =>
                    setSelected(
                      e.target.checked
                        ? [...selected, l.id]
                        : selected.filter((i) => i !== l.id),
                    )
                  }
                />
                {l.displayName}
              </label>
            ))}
          </div>
          <button
            disabled={busy || !selected.length}
            className="primary"
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
          </button>
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
          <button
            disabled={busy || !view.oneDriveAvailable}
            className="primary"
            onClick={() => void act(() => signin(false))}
          >
            <LogIn size={16} />
            {view.binding ? "登录并选择目录" : "连接个人 OneDrive"}
          </button>
          {view.oneDriveAvailable && (
            <button
              disabled={busy}
              className="secondary"
              onClick={() => void act(() => signin(true))}
            >
              <Download size={16} />从 Microsoft To Do 导入
            </button>
          )}
        </>
      )}
      {busy && !signingIn && <p role="status">处理中…</p>}
      {msg && (
        <p className="message" role="status">
          {msg}
        </p>
      )}
      <button className="text-button" onClick={close}>
        关闭
      </button>
    </section>
  );
}
