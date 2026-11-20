# PixelForge 技术文档

<div class="docs-studio-cover" markdown>
<div markdown>

用开源 CLI 在本地编辑像素和动画，导出 PNG 精灵表与图集 JSON。

[安装 PixelForge](install.md){ .md-button .md-button--primary }
[验证示例工程](showcase.md){ .md-button }

[v0.1.0-rc.6 预发布 · 安装包与校验和](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.6)

</div>
<figure><a href="/pixelforge-demos/gallery/#tidal-glasshouse"><img src="https://resources.pixelforge.torzsi.com/studio/rc6/1bacf12e29c69944/glasshouse-facade-preview.png?rev=d440a94c29c332e9" width="384" height="288" alt="潮汐温室原稿，暖光门廊和冷绿色玻璃罩"></a><figcaption>潮汐温室 · 128 × 96 px 立面</figcaption></figure>
</div>

## 示例工程

| 检查内容 | 工程 |
| --- | --- |
| 图块如何组成一张五层地图 | [潮汐温室](/pixelforge-demos/gallery/#tidal-glasshouse) |
| 四向步态和帧时长 | [雪岭邮差](/pixelforge-demos/gallery/#alpine-courier) |
| 机关的启动与循环状态 | [星芒风候仪](/pixelforge-demos/gallery/#observatory-machine) |
| 保留像素轮廓、切换整套配色 | [潮汐科考道具](/pixelforge-demos/gallery/#tide-survey-icons) |
| 从本地工程重新导出并校验图集 | [守灯人与哪吒的 CLI 教程](showcase.md) |

## 部署与接入

- 用 Agent 或自有脚本调用本地 CLI。[Agent 接入](agent-skill.md) · [Tool API](tool-api.md)
- 在 Godot 或 Unity 设置切片、轴心和动画映射。[引擎导入指南](export.md)
- 登记画布、色板和角色方向，检查资产依赖。[项目清单](project-manifest.md)

## 试用前核对

- **数据流**。CLI 在本机读写工程。启用外部 Agent 时，按所选服务与配置核对上下文、提示和文件的处理方式。浏览器 AI 演示的路径见[数据流说明](ai-driver.md)。
- **验收范围**。结构检查覆盖图集与标签。动作质量、团队协作方式和游戏内效果应加入试用验收。参见[已知边界](known-limits.md)和[检查与性能](quality.md)。
- **许可与版本**。软件采用 MIT 或 Apache-2.0，示例美术采用 CC BY 4.0。[许可详情](licensing.md) · [发布记录](https://github.com/starcloud530/pixelforge-ai/releases)
