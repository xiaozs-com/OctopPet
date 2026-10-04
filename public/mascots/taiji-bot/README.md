# 太极机器人：OctopPet 半身素材包 v1

包含高分辨率半身主形象 hero.png、32 帧图集 spritesheet.png、8 种状态的静态 PNG 和循环动画 WebP、manifest.json、可离线打开的 preview.html。

动画是每种 4 帧、4 fps、1 秒循环的关键帧版本。图集由 imagegen 根据已确认的半身机器人参考生成，PNG 和 WebP 由 FFmpeg 确定性导出。不同姿势有少量轮廓和比例变化，尚未在真实 macOS/Windows OctopPet 窗口验收。

## 使用

把本目录复制到 OctopPet 的 public/mascots/taiji-bot/。默认素材路径为 /mascots/taiji-bot/idle.webp，静态降级路径为 /mascots/taiji-bot/idle.png。

OctopPet 现有 MascotImage 使用 img，可直接加载动画 WebP；不需要精灵图播放器。前端 MascotId、MASCOT_SRC、设置选项和 Rust select_mascot 需要允许 taiji-bot。旧 Peek/Type 素材可保留。

根据业务状态选择文件：idle、waving、listening、thinking、working、waiting、success、error。任务状态的自动切换需在当前项目的真实事件接口中接线，素材包自身不包含 Agent/工作流运行实现。完成、失败、挥手可以播放一轮后由调用方切回 idle；图片文件本身设置为无限循环。

## 预览与验证

双击 preview.html，对比 80/128/160/224 像素、浅色和深色背景。所有动画 WebP 应包含 4 个 ANMF 帧，每帧 250ms，画布 222×222，透明背景。

原始生成图集 spritesheet-source.png 为 887×1774，动作行间距不完全均匀。已根据透明通道确定各行动作边界，单独截取、居中留白后重组为标准 888×1776 图集 spritesheet.png，再按 222×222 切帧，避免截断头顶或带入下一行。

本包未修改聊天协议，未提供安装包，未发布到 GitHub。
