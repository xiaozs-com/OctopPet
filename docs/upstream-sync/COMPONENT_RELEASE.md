> 2026-10-08：组件发布方案已按用户提供规范整改，当前以 [OctopPet 标准组件记录](../OCTOPPET_COMPONENT.md) 为准。本文中的 pd-device-bridge 宠物包 ID、全局目录合并和旧上传包仅为历史记录。

# 桌面宠物组件的安装、更新与源码同步

## 用户入口

屏幕自动化小助手 → 功能组件 → AI 桌面宠物 → 在线安装 → 打开。
以后在同一个位置检查更新和卸载。更新/卸载前从宠物托盘退出，避免 Windows 锁住正在运行的程序。

一个组件包同时包含 OctopPet.exe 与 pd-device-bridge.exe，组件 ID 保持 pd-device-bridge，防止已有 Pet 的桥接查找路径失效。当前只提供 Windows x64。登录信息仍由 Pet 的系统凭据存储管理；组件包不带账号密码，首次使用需要用户登录。

客户端包与 Linux 服务端扩展分开发布。客户端更新不能代替 Octop 的服务端补丁安装；服务端升级后按 README 中的兼容性检查维护，不要求用户安装 SSH 或新增连接。

## 最小源码改动

- Pet：现有桥接和眨眼补丁保持独立；新增 scripts/package_pd_component.py，只负责组件打包，不改变上游目录布局。
- 小助手：components/manager.py 将已安装组件的 launchable 信息交给界面；components/operations.py 增加固定入口启动；gui/main_window.py 增加“打开”按钮。浏览器等后台组件没有 launchable 标记，不显示可用的启动动作。相关测试为 tests/test_component_application_launch.py。
- 组件发布：使用已有通用外部目录和签名机制，不把 Pet 写入小助手内置组件表。以后发 Pet 新版本只更新包和签名清单；小助手不需要为每次 Pet 更新再改源码。

当前本机的小助手源码目录没有 .git，不能用其 Git HEAD 作为可靠基线；本轮启动入口的独立补丁见 snapshots/helper-component-launch.patch。同步小助手源码时先检查补丁上下文，不能覆盖整个 main_window.py。

## 本地构建与打包

先完成 Pet 的 make all（Windows 使用 scripts/check-windows.ps1）与独立桥接测试，再构建 Pet 发布程序和桥接发布程序：

```powershell
npm run tauri -- build --no-bundle
cargo build --release --manifest-path D:/ai/screen-automation-device-bridge/native/Cargo.toml
python scripts/package_pd_component.py --version 0.1.1 --bridge D:/ai/screen-automation-device-bridge/native/target/release/pd-device-bridge.exe
```

无签名参数时仅生成候选包和 manifest，不生成可在线安装的 latest 清单。正式清单必须使用小助手现有受信任签名工具和私钥；脚本会验签成功后才写 latest-windows-x64.json，私钥不进入 ZIP。可通过 --signing-python 指定包含 cryptography 的小助手构建 Python。

组件版本与 Pet 上游版本分开记录：本组件 0.1.1 可包含 Pet 0.2.0，协议为 pd-chat-cli@2。每次发包使用新的组件版本，不覆盖相同版本的已发布 ZIP。更新前后配置和凭据均不放在组件版本目录。

## 上传边界

本地输出默认位于 .cache/component-release。发布需上传版本 ZIP 与签名 latest 清单到组件下载站，catalog-entry.json 是需合并的条目，不能拿它覆盖完整 catalog.json。保留其他组件条目，合并后对整个 schema=1 目录重新签名。

下载站沿用 https://www.xiaozs.com/sah/components，组件目录要求 HTTPS。它和 Octop 服务地址相互独立，不改变现有 http://47.77.184.54:8088/ 聊天连接。

本次不自动上传网站、修改在线目录、打包或发布小助手主程序。已有用户需要包含上述通用启动入口及外部目录功能的小助手版本，才有完整的“安装→打开→更新”体验。源码可测试通过不等于已向用户上线。

## 上游同步顺序

1. 保存当前可用版本；在独立分支或临时目录更新原项目。
2. 分别迁移桥接、眨眼、小助手启动入口三个补丁，先检查上下文，避免盲目覆盖。
3. 重跑各项目检查；确认普通聊天、按需 CLI、组件安装/更新、配置保留。
4. 重新构建组件，提升组件版本；保留旧包及源码基线记录。
5. 通过验收后再更新签名 latest 和目录；Octop 升级另外核对服务端补丁。

上游接线位置改变时仍需要人工调整。补丁、基线和独立组件的作用是缩小需核对的范围，不能承诺所有上游版本自动无冲突更新。

## 本轮交付与验证

Windows x64 签名组件 0.1.1 已生成在 .cache/component-release/0.1.1；latest-windows-x64.json 已通过小助手受信任公钥验签。catalog-candidate.json 根据本机已验签缓存合并，保留其他组件；缓存可能落后于网站，上传前应使用网站当前目录重新合并，不能盲目覆盖。scripts/merge_pd_component_catalog.py 会先验签输入目录，再只替换本组件条目、签名和验签输出；不会请求网络或上传。

Pet 的 Windows make all 等价检查通过，包含 130 项前端与 24 项 Rust 测试；正式 release 构建通过。小助手相关组件测试共 40 项，其中 38 项通过、2 项跳过。实际签名 ZIP 在临时目录完成安装、重复安装、卸载和用户数据保留检查；临时签名 0.1.2 包还完成 0.1.1→0.1.2 更新和成对入口检查，未发布该测试版本。界面代码通过编译检查，尚未在重启的小助手 GUI 中人工验收“打开”按钮。

本轮无远程上传、无 Git 提交/推送，未替换正在使用的组件或重启用户的小助手。正式对外仍待网站目录发布及小助手主程序交付。
