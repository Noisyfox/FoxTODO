# FoxTODO

FoxTODO 是面向 Windows、macOS 和 Android 的 Tauri 2 任务应用。无需登录即可在本机创建清单、编辑任务和安排提醒。首次连接个人 OneDrive 时，应用将已创建的本机清单带入所选目录。各设备选择同一个目录即可共享清单。

将事项拖到侧栏中的目标清单即可移动。手机上长按事项，再拖到「移动到清单」中的目标。移动的事项会排在目标清单手动顺序的末尾。

## 连接微软账号

在 OneDrive 面板点击「从 Microsoft To Do 导入」，用微软个人账号授权。FoxTODO 自动返回清单选择界面，导入勾选的清单。首次使用可以先导入到本机，再点击「连接个人 OneDrive」选择同步目录。

### 发布者一次性配置

1. 在 Microsoft Entra 创建仅支持个人 Microsoft 帐户的应用注册，或打开已有的 FoxTODO 注册。使用其「应用程序(客户端) ID」。[注册应用](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app)
2. 在「身份验证」中配置原生公共客户端。Windows 和 macOS 的「移动和桌面应用程序」使用 `http://localhost`，实际登录时监听本机临时端口。Android 使用浏览器 Custom Tab，在「Android」平台填写包名 `io.noisyfox.foxtodo` 和签名证书的 SHA-1 Base64 哈希。当前测试包的哈希为 `q+8H2dckkpGcwQ/aQxD+N2SH2hs=`。正式包使用下方 CI 配置的签名证书。Android 原生授权请求和系统回调从同一份深层链接配置读取证书哈希。三端使用授权码 + PKCE（S256），无需客户端密钥或托管服务器。[微软原生应用回调配置](https://learn.microsoft.com/en-us/entra/identity-platform/how-to-add-redirect-uri)
3. 打开「API 权限 → 添加权限 → Microsoft Graph → 委托的权限」，添加 Files.ReadWrite 和 Tasks.Read。用户首次连接时授权文件读写，并通过 offline_access 保持登录；只在主动导入时请求 Tasks.Read。[配置 Graph 委托权限](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-configure-app-access-web-apis)
4. 将公开的客户端 ID 单独写一行到 [microsoft-client-id.txt](src-tauri/microsoft-client-id.txt)，然后重新构建三端安装包。也可在编译 Rust 客户端的环境中设置 `FOXTODO_CLIENT_ID`。同一注册可用于三个平台。未配置 ID 的构建可用于本机离线编辑。

## 文件和同步

每个清单使用稳定的 UUID 作为 JSON 文件名，名称、分组、任务和附件内容都在文件内。应用在 OneDrive 中只处理自己能识别的清单。客户端通过 Graph 直接访问选定的目录，无需安装 OneDrive 桌面客户端。

在应用数据目录中，每份清单有一份本地工作文件。编辑时，应用把正文、待上传标记和上次同步的清单内容及版本一起原子写入。关闭应用后可以继续离线编辑。首次同步完成后，对应的云端内容才能离线访问。目录选择与凭据分别保存在本地设置和系统凭据存储中。

应用在编辑后、启动、回到前台、联网和手动刷新时同步，前台也会定期刷新。每次提交一个清单的完整 JSON。两台设备离线修改同一清单时，不同任务的增删改会自动合并。任务内容与拖动排序、不同清单属性也可独立合并。两端对同一任务做出不同的内容修改、删除与编辑相遇，或重复任务生成不同的下一期时，可以逐项选择本机或 OneDrive 版本，独立修改会一并保留。整张清单的删除与编辑相遇，或旧数据缺少可靠的共同版本时，应用会提示选择整张清单的处理方式。退出期间的待上传内容在下次运行并联网后继续处理。

系统提醒在设备下载或保存相应任务后安排。Android 的投递时间受系统省电与闹钟权限策略影响。Windows 建议通过安装程序安装，macOS 请从应用包启动，以便系统识别提醒来源。

## 开发与构建

安装 Node.js 22.12 或更新版本、pnpm、Rust stable 及目标平台的 [Tauri 开发依赖](https://v2.tauri.app/start/prerequisites/)。Android 还需要 JDK、Android SDK 36、NDK 和相应的 Rust Android target。仓库中包含 Android 工程。

| 操作 | 命令 |
|---|---|
| 安装前端依赖 | `pnpm install` |
| 启动桌面开发版 | `pnpm tauri dev` |
| 前端类型检查与打包 | `pnpm build` |
| 重复任务测试 | `pnpm test` |
| Rust 持久化与模拟双设备同步测试 | `cargo test --manifest-path src-tauri/Cargo.toml` |
| 在目标桌面系统上打包 | `pnpm tauri build` |
| 构建 Android ARM64 调试 APK | `pnpm tauri android build --target aarch64 --debug --apk` |

前端使用 React、shadcn/ui 和 Tailwind CSS 4。共享控件位于 `src/components/ui`，主题在 `src/style.css` 中配置。

Rust 持久化、Graph、认证、导入与同步代码在 src-tauri/src。模拟测试覆盖响应丢失、上传期间编辑、并发提交和删除冲突。真实账号的权限、上传会话提交行为和附件导入仍需使用个人 OneDrive 验收。

## GitHub Actions 构建

可在 [FoxTODO Nightly](https://github.com/Noisyfox/FoxTODO/releases/tag/nightly-builds) 下载 Windows x64 安装程序、macOS Apple Silicon DMG 和正式签名的 Android ARM64 APK。推送到 `main` 或在 `main` 手动运行 **Build FoxTODO** 会触发 Nightly 构建。三个平台都成功后，工作流更新同一个预发布版，下载地址保持固定。发布说明链接到对应的源码提交和构建记录。

macOS Nightly 使用临时签名（ad hoc），尚未经过 Apple 公证。将 FoxTODO 拖入「应用程序」后首次打开时，如果 macOS 提示无法验证开发者，请确认安装包来自本仓库的 Nightly，再到「系统设置 → 隐私与安全性」为 FoxTODO 选择「仍要打开」。[Apple 的首次打开说明](https://support.apple.com/zh-cn/102445)

推送到 `main`、打开或更新 PR，或手动运行 **Build FoxTODO** 后，可在对应 Actions 运行的 **Artifacts** 下载 Windows x64 安装程序、macOS Apple Silicon 磁盘映像和 Android ARM64 未签名 APK。推送到 `main` 或在 `main` 手动运行时，还会生成可直接安装的 `FoxTODO-android-arm64-signed`。分发时使用其中的 `FoxTODO-android-arm64-release.apk`。从使用其他证书的调试版切换时，Android 会拒绝覆盖安装，卸载前请先确认数据已同步。

正式签名使用 GitHub Environment `android-release`，其部署规则仅允许 `main` 分支。该环境内的 `ANDROID_KEYSTORE_BASE64` 是 PKCS#12 密钥库的 Base64 编码，`ANDROID_KEYSTORE_PASSWORD` 是密钥库密码，别名为 `foxtodo`。签名任务使用同一次运行生成的 APK，签名后会验证 APK 签名以及 Android 登录回调与实际签名证书是否一致。

在 Microsoft Entra 的 FoxTODO 应用注册中，进入「身份验证」，为包名 `io.noisyfox.foxtodo` 添加 Android 签名哈希 `/1zoPBCNDNq7tO3p9GPPso07eXY=`，保留原有调试回调。此值是正式证书的 SHA-1 Base64，开头的斜杠属于哈希。CI 使用 `src-tauri/tauri.android.release.conf.json`，本地普通调试构建继续使用 `src-tauri/tauri.android.conf.json`。
