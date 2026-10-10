# Paldee Pet：PD 小助手独立可选组件

本次以用户提供的《PD 小助手外部功能组件接入规范》为准：
D:/ai/screen-automation-cua-component/build/helper-cua-integration/docs/EXTERNAL_COMPONENT_PACKAGING.md。
首版 ID 为 octoppet，组件与程序版本为 0.2.3；支持 Windows x64 与 macOS x86_64（darwin）两个平台。
旧的 pd-device-bridge 宠物组件打包方案、全局目录合并工具和相关上传包已退役，不能用于本次发布。

> 2026-10-10：新增 macOS x86_64 平台。两个平台共用同一 octoppet 组件 ID 与 `octoppet-component@1` 运行协议，仅在传输原语（Windows 命名管道 / macOS Unix 域套接字）和可执行格式上不同。参见文末「macOS 平台差异」。

## 仓库、程序及用户数据

- Pet 源码保持在独立仓库 D:/ai/OctopPet，origin 为 https://github.com/xiaozs-com/OctopPet.git。
- 本轮基线为 53b675d；已有未提交的眨眼、文档和快照改动保留。未提交、push 或发布本轮改动。
- 构建采用现有 React/Vite + Tauri 2，Windows x64 MSVC，关闭安装器打包；不全局安装、不修改 PATH、不注册自启动。
- ZIP 入口为 paldee-pet.exe（实际 Tauri release 程序）；网页资源已经嵌入程序。
- 现有 PD CLI 桥接仍需独立开发的 pd-device-bridge.exe，作为运行必需文件随包附带；只复制发布程序与许可证，不复制源码进小助手。
- Pet 许可证为 MIT。用户明确同意 PD 桥接代码按 MIT 分发，桥接独立仓库已补 LICENSE；第三方许可证与来源清单随包提供。
- 配置默认位于 %APPDATA%/com.octop.pet/config.json，WebView 数据位于同目录的 webview/；保留原配置标识和系统 keyring 服务 com.octop.pet。
- 发布版密码与令牌仅存系统 keyring，不在 ZIP 或配置中。开发版既有 dev-secrets.json 仍只用于开发，不进入组件包。
- OCTOPPET_DATA_DIR 可指定绝对的独立用户目录，用于隔离测试或部署；拒绝整个组件根目录（含所有版本和同级数据子目录）及其父目录；检查通过后才创建目录。用户数据不随组件目录卸载。
- 运行控制没有常驻日志文件；JSON 错误直接返回调用者，host 标准输出丢弃。Tauri 开发输出仍由开发控制台承接，不把窗口标题或账号数据写入发布包。
- Windows 必须已有可用 WebView2 Runtime。本组件不静默安装它；缺失时启动失败须由调用者提示。

## octoppet-component@1 运行协议

命令格式：paldee-pet.exe COMMAND --json，可选 --timeout-ms 100..15000；默认总超时 15000 毫秒。

| 命令   | 行为                                                                                   |
| ------ | -------------------------------------------------------------------------------------- |
| status | 查询运行、可见状态、PID、版本、用户目录及本机管道；未运行也成功返回 running=false      |
| start  | 未运行时启动同一路径程序；已运行时返回原 PID，不重复创建窗口，也不主动重新显示隐藏窗口 |
| stop   | 请求正常退出；等进程退出后返回 running=false。重复停止成功。不按任意 PID 强杀程序      |
| show   | 显示宠物窗口，不主动将聊天窗口或其他应用切到前台；未运行返回 NOT_RUNNING               |
| hide   | 隐藏宠物、聊天、设置窗口；不退出进程；未运行返回 NOT_RUNNING                           |

直接双击/无参数执行仍启动宠物；重复双击为无操作。内部 --component-host 仅用于启动后台 GUI 进程。

JSON 成功响应示例：

```json
{
  "protocol": "octoppet-component@1",
  "id": "octoppet",
  "version": "0.2.3",
  "ok": true,
  "command": "status",
  "running": true,
  "visible": true,
  "pid": 1234,
  "error": null
}
```

失败包含 ok=false 与 error.code/error.message。退出码：0 成功，2 INVALID_ARGUMENT，3 UNSUPPORTED_PROTOCOL，4 NOT_RUNNING，5 TIMEOUT，6 START_FAILED，8 IO_ERROR（含权限、帧格式或进程身份异常），9 WINDOW_ERROR。调用者必须同时检查退出码和 ok，不解析人类日志。

控制连接为仅本机命名管道：当前用户 SID、登录会话、用户数据目录固定散列及 dev/release 区分作用域。仅当前用户有 ACL 权限，拒绝远程客户端；第一个管道实例持有运行权，进程异常退出后由系统释放，不依赖遗留 PID 文件。客户端核对系统返回的管道服务器 PID；不同用户、会话或数据目录相互隔离。

聊天桥接协议仍为现有 pd-chat-cli@2，经原 Octop 聊天连接调用本机 CLI；生命周期管道不承接屏幕/鼠标/键盘命令，也不新增服务器连接或端口。本组件不声称屏幕操作全部为后台能力，原 PD CLI 的执行模式和产品限制照常适用。

管道帧为 4 字节小端长度 + UTF-8 JSON，最多 8192 字节。请求仅含 protocol/command；客户端读完响应后发送单字节 1 确认，服务端才断开。每个服务端请求最多 3 秒，窗口主线程调度最多 2 秒。协议不兼容结构化拒绝。

命令超时不会强杀现有 GUI 或其他进程；请求可能已生效，应再次 status 核对再重试。启动超时后仍可能稍后启动成功。异常退出后 status 返回未运行，start 可以恢复。更新和卸载之前必须先用旧版本入口执行 stop 并确认退出。

## 构建与打包

Windows x64：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build_pd_component.ps1 -BridgeRoot D:/ai/screen-automation-device-bridge
```

macOS x86_64（需在原生 x86_64 机器上执行）：

```bash
bash scripts/build_pd_component_macos.sh <screen-automation-device-bridge 根目录> [输出目录]
```

脚本依次执行 `npm run tauri build -- --no-bundle` 与 `cargo build --release --locked --target x86_64-apple-darwin --manifest-path <bridge>/native/Cargo.toml`，再调用 `scripts/package_pd_component_macos.py` 打包。桥接二进制已是 release 产物时，可直接调用打包器跳过重编：

```bash
python3 scripts/package_pd_component_macos.py \
  --pet src-tauri/target/release/octop-pet \
  --bridge <bridge>/native/target/x86_64-apple-darwin/release/pd-device-bridge \
  --bridge-root <bridge>
```

需已有 Node、Rust 与离线依赖缓存；脚本不安装这些工具。Windows 发布 ZIP 按固定白名单放入两个 EXE、component.json、根 LICENSE 与 licenses/ 文本，不遍历 target、缓存或用户目录。桥接采用静态 CRT，两个程序只直接导入系统 DLL，仍需系统已安装 WebView2 Runtime。PE 架构须为 AMD64，入口 status 的实际版本和运行协议也会检查。macOS 打包器额外校验两个可执行文件为 x86_64 Mach-O（`CPU_TYPE_X86_64`）且无非系统动态库依赖（`otool -L`），并以 `create_system=3` 配合 `external_attr` 记录可执行位（0o755），因为归档解包可能丢失嵌套二进制的可执行位、需安装器恢复。

输出在 build/component-release/paldee-pet/：版本目录下 `{id}-{version}-{platform}.zip`、`latest-{platform}.json`、（macOS）`catalog-entry-{platform}.json`，以及未签名清单备份与本地校验报告。

component.json 使用规范字段。catalog 条目是稳定的单条目录，只提供组件身份、分类和各平台清单地址，不包含 version/entry。版本、入口、大小和 SHA-256 由 latest 清单管理，并与 ZIP 内 component.json 核对。打包器本身不生成 catalog.json、不带全局签名。普通版本更新只需上传 ZIP 和已签名的 latest 清单；新增组件或新增平台（如本次 macOS）需把新平台清单地址合并进服务器当前 catalog.json 并重新签署全局目录。Paldee Pet 与 Cua 统一归入“交互增强”。

latest-{platform}.json 由打包器产出时为未签名清单。签名前留存 latest-{platform}.unsigned.json；由受控产品发布环境签名。仓库脚本不读取、创建、复制、输出或提交产品私钥。没有签名的清单不能在线安装，不得误当正式清单上传。签名流程见 [组件发布与源码同步](upstream-sync/COMPONENT_RELEASE.md)。

## 人工上传边界

服务器公开根目录为 /www/wwwroot/xiaozs/sah/components/（即 https://www.xiaozs.com/sah/components/ ）。按「先 ZIP，再平台清单，最后全局 catalog」的顺序上传：

1. 版本目录下的 ZIP → /sah/components/paldee-pet/{version}/；上传后用公开 URL 核对字节数和 SHA-256 与清单一致。
2. 已签名的平台清单 latest-{platform}.json → /sah/components/paldee-pet/。
3. （仅新增组件或新增平台时）已重签的全局 catalog.json → /sah/components/catalog.json，覆盖服务器现有目录。合并前先从服务器下载当前 catalog.json 作为起点，保留其他组件条目（当前为 cua-driver-windows 与 octoppet），只给 octoppet 增加（或更新）对应平台的清单地址和支持平台数组，再用发布私钥重新签名整个目录后上传。建议在服务器先备份旧的 catalog.json。

catalog 条目（catalog-entry-{platform}.json）交给全局目录维护者合并、重新签署并最后上传，不直接上传到组件子目录。OctopPet 仓库脚本不访问/覆盖 Cua 或浏览器增强的组件文件；历史候选全局目录不是本次产物。

注意浏览器增强组件（browser-enhancement-chromium）是另一个独立签名组件，与本组件共用同一发布私钥与 key_id，但其发现走「约定 URL」（{base}/browser-enhancement-chromium/latest-{platform}.json），不依赖也不出现在全局 catalog.json 中。合并 octoppet 的平台清单时不要把浏览器增强错误地加入目录。

本轮仅做本地整改、构建及临时目录验收。小助手调用层还需按此协议调用 start/status/show/hide/stop，再完成原有组件仍可见、真实签名安装、更新和卸载联调。仅安装 ZIP 不意味着通用安装器自动获得这些控制动作。

## 同步上游

保留既有桥接与眨眼改动；新增运行协议集中在 component_runtime.rs，仅在 lib.rs、配置/开发凭据目录及随包桥接查找处接线。打包、测试与发布记录留在 scripts/ 和本文，避免修改上游构建和项目布局。上游更新后先检查这些接线位置、重跑检查及打包验收；不直接覆盖整个文件。

本机适配器源码和服务端扩展仍在独立桥接仓库，OctopPet 仓库不加入 Octop Python 代码；本次不改服务器补丁或服务地址。

本轮验收结果详见 [品牌版验收报告](PALDEE_PET_COMPONENT_ACCEPTANCE.md)。独立组件源码补丁可用 python scripts/export_pd_component_patch.py 重新导出，默认基线 53b675d；同步新上游后显式指定 --base，先检查上下文再应用。这个层不包含已有眨眼改动，也不操作 Git 索引或全局目录。桥接仓库的许可声明单独留在其独立仓库。

对外名称与文件路径使用 Paldee Pet / paldee-pet；内部 ID octoppet、协议和数据标识保持兼容。原源码编译产物 octop-pet.exe 由打包器复制为 paldee-pet.exe，不改 Cargo 包名。0.2.0 旧包保留，仅新版本使用新入口和下载地址。

start 命令使用同一用户/会话/数据目录作用域的 Windows 命名互斥锁串行检查和启动；锁等待计入默认 15 秒或自定义总超时，进程退出自动释放。没有常驻服务或新端口。

## 打开与随主程序启动（0.2.2）

component.json 声明 launchable=true，以及现有 settings 格式的布尔项 start_with_helper，默认 false，显示名为“随“屏幕自动化小助手”启动”。设置保存到小助手独立 component_configs/octoppet.json，不写入组件版本目录。

小助手“功能组件”选中 Paldee Pet 后启用“打开”；打开通过 start --json 再 show --json，能显示已隐藏的现有实例，失败原因显示在组件页。小助手启动只读本地已验签目录，对用户明确开启此设置且已安装的应用调用 start --json，不联网下载，不注册 Windows 自启动，不强制 show，不随小助手退出而关闭独立桌宠。

本功能也需要小助手调用层的本轮修改：components/manager.py、components/operations.py、gui/main_window.py；只更新 ZIP 无法让旧主程序执行联动启动。源码运行的小助手需重启；安装版需发布包含这些修改的新主程序。minimum_helper_version=1.2.6 表示配置/组件协议基线，不替代核实主程序包含本轮调用层。

## macOS 平台差异（0.2.3）

macOS x86_64 平台与 Windows 共用同一 octoppet 组件 ID、`octoppet-component@1` 运行协议、`octoppet` 数据目录散列算法与 dev/release 区分。调用者看到的命令、JSON 负载、错误码和退出码一致。仅传输原语与可执行格式不同：

| 维度             | Windows                                                  | macOS                                                                                      |
| ---------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 控制通道         | 当前用户命名管道（ACL 按 SID 作用域）                    | `$TMPDIR` 内 Unix 域套接字（目录 0700、套接字 0600），作用域为所属 uid，编码进套接字文件名 |
| 服务器进程核对   | `GetNamedPipeServerProcessId`                            | `LOCAL_PEERPID` 对端凭据检查                                                               |
| 入口可执行格式   | PE，须 AMD64；按导入表校验只链接系统 DLL                 | Mach-O，须 `CPU_TYPE_X86_64`；按 `otool -L` 校验无非系统动态库                             |
| 异常退出接管核对 | `TerminateProcess` + `QueryFullProcessImageNameW` 验镜像 | `SIGKILL` + `libproc.proc_pidpath` 验镜像                                                  |
| 归档可执行位     | PE 不需要                                                | `external_attr` 记录 0o755，安装器须在解包后恢复                                           |

用户数据默认目录在 macOS 上为 `~/Library/Application Support/com.octop.pet`（与 `app_config_dir()` 返回一致，已有安装不会迁移）；`OCTOPPET_DATA_DIR` 仍可用于隔离测试或部署，拒绝整个组件根目录及其父目录的规则不变。

macOS 运行时实现集中在 `src-tauri/src/component_runtime_macos.rs`（镜像 Windows 的 `component_runtime.rs`）；`lib.rs` 与 `config_cmd.rs` 通过 `cfg(any(windows, target_os = "macos"))` 同时为两平台启用组件运行时与 `data_dir()` 解析。`cargo check` / `cargo test`（lib 8 项 + 集成 20 项）在本机通过。

macOS 打包相关的第三方许可证：18 个 macOS 专属 Rust crate（objc2 / block2 / dispatch2 系列）上游只发布 LICENSE.md 指针文档、crate 包不带正文，故从 SPDX license-list-data 固定提交拉取官方 MIT 标准正文，存于 `packaging/third-party-licenses/objc2-shared/LICENSE-MIT.txt`，并在 SOURCES.json 为这 18 个 crate 各登记一条共享记录（trio 许可证「Zlib OR Apache-2.0 OR MIT」均包含 MIT，一份正文满足全部）。

macOS 验收测试 `scripts/test_pd_component_macos.py` 镜像 Windows 的 `test_pd_component.py`，覆盖元数据/架构、中文空格安装路径、并发与重复启动、正常与异常停止、超时、协议不兼容、独立用户数据保留与卸载。需对真实签名包执行；本仓库不自动运行。

## 受控签名流程（0.2.3 起）

组件清单与全局目录的 Ed25519 签名属于小助手项目的产品发布步骤，使用独立小助手仓库的受信任发布私钥（key_id `xiaozs-components-2026-01`）；客户端用小助手内置公钥 `EUlug4+wmNeU6MVKjjC1/9cSygyWsSAeEVAHaw13Pqk=` 验签。本仓库刻意不持有、不读取、不复制、不提交该私钥；打包器只产出 `"signature": "pending"` 的未签名清单与 catalog 条目。

签名工具为小助手仓库的 `packaging/sign_component_manifest.py`（依赖 `cryptography`）。流程：先签名平台清单 `latest-{platform}.json`；新增平台时还需下载服务器当前 `catalog.json`，合并新平台清单地址后用同一工具重签整个全局目录。两个产物都要用小助手 `components/signatures.verify_manifest` 复核通过后再上传。签名可在具备私钥的受控机器（含本机）执行；私钥不得进入任何上传文件或提交。
