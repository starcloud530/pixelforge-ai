# 检查、性能与设计边界

## 可复现检查

工具链见 [dev-setup.md](dev-setup.md)。Python 3.9+ 用于本地检查脚本；CI 使用 Python 3.12。
安装依赖：`pnpm install --frozen-lockfile`。`packageManager` 固定 pnpm 10，避免不同大版本重写锁文件。

| 命令 | 范围 |
| --- | --- |
| `pnpm lint` | Rust 格式、全目标严格 Clippy、Web/landing TypeScript、离线文档一致性 |
| `pnpm quick` | 构建 CLI，8 次基础调用，播放器 JS 测试、文档一致性 |
| `pnpm smoke` | 构建 CLI，含失败路径与文件不变性验证的 25 次调用，加地图/图集/登记夹具门禁 |
| `cargo test --workspace --locked --no-fail-fast` | 全工作区 Rust 测试，含并发、revision、转换、只读播放器及输出保护 |
| `pnpm test` | 编译 Node WASM 并测试前端一轮编排与 proxy，不调用真实模型 |
| `pnpm build` / `pnpm build:landing` | Web/WASM 与 landing，包含 TypeScript 检查 |
| `pnpm test:browser` | 真实 Chromium 的播放器桌面/窄屏、键盘、错误恢复、只读与轮询回归 |
| `pnpm check:docs` / `mkdocs build --strict` | 本地链接、工具目录与站点入口一致性，以及文档构建 |

Windows 若没有 `python3` 别名，用 `python scripts/checks.py lint|quick|smoke`。
`quick` 和 `smoke` 的 `--no-build` 参数要求已存在当前源码构建的 CLI，不会回退到 PATH 中的旧安装。
可用 `--artifacts DIR` 保留逐调用 JSON 证据。

运行浏览器测试前先 `cargo build -p pixelforge-cli --locked` 和 `pnpm exec playwright install chromium`。
Linux CI 使用 `pnpm exec playwright install --with-deps chromium`。
本地可用 `PIXELFORGE_BROWSER_EXECUTABLE` 指定已有 Chrome；`PIXELFORGE_TEST_ARTIFACTS` 指定截图目录。
浏览器测试生成的项目在临时目录，所有像素通过 CLI 创建，结束后清理。

CI 的 `test and export gate` 汇总 Rust 的 Linux/macOS/Windows 测试、Rust 1.83、Node 20/22、
Web/WASM、浏览器与文档检查。任一依赖失败、取消或跳过都会使汇总失败。
这不会修改分支保护设置，也不执行版本发布或服务部署。

## 代码设计与兼容性

- 内核拥有像素、索引规则和工程格式；CLI 处理磁盘与进程；播放器只读。
- 内部加载、约束、绘制和转换辅助使用具名参数对象，避免同类型位置参数错配。
- CLI 分派只反序列化 format，不复制整棵像素 JSON 树；只读会话在进入内核前拒绝修改，并复用已加载快照的 fingerprint，避免反复序列化大型工程。
- parser/loader 的复合返回值使用命名类型别名，CLI 和 Tool API JSON 格式保持不变。
- 严格 Clippy 保持开启。九个既有公开标量绘制/约束接口保留源兼容性，逐函数声明
  `expect(clippy::too_many_arguments)`；`Mask::try_from_rects` 的 `Err(())` 同样保留兼容例外。
  没有全模块/全仓库 `allow`；新增内部函数不继承这些例外。以后改变接口时应重新评估这些预期。
- Windows CLI 显式保留 8 MiB 主线程栈（按需提交内存），与已验证的 Unix 环境对齐；
  原默认 1 MiB 无法容纳 clap 生成的 debug 命令构建栈帧。Windows 全套 CLI 集成测试覆盖启动与命令执行；
  这不是对任意深度 JSON 或无界递归的保证，后续扩充命令目录仍需关注生成代码的栈占用。
- `rust-version = 1.83` 既约束 Cargo，也让 Clippy 避免推荐最低版本不可用的 API。

## 输出与失败语义

预览、diff 和导出先检查全部目的路径：不得别名到输入工程、地图、图块工程、检查点或锁文件，
也不能让两个输出指向同一位置。软链接和带硬链接别名的目的文件拒绝写入；改用普通目的文件。
错误码为 `OutputConflict`，退出码 2，预检查失败不会先写第一份输出。

工程锁路径通过规范化父目录和稳定文件名定位，显式解析符号链接；避免 Windows 原子替换时
规范化旧数据句柄得到 NTFS 删除区路径。CI 在 Windows 连续执行 20 轮并发编辑回归。

每个产物采用临时文件加原子替换，避免一次写入失败截断旧产物。但 PNG+atlas、地图 PNG+TMJ、
PNG 序列，以及工程+checkpoint，仍是多个文件，不是跨文件事务；后续文件失败时先前文件可能已发布。
非协作进程在路径检查与写入之间替换目录的情况不属于此检查的保证。

## 性能测量

```bash
pnpm perf --output /tmp/pixelforge-performance.json
# 可选：增加真实高色数 PNG 输入；不修改原图。
pnpm perf --source /path/to/reference.png --output /tmp/pixelforge-performance.json
```

默认构建 release CLI，生成 32×32/8 帧、64×64/16 帧、256×256/32 帧的负载夹具，
测量 scene、preview、sheet、APNG 的耗时与子进程峰值 RSS，并测量播放器启动和编辑到快照更新的延迟。
可选输入会测试五个可用转换引擎。所有生成和修改都经 CLI；色条和复制帧仅用于负载测试，不代表动画质量。

默认每项三次，导出项先预热一次；报告原始样本、中位数和近邻 p95。少量样本不能当作线上 SLA。
macOS 的 RSS 单位按字节、Linux 按 KiB 归一化；没有 `wait4` 的平台记为 null，不能当作 0。
播放器刷新延迟包含 CLI 编辑和轮询，未采集该长驻进程的峰值内存。

不将绝对耗时作为跨机器 CI 门槛。比较优化前后时固定源码、编译 profile、输入哈希、硬件和重复次数。
默认负载不是所有合法工程的最大规模；高色数、长动画、大量层和极端文件仍需专项测量。
离线文档检查也不等于外链、所有锚点和所有参数 schema 都已校验。
