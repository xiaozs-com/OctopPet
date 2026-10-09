> 本文记录旧 0.2.0 包的验收及哈希，保留作历史依据。当前品牌包请参阅 [Paldee Pet 0.2.1 验收记录](PALDEE_PET_COMPONENT_ACCEPTANCE.md)。

# OctopPet 组件整改验收报告

日期：2026-10-08。ID octoppet；版本 0.2.0；Windows x64；协议 octoppet-component@1。

## 构建产物

| 绝对路径                                                                                                |   字节数 | SHA-256                                                          |
| ------------------------------------------------------------------------------------------------------- | -------: | ---------------------------------------------------------------- |
| D:\ai\OctopPet\build\component-release\octoppet\0.2.0\octoppet-0.2.0-windows-x64.zip                    | 15377879 | fe292b8a9d097f9c60f53538e09c2afd9319a8d97e704f8b36b462b1b0f4a152 |
| D:\ai\OctopPet\build\component-release\octoppet\latest-windows-x64.json                                 |      429 | 2a30a61c23f47886bdd2e0aa27e9ed4c909b710c33a142a491db8424e10331e8 |
| D:\ai\OctopPet\build\component-release\octoppet\catalog-entry.json                                      |      515 | c544f8a4859f2014d939ef51e30977b57475bb56d2549b6baad1fb471d2779e1 |
| D:\ai\OctopPet\src-tauri\target\x86_64-pc-windows-msvc\release\octop-pet.exe                            | 25062912 | 5e1872073ef41e29cfdf411e034bc6decbb605ec2bd1b48ce2ee361393c73230 |
| D:\ai\screen-automation-device-bridge\native\target\x86_64-pc-windows-msvc\release\pd-device-bridge.exe |   491520 | 0fbb698ba8d977c760d4c57aa15210e23d5fc9f287a931113a144b19cbabac33 |

latest-windows-x64.json 为未签名清单；原始未签名备份已保留。没有读取、创建、复制或输出产品签名私钥。签名必须在受控发布环境完成后才可上传和安装。catalog-entry.json 只有 OctopPet 自己的条目，没有生成全局 catalog.json。

## 修改文件

- src-tauri/src/component_runtime.rs：Windows 当前用户命名管道控制，JSON/超时/唯一实例/正常退出；GUI 子进程不继承调用者输出管道。
- src-tauri/src/lib.rs：最小运行入口接线及独立 WebView 目录；src-tauri/src/config_cmd.rs、secrets_cmd.rs：保持原配置标识和 keyring 服务，支持独立测试用户目录；pd_bridge_cmd.rs：读取标准包元数据，定位随包桥接。
- src-tauri/Cargo.toml、Cargo.lock：Windows 控制所需的已缓存依赖；未改动现有依赖版本。
- scripts/build_pd_component.ps1、package_pd_component.py、test_pd_component.py、export_pd_component_patch.py：Windows x64 构建、规范打包、实际程序验收。旧全局合并工具已备份退役。
- packaging/third-party-licenses/：补齐缺失许可证与固定源指纹。
- .gitignore、CHANGELOG.md、docs/OCTOPPET_COMPONENT.md 与旧方案文档：构建产物隔离及整改记录。
- 独立桥接仓库 D:/ai/screen-automation-device-bridge/LICENSE、native/Cargo.toml：按用户授权补 MIT 声明；桥接构建采用静态 CRT，未复制源码进小助手。

当前分支为 master，HEAD 53b675d，为浅克隆；origin 指向 xiaozs-com/OctopPet，未更改分支或远程。初始已有的眨眼、素材说明及补丁快照工作区改动保留；本轮未提交、push 或发布。

## 验证

- 仓库 make all 的 Windows 等价检查通过：格式、ESLint、TypeScript、Rust fmt/check/clippy，前端 130、Pet Rust 27 测试通过。
- 静态 CRT 桥接 Rust 3 测试通过，两个 EXE 的 PE Machine 均为 AMD64；打包检查拒绝 x86 与未随包提供的非系统 DLL。
- 最终 ZIP 可解压，入口、许可证及成对桥接存在；三份 ID/版本/入口及架构一致；ZIP 字节数和哈希已复核，文件白名单检查通过。
- ZIP 共 718 个文件，含 440 个第三方依赖许可证记录。没有缓存、凭据、日志、测试数据或用户数据。
- 实际解压包 11 项测试通过：中文/空格路径、串行与并发重复启动、显隐、正常停止、异常退出恢复、协议不匹配拒绝、超时、x86/错误元数据/大小/哈希拒绝、整个组件根及其他版本目录的用户数据拒绝规则、删除整个组件根后保留独立用户数据。
- 测试仅控制隔离目录中的宠物，未操作业务窗口、输入设备、服务器或 Cua 文件。首次诊断失败的候选包留在 .cache，未发布；最终包已重新验收。

## 尚未完成的边界

- 平台清单签名与公网下载/验签/安装检查。用户人工上传目标为 /www/wwwroot/xiaozs/sah/components/octoppet/。
- 全局目录由产品维护者合并自己的目录条目并签名；本仓库不会生成或覆盖它。
- 小助手调用层尚未按 octoppet-component@1 真机联调；不能把本地解压安装验收等同于已完成小助手功能组件 UI 的安装/更新/停止/卸载流程。
- Windows x64 之外未发布；新包在本机现有 Windows/WebView2 环境验收，未测试没有 WebView2 的全新机器。普通聊天/本机 CLI 仍沿用既有桥接，本轮没有重新连接远程 Octop 验收。

## 实现参考

- Windows 命名管道权限：[Microsoft](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)。
- 子进程句柄继承：[Microsoft](https://learn.microsoft.com/en-us/windows/win32/procthread/inheritance)。
- 第三方源及下载指纹在 packaging/third-party-licenses/SOURCES.json，ZIP 内保留完整许可文本与源码下载地址。
