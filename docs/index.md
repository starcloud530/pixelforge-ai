# PixelForge

**Create editable pixel art and animation with your AI agent.**

与 AI Agent 一起创作角色、编排逐帧动作，再导出游戏所需的精灵表与图集。索引色、图层与帧时长保留在同一份可编辑工程里。

[开始创作 · Install](install.md){ .md-button .md-button--primary }
[查看作品 · Gallery](https://starcloud530.github.io/pixelforge-demos/gallery/){ .md-button }

## 哪吒 · 火尖枪攻击

![哪吒蓄力、出枪与收势的12帧像素攻击动画](https://raw.githubusercontent.com/starcloud530/pixelforge-demos/main/demos/nezha-frame-attack/final/nezha-attack.gif){ width="384" }

128 × 128 像素，12 帧，四个独立图层。从蓄力、蹬地刺出到收势，身体、脚步与混天绫逐帧编排。

[打开示例：播放、编辑与导出](showcase.md) · [制作流程与源工程](https://github.com/starcloud530/pixelforge-demos/tree/main/demos/nezha-frame-attack)

## 你的创作流程

1. **创作角色**：让 Codex、Claude Code 或 Cursor 调用本地 `pixelforge`，从剪影推进到配色与细节。
2. **编排动作**：复制关键姿势、调整图层与帧时长，在本地播放器中检查节奏。
3. **持续精修**：按调色板换色，检查帧间差异，逐步修改同一份工程。
4. **交付资产**：导出 PNG 序列、精灵表、atlas JSON 与 APNG 预览。

## 文档导航

| 目标 | 从这里开始 |
| --- | --- |
| 安装与第一份工程 | [安装指南](install.md) |
| 播放并导出哪吒动画 | [示例教程](showcase.md) |
| 让 Agent 使用创作技能 | [Agent 与 CLI](agent-skill.md) |
| 将精灵表导入游戏引擎 | [导出与引擎导入](export.md) |
| 管理角色与资产关系 | [项目清单](project-manifest.md) |
| 查询工具参数 | [Tool API](tool-api.md) |
| 选择运行环境与工作流 | [使用说明](known-limits.md) |
| 许可与商业使用 | [许可说明](licensing.md) |

产品网站：[PixelForge](https://starcloud530.github.io/pixelforge-demos/) · 产品文档：[Docs](https://starcloud530.github.io/pixelforge-ai/) · [下载与更新记录](https://github.com/starcloud530/pixelforge-ai/releases)
