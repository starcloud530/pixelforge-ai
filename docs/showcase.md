# 示例工程验证

用守灯人和哪吒的源工程练习播放动画、重新导出和校验图集。

[安装 PixelForge](install.md){ .md-button }
[浏览作品集](/pixelforge-demos/gallery/){ .md-button }

## 港口守灯人：验收四向角色 { #port-keeper }

64 × 64 画布，四个朝向，每向一帧闲置和四帧行走。角色有四层，换装版新增独立披风层。下载包同时提供共享色板的道具与局部修改工程。

[查看守灯人工程包](/pixelforge-demos/gallery/#port-keeper) · [查看源文件和完整重放方法](https://github.com/starcloud530/pixelforge-demos/tree/main/demos/demo-zombie)

### 播放原始工程

解压角色交付包，在解压目录中运行。播放器可暂停、逐帧查看及选择动作；完成后按 Ctrl+C 退出。

```bash
pixelforge --version
pixelforge play --project a/keeper.pxf.json
```

### 验证并重新导出图集

第一条检查原始 PNG 与 atlas JSON。后两条从同一份工程导出一对新文件，再检查新结果。图集 JSON 的 `image` 字段指向同目录下的 PNG。

```bash
pixelforge validate-atlas --atlas a/keeper-sheet.atlas.json
pixelforge export sheet --project a/keeper.pxf.json --cols 5 --output keeper-check.png --atlas keeper-check.atlas.json
pixelforge validate-atlas --atlas keeper-check.atlas.json
```

查看校验输出中的错误、告警和检查项。打开 `keeper-check.atlas.json`，可检查 `idle_down`、`walk_down` 等八个标签，以及每帧的坐标、尺寸和时长。要验证换装，再对 `b/keeper-storm-sheet.atlas.json` 运行同一条校验命令。

### 带进目标工程

从 [Godot 导入](export.md#godot-4)或 [Unity 导入](export.md#unity)继续，在引擎内设置最近邻过滤、切片、轴心和动画映射。建议在目标背景和真实缩放比例上预览，确认脚底稳定、动作可读并适配现有管线。

[查看四套导出的结构校验报告](https://github.com/starcloud530/pixelforge-demos/blob/main/demos/demo-zombie/validation.json) · [查看重放版本与图集哈希](https://github.com/starcloud530/pixelforge-demos/blob/main/demos/demo-zombie/media-latest.json)

## 哪吒：检查逐帧动作 { #nezha }

这段朝右的普通攻击使用 128 × 128 画布，12 帧总长 1070 ms。四层分别保留红绫、下肢与风火轮、身体、手臂与枪。

[查看哪吒动画与工程包](/pixelforge-demos/gallery/#nezha) · [查看姿势配置和完整制作脚本](https://github.com/starcloud530/pixelforge-demos/tree/main/demos/nezha-frame-attack)

### 打开与播放

解压动作交付包，或直接[下载源工程](https://raw.githubusercontent.com/starcloud530/pixelforge-demos/main/demos/nezha-frame-attack/final/nezha-attack.pxf.json)，然后在文件所在目录执行：

```bash
pixelforge play --project nezha-attack.pxf.json
```

暂停后逐帧检查前脚落定、出枪速度和红绫回摆。用 `ready`、`charge`、`thrust` 和 `recover` 标签跳转到对应阶段。

### 导出游戏资源

```bash
pixelforge export sheet --project nezha-attack.pxf.json --cols 6 --output nezha-check.png --atlas nezha-check.atlas.json
pixelforge export apng --project nezha-attack.pxf.json --output nezha-check.apng
pixelforge validate-atlas --atlas nezha-check.atlas.json
```

游戏导入使用精灵表与 atlas JSON。透明 APNG 保留时长，用于审阅和分享。

### 接着创作

让 Agent 先执行 `pixelforge skills read pixelforge` 和 `pixelforge skills read pixel-art-craft`。可以从修改蓄力段时长开始，保留角色像素与图层，播放确认节奏后导出新图集。

两套示例为 CLI 绘制、AI 辅助，采用 CC BY 4.0，署名 PixelForge / starcloud530。[许可详情](licensing.md)
