# 哪吒攻击动画

从一份完成的角色动画开始，体验播放、逐帧查看和游戏图集导出。

![哪吒火尖枪攻击](https://raw.githubusercontent.com/starcloud530/pixelforge-demos/main/demos/nezha-frame-attack/final/nezha-attack.gif){ width="384" }

这段动画使用 128 × 128 画布，12 帧总长 1070 ms，包含准备、蓄力、刺出和收势。工程分为红绫、下肢与风火轮、身体、手臂与枪四层。

## 打开与播放

先[安装 PixelForge](install.md)，再下载工程：

```bash
curl -L --fail \
  https://raw.githubusercontent.com/starcloud530/pixelforge-demos/main/demos/nezha-frame-attack/final/nezha-attack.pxf.json \
  -o nezha-attack.pxf.json
pixelforge play --project nezha-attack.pxf.json
```

Windows 也可以在浏览器中[下载工程](https://raw.githubusercontent.com/starcloud530/pixelforge-demos/main/demos/nezha-frame-attack/final/nezha-attack.pxf.json)，在文件所在目录运行同一条 `pixelforge play` 命令。

播放器支持播放、暂停和逐帧查看。将它与 Agent 的编辑流程配合使用，检查出枪节奏、身体位移与红绫回摆。

## 导出游戏资源

```bash
pixelforge export sheet --project nezha-attack.pxf.json \
  --output nezha-attack-sheet.png --cols 6 --atlas nezha-attack.atlas.json
pixelforge export apng --project nezha-attack.pxf.json --output nezha-attack.apng
```

精灵表与 atlas JSON 用于游戏导入；APNG 保留透明背景与帧时长，适合预览。详细参数见[导出指南](export.md)。

## 接着创作

让 Agent 先读取内置技能，再说明你想调整的动作或配色：

```bash
pixelforge skills read pixelforge
pixelforge skills read pixel-art-craft
```

例如：“保留哪吒的身份特征，调整蓄力到刺出的帧时长；播放检查，再导出一份新图集。”

[完整制作流程与可重跑脚本](https://github.com/starcloud530/pixelforge-demos/tree/main/demos/nezha-frame-attack) · [更多作品](https://starcloud530.github.io/pixelforge-demos/gallery/)

示例美术 CC BY 4.0，署名 PixelForge / starcloud530；CLI 绘制，AI 辅助编排。[许可与商业使用](licensing.md)。
