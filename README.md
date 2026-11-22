# FoxTODO

FoxTODO 是面向 Windows、macOS 和 Android 的 Tauri 2 任务应用。无需登录即可在本机创建清单、编辑任务和安排提醒。首次连接个人 OneDrive 时，应用将已创建的本机清单带入所选目录。各设备选择同一个目录即可共享清单。

## 连接微软账号

在 OneDrive 面板点击「从 Microsoft To Do 导入」，用微软个人账号授权。FoxTODO 自动返回清单选择界面，导入勾选的清单。首次使用可以先导入到本机，再点击「连接个人 OneDrive」选择同步目录。

### 发布者一次性配置

1. 在 Microsoft Entra 创建仅支持个人 Microsoft 帐户的应用注册，或打开已有的 FoxTODO 注册。使用其「应用程序(客户端) ID」。[注册应用](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app)
2. 在「身份验证」中配置原生公共客户端。Windows 和 macOS 的「移动和桌面应用程序」使用 `http://localhost`，实际登录时监听本机临时端口。Android 使用浏览器 Custom Tab，在「Android」平台填写包名 `io.noisyfox.foxtodo` 和签名证书的 SHA-1 Base64 哈希。当前测试包的哈希为 `q+8H2dckkpGcwQ/aQxD+N2SH2hs=`。更换签名证书时，需要同步更新 Azure 注册、Android 深层链接路径和原生端授权请求中的回调 URI。三端使用授权码 + PKCE（S256），无需客户端密钥或托管服务器。[微软原生应用回调配置](https://learn.microsoft.com/en-us/entra/identity-platform/how-to-add-redirect-uri)
3. 打开「API 权限 → 添加权限 → Microsoft Graph → 委托的权限」，添加 Files.ReadWrite 和 Tasks.Read。用户首次连接时授权文件读写，并通过 offline_access 保持登录；只在主动导入时请求 Tasks.Read。[配置 Graph 委托权限](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-configure-app-access-web-apis)
4. 将公开的客户端 ID 单独写一行到 [microsoft-client-id.txt](src-tauri/microsoft-client-id.txt)，然后重新构建三端安装包。也可在编译 Rust 客户端的环境中设置 `FOXTODO_CLIENT_ID`。同一注册可用于三个平台。未配置 ID 的构建可用于本机离线编辑。

## 文件和同步

每个清单使用稳定的 UUID 作为 JSON 文件名，名称、分组、任务和附件内容都在文件内。应用在 OneDrive 中只处理自己能识别的清单。客户端通过 Graph 直接访问选定的目录，无需安装 OneDrive 桌面客户端。

在应用数据目录中，每份清单有一份本地工作文件。正文、待上传标记和上次已知的云端版本一起原子写入。关闭应用后可以继续离线编辑。首次同步完成后，对应的云端内容才能离线访问。目录选择与凭据分别保存在本地设置和系统凭据存储中。

应用在编辑后、启动、回到前台、联网和手动刷新时同步，前台也会定期刷新。每次提交一个清单的完整 JSON。并发修改同一清单时，应用保留本机稿，提供保留两份、采用云端版本和采用本机版本的操作。删除与编辑相遇也会提示。退出期间的待上传内容在下次运行并联网后继续处理。

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

界面位于 src，Rust 持久化、Graph、认证、导入与同步代码在 src-tauri/src。模拟测试覆盖响应丢失、上传期间编辑、并发提交和删除冲突。真实账号的权限、上传会话提交行为和附件导入仍需使用个人 OneDrive 验收。
