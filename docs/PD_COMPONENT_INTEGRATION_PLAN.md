> 源码修改清单、补丁快照与升级步骤统一见 [上游同步记录](upstream-sync/README.md)。本文件保留开发演进历史。

> 当前方案为第 10 节：按需小助手 CLI 代理。第 9 节独立窗口查询原型已退役。

> 最新方案见第 9 节。前面的 SSH/独立隧道设想已被用户否决，仅保留作为变更历史。

# 远程 Octop → Skill → 本机 PD CLI：功能组件接入方案

日期：2026-10-06。状态：源码调查与方案；已只读检查用户提供的服务地址，尚未登录、部署或建立工具连接。

远程 Octop 地址：`http://47.77.184.54:8088/`（用户提供）。只读检查结果：首页 HTTP 200；`/api/health` 返回 JSON，`ok=true`、`db=true`；`/api/version` 返回 401，需要登录后核实版本。`/openapi.json` 返回 HTML 页面回退，不能视为接口描述可用。用户已确认：可以 SSH 登录服务器，Octop 直接运行，不使用 Docker；SSH 转发权限尚未验证。

本轮方向修订：按用户要求采用现有 `screen-automation` Skill + CLI；取消 MCP 作为接入前提。上一版的 MCP 包装与连接器接入计划由本版替代，未实现过这些功能。

## 1. 已确认的方向

- Octop 在远程服务器运行，负责 AI 对话和工具选择。
- OctopPet 保留原有 Octop 聊天，作为宠物与聊天界面。
- 本机 PD 小助手负责执行；直接复用已有 CLI，不新增执行器或业务接口。
- 复用 `D:/ai/screen-automation/skills/screen-automation` 的 Skill、能力发现、安全和验证规则，不改成另一套任务匹配逻辑。
- PD 对接是独立、可安装、可更新、可卸载的功能组件，参考 Cua 组件的分发机制。
- 尽量不修改 Octop 和 OctopPet 的源码结构；必要补丁独立记录。

## 2. 调查结论与证据

| 内容           | 已核实的事实                                                                                      | 来源                                                                                                                                                                                                                  |
| -------------- | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PD CLI         | 有 workflow list、start-workflow、runs list、pause/resume/stop --run-id、result latest --workflow | `D:/ai/screen-automation-helper/runtime/cli.py`，解析器与命令分发                                                                                                                                                     |
| 组件分发       | 独立入口、组件元数据、签名清单、SHA-256、版本目录、回滚和卸载                                     | `components/manager.py`、`components/signatures.py`、`components/storage.py`                                                                                                                                          |
| 组件配置       | 元数据声明设置，配置保存到 component_configs/<id>.json                                            | `components/configuration.py`                                                                                                                                                                                         |
| Cua 组件       | 独立仓库和 sidecar；组件包装不等于执行协议必须照搬 NDJSON                                         | `D:/ai/screen-automation-cua-component/component.json`、`docs/INTEGRATION_CONTRACT.md`                                                                                                                                |
| 目录登记       | 当前组件中心通过固定 OPTIONAL_COMPONENTS 列表呈现组件，尚非自动发现                               | `components/catalog.py`、`components/operations.py`                                                                                                                                                                   |
| 后台启动器     | 当前只接受浏览器组件的固定启动路由，不是任意组件通用启动器                                        | `runtime/component_launcher.py`                                                                                                                                                                                       |
| Skill 调用     | Skill 用 resolve_cli.ps1 / .sh 定位当前电脑的已安装 CLI，依靠 Agent 执行命令                      | `D:/ai/screen-automation/skills/screen-automation/SKILL.md` 和 `scripts/resolve_cli.ps1`                                                                                                                              |
| Octop 执行位置 | 默认 local_shell 在 Octop 宿主机执行；远程文件存储不等于 Windows 桌面执行                         | [backend/resolver.py](https://github.com/TencentCloud/Octop/blob/main/src/octop/infra/backend/resolver.py)、[agent-backend-file-io.md](https://github.com/TencentCloud/Octop/blob/main/docs/agent-backend-file-io.md) |
| Skill 扩展入口 | 官方 Skill 插件通过 ctx.skills 同步 Skill 到 Agent 工作区，无需注册新工具                         | [demo-greeting-skill/main.py](https://github.com/TencentCloud/Octop/blob/main/plugins/demo-greeting-skill/main.py)                                                                                                    |
| OctopPet 聊天  | 现有聊天将用户请求交给远程 Agent，不会把服务端 execute 自动改为本机执行                           | `src/lib/chatStream.ts`、`src/hooks/useChatController.ts`                                                                                                                                                             |
| 执行确认       | Octop 有 HITL 恢复接口；当前 OctopPet 未实现审批事件与 SSE 恢复                                   | [chat/routes.py](https://github.com/TencentCloud/Octop/blob/main/src/octop/api/routers/chat/routes.py)、本地 `src/lib/chatStream.ts` 和 `src/hooks/useChatController.ts`                                              |
| 实例 Bridge    | 用于 Octop 实例之间的专家、聊天等转发，文档明确区别于 Connector/MCP                               | [docs/bridge.md](https://github.com/TencentCloud/Octop/blob/main/docs/bridge.md)                                                                                                                                      |

Octop 证据来自调查时可读取的上游 main，未核实用户远程部署版本。实施前必须按服务器实际版本重新核对接口并固定提交号。

本机验证：使用现有 Skill 的定位脚本找到 `C:/Users/David/AppData/Local/Programs/Xiaozs/ScreenAutomationHelper/ScreenAutomationHelper.exe`；实际执行 `cli status`、`cli describe`、`cli capabilities`，均正常退出并返回 JSON。describe 和 capabilities 的 `ok=true`；capabilities 声明 49 项。status 的任务状态字段是最近任务状态，不等于 CLI 进程是否可用。未执行截图、输入或流程。尚未核实可选组件的实际访问状态，也未取得可靠版本号。

## 3. 最小接入路径

```text
OctopPet ── 原有 HTTP/WebSocket ── 远程 Octop AI
                                      │
                           screen-automation Skill
                                      │
                         Agent 原有命令执行工具
                                      │
                         Skill 附带的 CLI 转发脚本
                                      │
                               受控加密连接
                                      │
                         本机功能组件 → PD CLI
```

组件可以暂名 `screen-automation-octop-component`，名称尚未定稿。本机组件集中负责 CLI 子进程调用、连接生命周期和受控文件回传；远程 Skill 脚本负责转发。无需 MCP 工具声明，也不新增 Octop 专用执行工具；复用 Agent 已有 execute/shell。不得复制 PD 核心或内嵌 Octop 源码。

现有 Skill 的 `$AppCli cli ...` 是本机调用方式，不能把它原封不动放到 Linux 服务器上运行。最小适配是在 Octop 使用说明中明确目标是 Windows 本机，并将调用入口改成随 Skill 分发的转发脚本（例如 `pd-local cli ...`，名称仅为示意）；该脚本不改 CLI 参数语义。公共 Skill 的任务方法、安全规则和流程标准继续共用来源，不能手工维护分叉副本。

### 远程连接建议

原型优先考虑已有 SSH 反向端口转发：本机组件只监听回环地址，本机主动向服务器建连，服务器端转发口也只绑定回环地址，Skill 脚本通过该口转发 CLI 请求。传输是简单的结构化 CLI 请求/响应，不使用 MCP。这不要求修改 Octop，也不要求电脑开放公网入站端口。[OpenSSH -R](https://man.openbsd.org/ssh#R)

用户已确认拥有服务器 SSH 权限，且 Octop 直接运行。实际 SSH 配置、转发许可及服务器执行环境仍待核实，尚未建立隧道。本机接收器应运行在当前登录的 Windows 桌面会话，不能用 Windows SSH 服务的非交互会话代替桌面执行进程。

正式用户安装体验可复用已有私网/转发设施；只有现有设施不满足时，才另议组件自己的设备配对与转发服务。当前不先设计新的云端中继平台。Octop 的实例 Bridge 也不能直接视为本机 CLI 工具通道。

### Skill、转发脚本与 CLI 的职责

- Skill 是方法与使用规范；CLI 是本机执行契约；转发脚本仅改变执行位置。
- 传输 CLI 参数数组、退出码、stdout/stderr 和请求标识，经子进程参数列表调用已配置的 CLI。接收器不提供任意系统 shell、可执行文件路径或任意文件读写入口。
- CLI 截图和结果返回的 Windows 路径不能直接供服务器读取。组件需提供与该请求/任务关联的有限文件回传，远程脚本将实际文件落到 Agent 可读的工作区后交给原有文件/图像工具；不能只把路径字符串转回来。图片回传和多模态读取要单独验收，CLI 成功不证明 AI 已看见屏幕。
- stdin 和敏感值的非回显通道保持 CLI 语义；需要时独立验证，不能偷偷改成命令行或日志明文。
- 查询流程、查看状态、读取结果先用于只读验证；启动和控制任务下一阶段开放。
- 初期以现有流程任务闭环验收。后续需要截图、识别、输入或流程编辑时，继续映射现有 CLI 能力，不另造执行底座。
- 流程、对话、审批和 run_id 分别保存关联，不能混用标识。
- `result latest --workflow` 是流程最近结果，不能声称是指定 run_id 的独立结果。
- 启动调用超时或断线后先查任务状态，不直接重复启动；关闭聊天/取消生成不等于停止 PD 任务。

## 4. 简化与最小补丁

1. 恢复 OctopPet 原有单一 AI 聊天入口，撤回本机关键词对话与模式切换。
2. CLI 调用能力迁入独立组件；对接配置由组件维护，不常驻宠物设置页。
3. 先复用聊天中的工具调用状态与模型文字结果。
4. 执行前确认优先复用 Octop 对 execute/shell 的审批和 Skill 原有授权规则；核实事件后，再为 OctopPet 增加最小审批视图及恢复调用。不能只审批服务器上的转发脚本名称而忽略实际 PD 动作，也不能把模型传来的 user_confirmed=true 当成用户确认。
5. 未完成确认与拒绝链路验证前，只开放查询工具。
6. 自定义结果卡片和宠物任务动画按实际需要另做小补丁；Octop Dashboard 的插件 UI 不代表 Tauri 客户端自动支持。
7. PD 组件中心至少需要目录登记；如需其管理常驻连接，单独评估生命周期接入，不能直接套用浏览器专用启动路由。

## 5. 当前本地改动盘点

基线是本地 HEAD `385d26138871a32f3d821e6128672d47cf94ba0f`，不是声明已与所有上游同步。太极机器人素材和八种动画已在该提交中，简化时保留。

调查前有 20 个已跟踪文件修改（139 行增加、25 行删除）和 12 个未跟踪文件。以下表格保留为清理前的历史盘点；当前执行结果见“界面清理记录”。

| 文件/范围                                                                   | 当前新增用途                       | 建议                                      |
| --------------------------------------------------------------------------- | ---------------------------------- | ----------------------------------------- |
| `src/main.tsx`                                                              | 路由到两模式聊天                   | 恢复原 ChatWindow                         |
| `src/windows/CompanionChatWindow.tsx` 及测试                                | helper/octop 模式选择              | 迁移完成后撤回                            |
| `src/windows/HelperChatWindow.tsx` 及测试、`src/styles/helper.css`          | 关键词对话、流程任务面板           | 撤回独立界面                              |
| `src/components/HelperConnectionSettings.tsx` 及测试                        | 本机路径和模式配置                 | 配置迁至组件                              |
| `src/lib/helperClient.ts` 及测试                                            | CLI 响应映射、关键词意图、状态映射 | 复用 CLI 数据契约；不迁移关键词聊天       |
| `src-tauri/src/helper_bridge.rs`、`helper_cmd.rs`                           | 本机 CLI 调用及 Tauri 命令         | 作为组件适配器参考；移出宠物运行依赖      |
| `src-tauri/src/lib.rs`、`src/lib/tauriApi.ts`                               | 注册 helper 命令与事件             | 移除直接 CLI 接线；动画事件另评估         |
| `src-tauri/src/config_cmd.rs`、`src/lib/types.ts`、`src/lib/configLogic.ts` | helper 配置及默认模式              | 恢复 Octop 聊天默认；已保存配置兼容另核实 |
| `src/windows/SettingsWindow.tsx` 及测试                                     | 新小助手标签、连接说明             | 恢复单一 Octop 设置结构                   |
| `src-tauri/src/tray.rs`、`src/lib/petContextMenu.ts`                        | 模式相关主程序入口                 | 恢复原入口逻辑，组件启动独立处理          |
| `src/windows/PetWindow.tsx`                                                 | 监听本机任务动画事件               | 素材保留；事件来源改为聊天/组件后单独实现 |
| `src/App.css`                                                               | 引入 helper 样式                   | 随独立界面撤回                            |
| `src-tauri/tests/command_logic.rs`                                          | helper 配置断言                    | 随契约调整；组件测试迁至组件仓库          |
| `AGENTS.md`、`README.md`、`README_CN.md`、`CHANGELOG.md`                    | 本机对话产品定位                   | 改为 AI 聊天 + 可选 PD 功能组件           |
| `index.html`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`           | 产品描述与聊天标题                 | 按最终定位调整，无需重构                  |
| `docs/HELPER_COMPANION.md`                                                  | 旧本机对话指南                     | 保留为历史方案或标明被新方案取代          |

## 6. 上游更新记录规则

每个必要补丁记录：补丁编号、基线提交、文件及接口、修改原因、可否移到组件、测试、同步上游时的保留/重做/撤回判断。

在独立组件仓库维护 `UPSTREAM_COMPATIBILITY.md`，记录 Octop 实测版本/提交、PD CLI 版本与协议、OctopPet 必要补丁及组件发布版本。当前文档是调查与现状盘点，不是已实施补丁清单。

实际撤回前先保存完整本地差异（包括未跟踪文件）。禁止整仓覆盖、直接 hard reset 或用整文件替换丢失无关修改；按本表逐项处理。代码改动完成后运行 `make all`，Windows/Mac 的窗口、焦点与透明度分别进行真机验收。

## 7. 分阶段验收

1. **只读原型**：组件调用已安装 CLI，Skill 脚本传回 status、describe、capabilities；隧道连通后远程 Octop 通过现有 execute 读取同一结果。不会运行业务流程。
2. **屏幕观察**：在用户指定的无敏感内容窗口验证图片回传、目标身份和 AI 实际读取图片；不把 CLI 返回的本机图片路径当成已观测。
3. **任务闭环**：用户确认/拒绝可见；一次确认只启动一次；按 run_id 查状态和控制；断线重连不重复执行；结果语义准确。用专用无敏感内容流程验证。
4. **界面收敛**：切回原 AI 聊天，删除已迁走的重复入口；审批可用，停止生成与停止任务区分清楚。
5. **组件分发**：签名包、安装、更新、回滚、卸载、配置保留和明确连接开关；不擅自注册自启动。

当前待确认：远程 Octop 的实际版本与 Agent 执行配置、SSH 连接配置及转发能力、组件最终名称。服务直接运行和用户有 SSH 权限已确认。已只读检查服务健康与本机 CLI，尚未实施上述接入步骤，也未部署服务或运行 PD 业务任务。

## 8. 界面清理记录（2026-10-06）

- 用户明确要求清理此前新增界面。本轮以 HEAD `385d261` 为基线，逐项恢复盘点中的 20 个文件，移除 12 个仅服务于本机 helper 模式的文件。
- 恢复 `src/main.tsx` → 原 `ChatWindow`，移除 helper/octop 模式切换、小助手设置标签、任务控制面板和直接调用 CLI 的 Tauri 命令。
- 撤回托盘、右键菜单及产品描述中与本机 helper 模式有关的改动。恢复基线默认值；已有用户配置中的宠物选择保持不动。旧 helper 字段不再使用，不清空 Octop 连接配置或 keyring。
- 保留 HEAD 中的太极机器人素材、八种动画、原有 Octop 聊天能力及本方案。组件尚未实现，不声称已有远程控制能力。
- 清理前完整备份 32 个文件、tracked.patch 和基线清单到 `.cache/helper-ui-before-cleanup-20261006.zip`，已校验 ZIP 完整性；备份未加入 Git。
- 本轮不提交、不推送、不改远程 Octop。
- 验证通过：Prettier 格式检查、ESLint、TypeScript 类型检查、Vitest（17 个文件、105 项测试）、cargo fmt --check、cargo clippy -- -D warnings、cargo check、cargo test，以及 npm run build。本机没有 make，因此逐项执行 Makefile 中 make all 的等价步骤，未声称运行过 make all 命令。
- 搜索确认源代码、产品说明和命令注册已无独立 helper 聊天接线。Mac 窗口/透明度仍需 Mac 真机验收；远程 PD 工具连接与 AI 审批不属于此次清理的已验证功能。

## 9. 同一聊天连接桥接（2026-10-07，当前实施方案）

用户要求：后台桥接必须复用宠物聊天的同一条 WebSocket；不使用 SSH、MCP 或另一条设备网络连接。

调用链：宠物发送普通 user_turn（附只读桥接能力）→ Octop 私有 Agent 的 Skill → 服务端进程内桥接适配器 → 原聊天 WebSocket 的 pd_cli_request → 宠物确认 → 独立本机组件 → 已安装 PD CLI 的 cli window list-visible → 原 WebSocket 的 pd_cli_result → Skill 返回 JSON → AI 在聊天中解释窗口列表。

服务器内部 Skill 通过 127.0.0.1 临时端口调用同进程适配器；这是服务器内部通信，不是另开到用户电脑的连接。本机不监听端口、不需要公网 IP、不注册自启动。

### 首版范围与边界

- 只支持 Windows 可见窗口列表。没有点击、输入、截图或任意命令执行。
- 每次读取在聊天窗口确认，拒绝不会执行 CLI。标题只返回前 128 字符，最多 200 个窗口。
- 请求绑定原连接和 thread_id；同用户同 Agent 同时存在多设备连接时拒绝选择设备。
- 首版仅绑定用户自己拥有的私有 Agent。共享 Agent 尚不支持，以免把其他用户设备带入工具调用；这不是无需配置产品的最终交付状态。
- 本机数据仅通过 WSS 或本机回环 WS 传输。目前 http://47.77.184.54:8088/ 必须先配置 HTTPS/WSS，普通聊天不受影响。
- 远程 Octop 必须安装独立适配器和 Skill，并应用小补丁。单独安装 Skill 无法使原 WebSocket 具有设备请求能力。
- 原型未部署到远程服务器，未验证实际远程 AI 能调用本机。服务器原版本、运行方式和工具配置仍需实际部署时核对。

### 更新记录与代码边界

独立工程：D:\ai\screen-automation-device-bridge。原来的 HTTPS 轮询原型已退役，备份在该工程 build/retired-polling-prototype.zip。

OctopPet 只新增 src/lib/pdChatBridge.ts（消息识别）、src-tauri/src/pd_bridge_cmd.rs（启动独立组件），并在 useChatController.ts、tauriApi.ts、lib.rs 各自现有职责处接线。没有增加第二套聊天界面、PD 设置页或 Octop Python 代码。

独立工程包含本机 Rust 可执行组件、pd-local-windows Skill、服务端 pd_chat_bridge.py 和补丁生成器。服务端只改 src/octop/api/routers/chat/ws.py 的导入、结果消息分支、私有 Agent 绑定、断线清理四处。补丁生成器检查源码锚点并记录原文件 SHA-256；上游变化时停止生成，需要重新审阅，不自动覆盖运行文件。

组件正式签名、外部目录发布和跨平台包还未完成。本轮输出是开发测试包，不宣称已进入 PD 官方组件目录。

### 本轮验证与交付

- 完成 make all 等价步骤：Prettier、ESLint、TypeScript、cargo fmt/check/clippy/test。当前 Windows 未安装 make，未直接执行该命令。
- 前端 18 个文件、112 项测试通过；Rust 3 项单元和 20 项集成测试通过。
- 独立本机组件 3 项 Rust 测试通过；服务端 5 项测试通过，包含 Skill 脚本经服务器回环接口、同一聊天发送器的往返。
- 实际已安装 CLI 查询成功，得到 9 个可见窗口；验证日志不输出窗口标题。
- 服务端候选补丁可编译，上游源码锚点变化和重复打补丁均拒绝。
- 开发测试包：独立工程 build/pd-device-bridge-windows-x64-dev.zip 与 build/pd-chat-bridge-server-dev.zip；已校验 ZIP 和 SHA-256。不是正式签名组件包，不是远程部署完成证明。
- 未提交、未推送，未修改 PD 主仓库和远程服务器。原生确认框的人工交互与真实远程 AI 完整流程仍待联调。

### Linux 1.0.2b6 部署适配

用户提供实际版本 1.0.2b6 与 site-packages 路径。上传安装工具新增 --installed/--inspect，使用服务同一 Python 定位包；针对官方 1.0.2b6 wheel 成功生成候选补丁。实际 Agent 的 Skill 位于工作目录内部 .octop/skills，安装工具与 Skill 凭据定位已修正并验证；旧 skills 布局仍支持。服务端 6 项测试通过，上传 ZIP 的模拟嵌套目录安装及原文件备份验证通过。服务器尚未应用补丁，待用户核对当前聊天 Agent ID 与私有所有权。

## 10. 按需小助手 CLI 代理 v2（当前实现）

只在原 screen-automation Skill 调用小助手 CLI 时转发，不代理普通 execute/其他工具。无独立窗口 Skill，不要求指定 Agent ID。服务器安装两个独立模块与同一 ws.py 四处接线，协议为 pd-chat-cli@2/helper.cli；客户端在原 user_turn 广告能力，原连接回传 CLI stdout/stderr/exit_code。

私有 Agent 的聊天连接登记后自动发现工作目录 .octop/skills 或 skills 中 screen-automation* 的 resolve_cli.sh；只增加 Linux 分支，指向工作目录 .octop/pd-chat-bridge/pd-helper，保留原文件 .before-pd-bridge。新 Skill 以后同样发现；版本更新有额外备份后缀。原 Skill 内容和 Mac 分支保留。每次登记不启动本机 CLI，只有代理被 execute 调用才发本机请求。

每次请求确认显示完整参数、明确结果及截图传往当前远程聊天。两端及独立组件只允许启用的 PD CLI 家族，固定程序路径，无 shell。已启用 status/describe/capabilities/health/access list 及 window/screen/mouse/keyboard/ocr。workflow、任意系统命令、文件上传等未开放，明确拒绝，不回退服务器执行。

截图仅接 screen capture 的 PNG 输出，最多 4 MiB，随同一聊天连接回传；代理保存到当前服务器工作目录并替换 CLI JSON 的 path。传输测试使用模拟图片；不宣称真实模型已看图。普通 stdout/stderr 各限制 1 MiB，CLI 执行最多 60 秒。

当前仍限 Windows 本机、私有 Agent 和单个服务器进程。共享 Agent、自定义隔离执行环境、Mac 与正式签名分发未完成。官方 1.0.2b6 ws.py 候选补丁已编译，但实际服务器仍未应用。组件入口版本仍为开发版 0.1.0，以协议 v2 区别旧包。所有实际安装、确认框交互、鼠标键盘屏幕动作与远程 AI 联调待人工验收。

### v2 验证记录

前端 18 文件 115 项测试通过；独立组件 2 项 Rust 测试与静态检查通过；服务端 8 项测试通过，覆盖普通聊天不触发 CLI、原连接与线程绑定、参数/输出/退出码、模拟截图服务器路径、现有 Skill 入口备份及幂等接线。实际安装的小助手 cli status 和 cli window list-visible 已通过新组件执行，未记录状态内容或窗口标题。新版上传 ZIP 不含独立 Skill，安装模拟验证无需 Agent 参数，预览不改文件、应用备份原源码、拒绝重复安装。实际云端与桌面输入动作未测试，服务器尚未部署。

### 现有 HTTP/WS 配置（当前要求）

用户明确要求保持 http://47.77.184.54:8088/，桥接不得要求更改端口、增加公网连接或配置 HTTPS。已取消前端仅 WSS/回环 WS 的限制，现有 WS 与将来的 WSS 使用相同协议和调用路径；逐命令确认、CLI 范围、会话绑定和普通聊天不执行 CLI 均保留。先前文档中的 WSS 必须条件已废止。实际远程联调仍待服务端部署。

### 只读查询体验

status/describe/capabilities/health/access list/window list-visible 无额外弹窗；操作和截图仍确认。本机 Tauri 调用错误及 CLI 非零退出原因展示在原聊天错误区。PD CLI 可以独立运行，status.running 是任务状态，不据此误报桌面未启动。没有新增界面、启动检查进程或服务器补丁。

### 移除桥接授权框（当前行为）

用户明确要求不设计桥接授权提示框。已删除 confirmPdCli、确认回调、只读分类和审批等待；窗口激活、输入和截图等已启用 CLI 请求直接执行。保留固定小助手入口、参数/家族限制、连接/线程绑定、去重、断线检查、错误提示及普通聊天不调用 CLI。之前各节关于桥接确认的要求均被本节替代；不修改 PD 产品自身的权限逻辑，不改变服务器。

### browser CLI 桥接修复

此前 browser 未加入四层命令范围，导致服务器 HTTP 400，再被代理错误折叠为 unavailable。已补齐前端、Tauri、独立本机组件、服务端校验；代理保留 HTTP 错误信息和本机 CLI 失败原因。browser 由已安装 PD 产品执行，浏览器扩展授权和依赖以产品实际返回为准。提供 update_bridge.py 仅更新两个独立服务器模块并备份，不重打 ws.py 补丁，不更改服务地址/端口。真实云端浏览器操作待用户更新后验收。

浏览器参数还支持原 Skill 的 browser 顶层短命令；独立本机适配器仅对该入口补齐 Windows 产品所需的 cli 前缀，已有 cli browser 不重复补齐。真实已安装 CLI 的 browser status 通过，退出码 0；未执行网页导航或写操作。前端 127 项、Rust 24 项、独立组件 3 项、服务端 10 项测试通过。上传包更新工具模拟验证预览不修改、更新备份两个模块、ws.py 字节保持原样。实际服务器需用户上传并更新两个模块后重启，再进行真实远程验收。
