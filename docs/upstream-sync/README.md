> 2026-10-08：组件发布方案已按用户提供规范整改，当前以 [OctopPet 标准组件记录](../OCTOPPET_COMPONENT.md) 为准。本文中的 pd-device-bridge 宠物包 ID、全局目录合并和旧上传包仅为历史记录。

# PD 扩展：原源码修改与上游同步记录

当前日常同步请使用 [Paldee Pet 上游同步流程](PALDEE_SYNC.md)。下列基线表为历史记录，不代表当前 Git 状态。

记录日期：2026-10-08。本文记录最终实现，更新时以实际源码、补丁和测试为准；之前的界面合并、SSH 隧道、独立窗口查询 Skill、强制 WSS、授权弹窗方案均已撤回。用户已报告桥接初测通过且组件启动问题解决；本文编写时未重新登录服务器核实部署状态。

## 1. 项目与基线

| 对象                  | 位置 / 基线                                                     | 状态                                                                                                                        |
| --------------------- | --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| OctopPet              | D:\\ai\\OctopPet；HEAD 385d26138871a32f3d821e6128672d47cf94ba0f | 桥接改动仍在工作区，尚未提交或推送                                                                                          |
| 太极素材前的 Pet 基线 | e0ded53e8f36f309e2b2105ef163824886e0da6a                        | 当前可追溯父提交；不是当前远端最新版证明                                                                                    |
| Pet origin            | https://github.com/xiaozs-com/OctopPet.git                      | 当前仅配置 origin，没有 upstream；浅克隆，不能把 origin 自动当官方上游                                                      |
| 服务器 Octop          | 用户最近确认 1.0.2b6；/home/admin/.octop/venv                   | 官方一键安装/PyPI 方式；运行用户 admin                                                                                      |
| 独立组件工程          | D:\\ai\\screen-automation-device-bridge                         | 服务端模块、代理、补丁生成器、安装/更新工具、本机组件；尚未单独 Git 管理，需要保存完整工程；不放入 Pet 的 Python 服务端源码 |

同步前先确定上游真实仓库与分支。本文不新增远端、不 fetch、不合并、不提交。浅克隆在缺少对象时需要补充历史，不能用未知 merge-base 推断完整改动。

## 2. Octop 原源码：只改一个文件、四处接线

仓库路径：src/octop/api/routers/chat/ws.py。

服务器已确认安装路径：/home/admin/.octop/venv/lib/python3.12/site-packages/octop/api/routers/chat/ws.py。Python 升级时此路径可能变化，必须重新 inspect。

| 位置 / 标记                                               | 改动                                                                        | 更新后必须核对                                                                              |
| --------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| WS_CHANNEL_ID 导入附近；PD_CHAT_BRIDGE_PATCH              | 可选导入独立 pd_chat_bridge.broker，未安装时保留普通聊天                    | Python 模块加载位置；不要吞掉其他实际异常                                                   |
| dashboard_chat_ws 消息分派                                | 消费 pd_cli_result，交给 broker.complete(connection_id, payload)            | 插入正确的已认证 dashboard 连接，不是另一个 bridge 路由                                     |
| prepare_dashboard_turn 成功后、build_dashboard_inbound 前 | 识别 pd_bridge 提示，绑定 user/agent/thread/connection/send_frame/workspace | 私有所有权检查 row.user_id == user.id；registry 方法、prepared.thread_id 和 send_frame 接口 |
| finally 中 hub.unregister 前                              | broker.disconnect(connection_id)                                            | 断线释放待调用，保留上游原来的流式恢复行为                                                  |

协议是 pd-chat-cli@2，操作 helper.cli；请求/结果是 pd_cli_request / pd_cli_result。继续使用原 WS/WSS，不增加公网设备端口，不要求当前 HTTP 服务改成 HTTPS。普通聊天不执行本机 CLI。

服务器独立新增两个模块：

- site-packages/pd_chat_bridge.py：连接登记、服务器内部回环调用、校验、结果与生命周期。
- site-packages/pd_cli_proxy.py：原 Skill 的 CLI 代理；返回 stdout/stderr/exit_code，截图回传后替换为服务器文件路径。

这些不是 Octop 包内部的新目录，不修改模型 Provider、Agent 核心、全局 execute 或数据库结构。服务器内部回环端口不是到用户电脑的新连接。

安装时用户报告的原文件备份：
/home/admin/pd-chat-bridge/install-records/20261007T152911711660Z/ws.py.original。

补丁工具与真实安装记录：

- packaging/patch_octop.py 检查四个源码锚点、拒绝重复补丁、编译候选文件，输出 patch、候选 ws.py 和原源码指纹。
- install_server.py --installed 只预览；--apply 首次安装并备份。
- update_bridge.py 只更新两个独立模块并备份，要求 ws.py 已有补丁。它不会重打源码补丁。
- 参考补丁见 snapshots/octop-chat-bridge.patch。参考指纹来自本地官方 1.0.2b6 文件，实际服务器的 install-records 为部署依据；不能拿参考指纹冒充重新核对服务器。

## 3. OctopPet 原源码修改清单

### 3.1 桥接接线：三个已有生产文件

| 文件                           | 修改位置与目的                                                                                                 | 上游变化后迁移点                                                                           |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| src/hooks/useChatController.ts | 导入桥接；创建原 socket 后绑定处理器及 setError；原 user_turn 附 pd_bridge；消息进入流 reducer 前先处理 CLI 帧 | 若聊天控制器拆分，按“创建连接、发送 user_turn、接收帧、显示错误”四个职责迁移，不整文件覆盖 |
| src/lib/tauriApi.ts            | 新增 pdExecuteCli(args) 调用封装                                                                               | 保持 invoke 只在 API 层；TS 参数与 Rust 对齐                                               |
| src-tauri/src/lib.rs           | 模块声明和 pd_execute_cli 命令注册                                                                             | 保留上游插件与注册列表，追加所需项                                                         |

新增独立文件：

- src/lib/pdChatBridge.ts：协议、参数范围、请求去重、线程匹配、原连接回传及错误原因。
- src-tauri/src/pd_bridge_cmd.rs：仅 chat 窗口允许调用；启动独立组件；超时/输出限制；定位 versions/<version> 目录。
- src/lib/pdChatBridge.test.ts：普通聊天不执行、顶层 browser、错误提示、无授权框、去重与断线等测试。

其他修改：

- src/windows/ChatWindow.test.tsx：WS/WSS 原连接往返、本机不可用时可见错误；测试 WebSocket 常量。
- CHANGELOG.md：界面清理、按需桥接、取消授权框及 browser 修复。
- docs/PD_COMPONENT_INTEGRATION_PLAN.md：演进记录；最终行为以最后追加章节和本同步文档为准。

当前没有独立小助手聊天界面、第二条设备连接、桥接授权提示框或强制 HTTPS 条件。没有改 OctopPet 的登录、keyring、模型选择和原聊天流 API。命令限制保留，不代表所有 PD CLI 家族已开放。

### 3.2 已提交的太极机器人改动

提交：385d26138871a32f3d821e6128672d47cf94ba0f。

| 分类                    | 文件                                                                  |
| ----------------------- | --------------------------------------------------------------------- |
| 允许新宠物名称          | src-tauri/src/config_cmd.rs、src/lib/types.ts                         |
| 设置列表 / 默认配置识别 | src/windows/SettingsWindow.tsx、src/lib/configLogic.ts                |
| 图片与动画呈现          | src/components/MascotImage.tsx                                        |
| 新增动画逻辑 / 测试     | src/lib/mascotAnimation.ts、src/lib/mascotAnimation.test.ts           |
| Rust 配置测试           | src-tauri/tests/command_logic.rs                                      |
| 资源与说明              | public/mascots/taiji-bot.png、public/mascots/taiji-bot/、CHANGELOG.md |

该提交共 35 个文件，素材含 PNG/WebP、8 种动画、manifest 和预览验证工具。资源已在 Git 中；桥接快照不重复装入这些二进制资源。同步时单独保留/迁移这个提交；若上游已含同等改动，不重复应用。

检查完整范围：

```text
git show --stat 385d261
git show 385d261 -- src/components/MascotImage.tsx src-tauri/src/config_cmd.rs
```

## 4. 运行时 Skill 修改：必须单独留意

Octop 原库外，独立模块 provision_cli 会修改 Agent 工作区已有 screen-automation* Skill 的 scripts/resolve_cli.sh：

1. 在工作目录 .octop/skills 或 skills 查找该 Skill 的解析脚本。
2. 增加标记 PD_CHAT_CLI_RESOLVER_V2 和 Linux 分支，返回当前 Agent 的 .octop/pd-chat-bridge/pd-helper。
3. 保存 resolve_cli.sh.before-pd-bridge；后续不同原版本保留带原内容哈希后缀的备份。
4. 不改 Skill 的其余脚本、说明或 Mac 分支，不新增独立窗口 Skill。
5. 私有 Agent 下一次宠物聊天连接时重新发现新安装的 Skill，并更新代理脚本副本。

工作目录根还生成 .pd-chat-caller.json；它含临时凭据，绝不能进入补丁、Git、截图或支持附件。

原 Skill 更新替换了解析脚本时：保留备份，下次新聊天会重接。若新 Skill 不再采用 resolve_cli.sh，或目录改名不以 screen-automation 开头，必须迁移发现逻辑，不能宣称自动兼容。

本机 PD 主仓库未因桥接修改源码。浏览器产品自己的组件状态和权限仍由 PD 决定。独立本机组件允许 cli 与 browser 入口；browser 短命令在调用 Windows 程序前只补一次 cli 前缀。

## 5. Pet 更新步骤

### 更新前

1. 保存当前工作区；不要直接 reset/覆盖。Git status 中可能存在 CRLF 噪声，使用实际 git diff 检查，不批量恢复全部 M。
2. 生成新的桥接补丁和指纹：

```text
python scripts/export_pd_bridge_patch.py
```

3. 保存 docs/upstream-sync/snapshots 下补丁、JSON、本同步文档和独立组件工程。快照含三个新增源码文件，避免只保存 git diff 而漏文件。
   导出器默认基线固定为桥接前的 385d261，避免提交桥接后相对 HEAD 导出空补丁。以后上游同步后应使用 python scripts/export_pd_bridge_patch.py --base <新基线提交>；文件拆分时同步维护导出器的文件清单。导出脚本位于 scripts/export_pd_bridge_patch.py，本身属于维护工具，不是运行时桥接代码。

4. 桥接尚未提交；以后可按用户授权单独提交到扩展分支。本轮不自动提交。

### 在隔离分支迁移

- 确认真实上游后再 fetch，使用新分支/工作树测试，不直接改当前运行的 main。
- 上游已经包含太极提交时跳过；否则迁移 385d261，保留素材与配置兼容。不要把旧版上游整体覆盖回去。
- 在新工作树检查桥接快照：

```text
git apply --check <快照绝对路径>/octop-pet-bridge.patch
```

- 检查成功后应用；若上下文冲突，先审阅差异，按表中职责迁移。可在隔离工作树用 git apply --3way 辅助处理；禁止用旧 useChatController.ts、lib.rs 整文件替换上游新代码。
- 快照基线是当前 HEAD，不是永远适用的补丁。同步验证后，显式指定新的桥接前基线重新导出；原始快照保留在备份里。

### 验证与接入

项目要求 make all。Windows 没有 make 时完整执行其等价项：

```text
npx prettier --write .
cargo fmt --manifest-path src-tauri/Cargo.toml
npx prettier --check .
npm run lint
npx tsc --noEmit
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo check --manifest-path src-tauri/Cargo.toml
npm test
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

只在测试通过后切换开发/运行版本；保留旧版本以便回退。Mac 窗口与透明度仍需 Mac 真机验证。

## 6. Octop 升级步骤

1. 停止 Octop 服务，备份运行中的 ws.py、两个独立模块和 install-records。Agent 工作区正常保留，凭据文件不导出。
2. 按官方原方式升级 Octop；不要把升级前整个 site-packages/octop 复制回来。
3. 重新执行 install_server.py --installed --inspect，确认版本、Python 和实际路径。HTTP 8088 服务配置保持原样。
4. 对升级后的原 ws.py 生成候选补丁：

```bash
/home/admin/.octop/venv/bin/python packaging/patch_octop.py \
  --source <升级后实际路径>/octop/api/routers/chat/ws.py \
  --output install-records/rebase-新版本号
```

5. 锚点变化、已有标记或编译失败就停止，按第 2 节审阅四处语义；编译成功仍需检查 registry/所有权/thread/send_frame 接口，锚点不是完整兼容证明。
6. 若独立模块还存在：先备份升级后的原 ws.py，审阅后仅将新生成 ws.py.patched 放到实际 ws.py；然后运行 update_bridge.py 预览/更新两个模块。它必须在源码补丁恢复后才能运行。
7. 若整个环境重建且两个模块都缺失：使用最新包的 install_server.py --installed 预览，再用 --apply 首次安装。若只有一个模块缺失，先停止并核对恢复，不强行套用更新工具。
8. 注意：当前 install_server.py 会拒绝覆盖已有模块；它不是“升级后自动重打补丁”的万能工具。不能在模块还在时盲目重复 --apply。
9. 按原方式启动服务，重新发起宠物聊天以生成新进程的调用凭据、刷新 Skill 代理。

服务器原文件恢复示例需在确认候选文件和备份目录后由管理员操作；不自动修改实际服务器。本机桥接程序不需要 SSH，服务器维护通过用户现有管理方式。

## 7. 验收、回退及记录维护

最小验收：

- 普通聊天正常，且没有本机 CLI 调用。
- 只读状态 / 可见窗口返回结果，无授权弹窗。
- window activate 与 browser status 请求使用原连接；browser 短入口不被误拦。
- 小助手缺失、产品错误或桥接连接异常有明确提示。
- 断线/重复帧不重复执行；线程与私有 Agent 绑定仍有效。
- 模拟 PNG 回传保留字节并返回服务器可读路径；实际屏幕/网页操作使用用户指定的测试窗口验收。

回退：

- Pet：切回保存的完整旧版本，而不是仅退前端或仅退组件。
- Octop：停止服务，恢复同一版本安装记录里的 ws.py 与对应独立模块，按原方式启动。不能拿 1.0.2b6 的 ws.py 覆盖更高版本。
- Skill：按对应 .before-pd-bridge 备份恢复 resolve_cli.sh；不删除整个 Agent/Skill 目录。
- 桥接、Pet 与组件协议必须配套；当前为 pd-chat-cli@2。

当前代码验证：前端 130 项、Pet Rust 24 项、组件 Rust 3 项、服务端 10 项测试及构建通过；用户报告初测成功。新版本更新后必须重跑，不继承本次测试结论。

每次修改或同步后：更新此文的版本/文件表/测试结果，重导出 Pet 快照；独立组件目录重新生成服务器参考补丁、指纹和开发包，保留实际服务器的安装记录。

## 8. 本轮资料快照

.cache/PD-source-sync-20261008.zip 包含本同步文档、补丁/指纹、导出工具及独立组件源码（含 Cargo.lock）；不含凭据、运行日志、target、二进制测试包或整个 Pet 仓库。它不是完整 Pet 备份，太极资源仍需保留 Git 提交及仓库。正式版本管理尚未提交/推送，本轮未改变分支、Git 索引或实际服务器。

## 9. Windows 本地提交检查（2026-10-08）

在没有 make 的 Windows Git Bash 环境，正常 pre-commit 钩子调用
`scripts/check-windows.ps1`，执行与 make all 相同的格式、lint、TypeScript、
Rust 编译和前后端测试；不跳过检查。Rust 使用已有离线依赖缓存，缓存缺失时停止。
其他平台仍使用 make all。可以手动执行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/check-windows.ps1
```

本轮准备本地提交 PD 桥接源码、测试和上游补丁记录，保留原来的太极素材提交；
不推送、不发布。前文的 385d261 是桥接前基线，不应在新提交后当作当前 HEAD。
独立组件工程现纳入本地 Git；install-records、build、凭据及二进制保留本地且不提交。
本地检查不能代替远端 Octop 部署与实际账号会话验收。

### 后续默认眨眼修正（2026-10-08）

在原太极素材提交之后，src/components/MascotImage.tsx 改为默认固定 idle.png 主体并以 SVG/CSS 仅眨眼，新增 src/styles/mascot.css 和 src/components/MascotImage.test.tsx。更新时除 385d261 素材提交外，还需迁移这三处改动、CHANGELOG 和素材说明。bridge.patch 不包含此独立视觉改动，见 snapshots/taiji-idle.patch。原 PNG/WebP 未编辑，不新增图片生成依赖。

视觉补丁可用 python scripts/export_pd_bridge_patch.py --mascot 重新导出；上游同步后同样显式指定 --base。桥接和眨眼补丁互不覆盖生产文件；CHANGELOG 由桥接快照保存完整当前变更。

## 10. 功能组件交付（2026-10-08）

桥接源码已在本地提交 53b675d；385d261 继续作为桥接前的导出基线。眨眼和本次组件交付改动仍在工作区，未推送。Pet 安装和更新复用小助手的签名组件机制，详见 [组件交付记录](COMPONENT_RELEASE.md)。小助手仅补充通用应用启动入口，已独立保存任务补丁；以后 Pet 发新版不需要重复修改小助手源码。
