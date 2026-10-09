# Paldee Pet：PD 小助手独立可选组件

本次以用户提供的《PD 小助手外部功能组件接入规范》为准：
D:/ai/screen-automation-cua-component/build/helper-cua-integration/docs/EXTERNAL_COMPONENT_PACKAGING.md。
首版 ID 为 octoppet，组件与程序版本为 0.2.3；只支持实际构建的 Windows x64。
旧的 pd-device-bridge 宠物组件打包方案、全局目录合并工具和相关上传包已退役，不能用于本次发布。

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

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build_pd_component.ps1 -BridgeRoot D:/ai/screen-automation-device-bridge
```

需已有 Node、Rust/MSVC、WebView2 和离线依赖缓存；脚本不安装这些工具。发布 ZIP 按固定白名单放入两个 EXE、component.json、根 LICENSE 与 licenses/ 文本，不遍历 target、缓存或用户目录。桥接采用静态 CRT，两个程序只直接导入系统 DLL，仍需系统已安装 WebView2 Runtime。PE 架构须为 AMD64，入口 status 的实际版本和运行协议也会检查。

输出在 build/component-release/paldee-pet/：0.2.3/paldee-pet-0.2.3-windows-x64.zip、latest-windows-x64.json、catalog-entry.json，以及未签名清单备份与本地校验报告。

component.json 使用规范字段。catalog-entry.json 是稳定的单条目录，只提供组件身份、分类和各平台清单地址，不包含 version/entry。版本、入口、大小和 SHA-256 由 latest 清单管理，并与 ZIP 内 component.json 核对。没有 components 数组或全局签名，不生成 catalog.json。普通版本更新只需上传 ZIP 和已签名的 latest 清单；新增组件或修改目录信息时才合并并重新签署全局目录。Paldee Pet 与 Cua 统一归入“交互增强”。

latest-windows-x64.json 当前是未签名清单。签名前留存 latest-windows-x64.unsigned.json；由受控产品发布环境签名。仓库脚本不读取、创建、复制、输出或提交产品私钥。没有签名的清单不能在线安装，不得误当正式清单上传。

## 人工上传边界

仅由用户上传到 /www/wwwroot/xiaozs/sah/components/paldee-pet/：先版本目录下 ZIP，再验证公开下载字节数和 SHA-256，最后上传已签名的 latest-windows-x64.json。

catalog-entry.json 交给全局目录维护者合并、重新签署并最后上传。OctopPet 仓库不处理全局目录，不访问/覆盖 Cua 的组件文件；历史候选全局目录不是本次产物。

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
