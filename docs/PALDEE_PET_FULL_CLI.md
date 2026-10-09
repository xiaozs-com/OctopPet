# Paldee Pet 0.2.3 完整 CLI 桥接

2026-10-09。本轮取消服务器、Pet 前端、Tauri 和独立桥接的命令类别白名单；保留固定小助手入口、参数类型/长度、聊天会话匹配及资源限制。未来 CLI 子命令自动透传。真实错误由本机 CLI 返回，不替换为“超出桥接支持范围”。

新增可选标准输入及文件包，仍用原 pd-chat-cli@2 / WS/WSS；有 IO 的方法为 helper.cli.io，避免旧客户端静默丢失输入。没有改 Octop ws.py 格式、服务地址、端口、Agent 或 Skill 格式。包格式仍遵循原外部组件规范，ID octoppet、入口 paldee-pet.exe、协议 octoppet-component@1 不变。

修正旧连接八次已完成调用后静默拒绝：最多八个并行操作，完成结果保留 128 项去重缓存。顺序连续操作不受八次限制。

Pet 修改：src/lib/pdChatBridge.ts、pdChatBridge.test.ts、tauriApi.ts、src-tauri/src/pd_bridge_cmd.rs、根版本/锁文件、文档与源码补丁。独立桥接修改：native/src/lib.rs、main.rs、新增 transfer.rs；server/pd_chat_bridge.py、pd_cli_proxy.py及测试、README、升级说明和独立服务器包脚本。桥接源码留在 D:/ai/screen-automation-device-bridge，不复制入小助手或 Pet 运行源码。

流程安装/检查时服务器目录自动上传；既有本机流程用 workflow show 获得目录，再 --pd-download 取回源码。输入 *-file 参数自动传文件，其他输入用 --pd-upload INDEX=PATH；标准输入 UTF-8 透传。单次文本最多 1 MiB、文件最多 128 个/4 MiB；成功副本保存在独立用户目录 bridge-transfers，供异步操作继续使用，卸载组件保留。

具体用法见独立桥接 FULL_CLI_USAGE.md；服务器更新见 SERVER_UPGRADE.md。上传后使用 /home/admin/.octop/venv/bin/python update_bridge.py 预览，停止原服务后加 --apply 更新，再按原方式启动并重新聊天。仅更新已有模块，不重打 Octop 补丁。两个端点都需本轮版本；未上传前不能声称远端已经完整支持。

签名在本地受控产品目录使用原密钥；本仓库继续生成未签名版本清单及自身目录条目，不生成全局 catalog.json，不改 Cua 或产品签名私钥。旧版本 ZIP 保留。小助手启动设置及眼睛动画保留。

## 最终验证与交付

Windows `scripts/check-windows.ps1` 全绿：前端 140 项、Pet Rust 27 项；独立桥接 Rust 6 项、服务器与真实 CLI 18 项、最终 ZIP 生命周期验收 12 项通过。真实 CLI 验证包含流程 schema/inspect/install/show/validate、流程源码取回和中文标准输入写入/读取。ZIP 验证包含中文及空格路径、重复启动、正常停止、异常退出、独立用户数据保留。

最终 Windows ZIP：15,426,748 字节，SHA-256 `5a8f39c78465c4660634db40217e07d0be3e7bfd5a7aa0f52a9b40b82be9781c`。

服务器更新 ZIP：12,155 字节，SHA-256 `a78a94c0fe367be13992d79132a3c3368c1ed227d3d4cf805d13078ccfd6d9ff`。

本地网站待上传文件在 `D:/ai/xiaozs.com/xiaozs/sah/components`，签名与哈希报告在 `D:/ai/xiaozs.com/component-release-staging/paldee-pet-0.2.3/signing-verification.json`。已验证产品签名并保留 Cua 原条目；未上传、未 push。服务器安装和真实宠物到远端 Octop 的联调仍待手动部署；现有已安装的旧 Pet 不会因本地构建自动升级。
