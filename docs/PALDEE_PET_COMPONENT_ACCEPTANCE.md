# Paldee Pet 0.2.1 组件验收

记录日期：2026-10-08。本报告对应本地品牌版，不表示已签名、上传或发布。

## 命名与兼容

显示名 Paldee Pet；ZIP 为 paldee-pet-0.2.1-windows-x64.zip；入口 paldee-pet.exe；服务器目录 /www/wwwroot/xiaozs/sah/components/paldee-pet/。
内部组件 ID octoppet、协议 octoppet-component@1、聊天桥接 pd-chat-cli@2、用户数据及 keyring 标识不变。只支持 Windows x64。

## 修改文件

- scripts/package_pd_component.py：新入口、ZIP 名、输出目录和 HTTPS 下载地址及目录校验。
- scripts/test_pd_component.py：按新入口验收，仍验证元数据、架构、中文空格路径、并发和重复启动、正常停止、异常退出、超时、协议不兼容和独立数据保留。
- package.json、package-lock.json、src-tauri/tauri.conf.json、src-tauri/Cargo.toml、Cargo.lock：根包版本 0.2.1，依赖版本不变。
- src-tauri/src/component_runtime.rs：实际验收发现同时启动时可能提前关闭管道；仅 start/status 对 EOF/断管做有总超时约束的重试，不重试其他操作；竞争实例退出码 0 时继续查询既有实例，不误报启动失败。start 请求使用同作用域 Windows 命名互斥锁，等待计入总超时，异常退出由系统释放；并发冷启动验收连续执行五轮。
- scripts/export_pd_component_patch.py：补丁纳入品牌接线与版本文件；不操作索引、分支或远端。
- CHANGELOG.md、组件规范说明、同步流程及历史验收提示：更新发布路径和新旧包边界。

品牌来源仍为 Tauri productName；设置页、页面标题、托盘和清单共用它。原始 Cargo 产物名保留，复制进 ZIP 时改名，不给上游增加包名修改。

## 发布和联调边界

latest-windows-x64.json 是未签名清单，签名前留存 unsigned 副本；不得直接用于线上安装。不读取产品私钥，不生成全局 catalog.json，不覆盖 Cua 文件。目录维护者应更新原 octoppet ID 条目的清单地址，不能另造重复 ID。
已有 0.2.0 包原样保留。更新须先按旧清单入口正常停止，再按新清单入口启动。小助手真实签名安装、更新、卸载和调用层联调仍未执行；远程 Octop 连接和屏幕操作没有在本轮重新测试。Windows 需要现有 WebView2；不宣称 x86 或 Mac 组件支持。

运行报告见输出目录 runtime-acceptance.json；原生源码差异通过 Git 审查，同步步骤见 [维护流程](upstream-sync/PALDEE_SYNC.md)。

## 最终产物与验证结果

Windows 等价 make all 通过：前端 130 项、Rust 27 项；最终启动互斥修改后 clippy -D warnings 与 Rust 27 项复测通过。生产构建通过；真实解压包 11 项验收全部通过（41.180 秒），包括五轮并发冷启动、中文空格路径、重复启动、显隐、正常停止、异常恢复、超时、协议拒绝及整个组件目录卸载后独立用户数据保留。

- `D:\ai\OctopPet\build\component-release\paldee-pet\0.2.1\paldee-pet-0.2.1-windows-x64.zip`：15378458 字节；SHA-256 `83a5c4a2f2cb3cdc35d609997af2ed4be8d4cdff69137de027dc27bdf84ceb85`。
- `D:\ai\OctopPet\build\component-release\paldee-pet\latest-windows-x64.json`：435 字节；SHA-256 `e372de98e19e24252e4ba7642ffd0a76194285ae2d0708c035a5718c85fa3ce8`。
- `D:\ai\OctopPet\build\component-release\paldee-pet\catalog-entry.json`：508 字节；SHA-256 `06ea920b22f9f4bd38ff326ca920a4709df3c9f66c691c2e2ad4fce2b992a6d6`。

ZIP 包含实际 x64 入口、component.json、LICENSE 与桥接及运行许可文件；元数据 ID/版本/架构/入口一致，解压和 CRC 校验通过，size/sha256 来自最终 ZIP。latest 清单与 unsigned 副本完全一致。未提交、推送、签名或上传。
