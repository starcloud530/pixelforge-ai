# Contributing

感谢关注 [PixelForge](https://github.com/starcloud530/pixelforge-ai)。本仓只维护 **引擎 / CLI**（索引色像素工程工具链）。

## 先开 issue

- Bug / 功能：用仓库 Issue 模板。
- **演示素材、可重放案例、eval tasks**：请开到 [`pixelforge-demos`](https://github.com/starcloud530/pixelforge-demos)，不要开在本仓。
- 标签：`bug` / `enhancement` / `documentation` / `area:*` 对外有用；`phase:*` 偏内部规划，外部贡献不必填。

## 小 PR

1. 从最新 `main` 开分支。
2. 保持改动聚焦；大功能先 issue 对齐范围。
3. 本地跑通：
   - `cargo test -p pixelforge-kernel`
   - `cargo test -p pixelforge-cli`
4. 打开 PR，填模板；关联 issue。
5. CI workflow **CI** / job **test and export gate** 必须绿。

## 合入方式

- 只接受 **squash merge** 进 `main`。
- 合入后删除头分支。
- 一人维护时可自合；不强制第二人 review。

## 安全

漏洞请按 [SECURITY.md](./SECURITY.md) 私报，勿开公开 issue。
