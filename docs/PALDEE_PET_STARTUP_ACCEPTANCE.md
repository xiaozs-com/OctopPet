# Paldee Pet 0.2.2 打开与联动启动验收

2026-10-08。本轮保留 0.2.1 包及先前记录；新版本才增加打开和可选联动启动声明。

设置名称：随“屏幕自动化小助手”启动。默认关闭，保存后下次启动主程序生效。仅小助手主程序启动时联动，不修改 PATH、Windows 自启动或开机任务。关闭主程序不关闭独立桌宠。不开启时不启动或下载任何组件。

Pet 修改：scripts/package_pd_component.py、scripts/test_pd_component.py、根版本文件、CHANGELOG、组件及同步文档。内部标识和独立用户数据不变。
小助手独立仓库修改前基线：43913008；记录时当前提交 ad49a8ee 已包含这四个文件的相同改动（本会话未执行 commit 或 push），不可重复应用补丁；修改 components/manager.py（传递协议）、components/operations.py（固定 JSON 生命周期调用及可选启动）、gui/main_window.py（异步打开和一次性启动钩子）、tests/test_component_application_startup.py（回归）。没有复制源码入 Pet 或复制 Pet 源码入小助手。

手动“打开”执行 start/show；联动启动只执行 start。失败不会阻塞主窗口，也不会妨碍其他已开启的组件。打开使用已安装清单入口，核对 JSON 协议/版本/ID、退出码、运行状态和 PID，不运行任意 shell。

小助手相关测试：30 项组件测试通过；37 项可选组件/外部目录测试完成，35 通过、2 跳过。本轮新增的 11 项启动和设置回归包含默认关闭、损坏偏好、版本/协议错误、超时、失败隔离及独立配置持久化。

签名和文件准备仍在本地产品发布目录进行。没有服务器上传、源码提交、push 或主程序安装包发布。旧主程序必须更新调用层；源码小助手重启才能加载变化。远程 Octop 和业务屏幕操作不是本轮验收目标。

真实界面联调发现启动/状态并发时旧管道实例可能尚被客户端持有，限制为 2 会导致创建下一监听失败并退出。运行协议改用系统默认管道实例上限，仍只保留一个待连接监听；新增十轮 start/status/status 并发验收，首实例所有权和当前用户 ACL 不变。

## 最终结果与产物

Windows 等价 make all 通过（前端 130、Rust 27）；并发修复后 Rust clippy 与 27 项测试复测通过。实际 ZIP 的 12 项验收全部通过（43.277 秒），包含五轮并发冷启动和十轮 start/status/status 并发。真实 Tk 组件页与实际签名安装共 10 项联调通过，实际点击“打开”和“保存配置”，验证默认关闭、保存、主程序启动钩子、隐藏窗口保持、关闭偏好及卸载保留数据。

本机组件通过现有签名安装器升级为 0.2.2，用户启动偏好没有修改。源码小助手需重启加载调用层。两份 JSON 已受信任验签，Cua 保留；网站本地目录已准备，服务器未上传。

- `D:\ai\xiaozs.com\xiaozs\sah\components\paldee-pet\0.2.2\paldee-pet-0.2.2-windows-x64.zip`：15379080 字节；SHA-256 `53b1af11664c6ac609afc580f01f1ab1b5dbc761dcbe889d6d5ffa273c4643db`。
- `D:\ai\xiaozs.com\xiaozs\sah\components\paldee-pet\latest-windows-x64.json`：635 字节；SHA-256 `bee8b094933577a4d76373f0934243e4473fbe709f5e7b6bad475d626a9cfa61`。
- `D:\ai\xiaozs.com\xiaozs\sah\components\catalog.json`：2067 字节；SHA-256 `043d62387dfee3c51e0334ef921b3fadea134e6212535c74e07e9ef0fb66170e`。

旧 0.2.1 ZIP 和已验收记录保留。失败的未发布 0.2.2 候选包和对应签名清单仅归档到缓存/暂存备份，不能上传。当前最终签名和联调报告位于 D:/ai/xiaozs.com/component-release-staging/paldee-pet-0.2.2/。Pet 构建目录中的 latest 仍是 unsigned 构建产物；上传应使用网站对应本地目录中的已签名文件。
