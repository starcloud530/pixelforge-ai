# 项目清单 Project manifest

`pixelforge-manifest` 是游戏美术的项目记忆。它记下画布档位、视角、目录约定、默认色板路径、一段短说明，以及资产根和相对路径。Agent 用 `pixelforge project show` / `project validate` 读回来。

它不是画布。像素仍在 `pixelforge-project` 里，画笔、图层、帧、导出都不读这份文件。它也不是素材库：不登记每一个文件，不记资源之间的关系，不查角色动画缺哪一格，也不做导出版本清单。

*A `pixelforge-manifest` is project memory for game art. It stores canvas tiers, perspective, directory rules, the default palette path, a short note, and the asset root with its relative paths. An agent reads it back with `pixelforge project show` and `project validate`.*

*It is not a canvas. Pixels stay in `pixelforge-project` files. Paint, layers, frames, and export do not read the manifest. It is not an asset library either: it does not register each file, it does not store relationships, it does not look for a missing animation cell, and it does not version a delivery.*

## 不在这里 Out of scope

- 可视化素材管理，或任何素材 GUI
- 地图 GUI
- 嵌在应用里的聊天
- 团队账号与权限
- 自动 AI 生图
- 逐个资产的登记与引用关系
- 角色动画矩阵、缺帧报告
- 带版本的导出或交付清单

灯塔试用（`examples/lighthouse-outpost/`）是 32×64 的单体，地图在另一个目录 `examples/base-tiles/`。这份格式拒绝 `..`，所以不把这两处收成同一个资产根。丧尸包才是下面的例子。

*Out of this file: a visual asset manager or any asset GUI, a map GUI, an embedded chat, team accounts and permissions, automatic image generation, a per-asset registry and its links, a character animation matrix or a missing-frame report, and a versioned export or delivery list.*

*The lighthouse trial (`examples/lighthouse-outpost/`) is one 32×64 sprite, and its map lives in `examples/base-tiles/`. This format refuses `..`, so those two directories are not one asset root. The zombie pack below is the checked-in example.*

## 格式 Format

version 1。`format` 必须是 `pixelforge-manifest`。未知字段拒绝，文件打不开（退出码 2，`error.code` 为 `ManifestFile`）。没有像素，没有撤销栈，没有检查点。

*Version 1. `format` must be `pixelforge-manifest`. An unknown field is refused and the file does not open (exit 2, `error.code` `ManifestFile`). There are no pixels, no undo stack, and no checkpoint.*

| 字段 | 必填 | 含义 |
| --- | --- | --- |
| `name` | 是 | 项目短名。去掉首尾空白后 1 到 64 个字符，不能有控制字符 |
| `note` | 是 | 给人与 Agent 读的风格或规范短句。1 到 400 个字符，不能有控制字符，也不要写成设计文档 |
| `perspective` | 是 | `top` 顶视、`side` 侧视、`front` 正视、`oblique` 斜视 / 四分之三 |
| `assetRoot` | 是 | 资产根，相对清单文件所在目录。`.` 表示就在该目录。其它值是相对路径 |
| `palette.path` | 是 | 默认色板，相对资产根。`.json` 必须是 `pixelforge-project`（色板在那份工程里）。`.gpl`、`.hex`、`.txt` 只要求文件存在 |
| `palette.ref` | 否 | 色板短标签。省略时文件不写 `ref` |
| `canvases` | 是 | 画布档位，1 到 32 条。每条是 `{ "name", "width", "height" }`，边长 `1..=256` |
| `layout` | 是 | 目录约定，1 到 64 条。不是文件名单 |

路径只用 `/`。不能是绝对路径，不能有 `..`、`.`、空段、`\` 或 `:`。这样相对路径出不了资产根。符号链接不另做沙箱。

`layout` 的每一条：

| 字段 | 必填 | 含义 |
| --- | --- | --- |
| `name` | 是 | 规则名，同一文件里不重复 |
| `dir` | 是 | 资产根下的目录，不重复 |
| `kind` | 是 | `character`、`prop`、`tile`、`portrait`、`ui`、`effect`。这是目录角色，不是资产编号 |
| `canvas` | 否 | 指向 `canvases` 里的 `name`。写了才核对该目录里的工程尺寸 |

*`name` is 1 to 64 characters after trim, with no control characters. `note` is 1 to 400 characters, also with no control characters. `perspective` is `top`, `side`, `front`, or `oblique`. `assetRoot` is relative to the manifest file; `.` means that directory. `palette.path` is relative to the asset root. A `.json` palette must be a `pixelforge-project`. A `.gpl`, `.hex`, or `.txt` palette only has to exist. `palette.ref` is an optional label and is omitted when unset. `canvases` is 1 to 32 tiers, each `{ "name", "width", "height" }` with edges `1..=256`. `layout` is 1 to 64 directory rules, not a file list.*

*Paths use `/` only. Absolute paths, `..`, `.`, empty segments, `\`, and `:` are refused, so a stored path cannot leave the asset root. Symlinks are not jailed further. A layout `kind` is `character`, `prop`, `tile`, `portrait`, `ui`, or `effect`: a directory role, not an asset id. `canvas` names a tier. The size check runs only when that field is set.*

下面是字段，不是字节级样例。写入器按 JSON 对象的键序输出，并在末尾加换行。已登记的例子是 [`examples/game-pack-zombie/pixelforge.manifest.json`](../examples/game-pack-zombie/pixelforge.manifest.json)：顶视，角色和道具都是 64×64，色板指向 `zombie/zombie.json`。

*The object below is the field list, not a byte-for-byte sample. The writer emits JSON-map key order and a trailing newline. The checked-in example is [`examples/game-pack-zombie/pixelforge.manifest.json`](../examples/game-pack-zombie/pixelforge.manifest.json): top-down, characters and props at 64×64, palette path `zombie/zombie.json`.*

```json
{
  "format": "pixelforge-manifest",
  "version": 1,
  "name": "zombie-outpost",
  "note": "顶视四向。角色和道具都是 64×64。本文件只记项目约定，不存像素。",
  "perspective": "top",
  "assetRoot": ".",
  "palette": { "path": "zombie/zombie.json", "ref": "zombie" },
  "canvases": [{ "name": "sprite", "width": 64, "height": 64 }],
  "layout": [
    { "name": "characters", "dir": "zombie", "kind": "character", "canvas": "sprite" },
    { "name": "props", "dir": "props", "kind": "prop", "canvas": "sprite" }
  ]
}
```

## 命令 Commands

```bash
pixelforge project init \
  --name zombie-outpost --perspective top \
  --note "顶视四向。本文件只记项目约定，不存像素。" \
  --canvas sprite:64x64 \
  --palette zombie/zombie.json --palette-ref zombie \
  --layout characters:zombie:character:sprite \
  --layout props:props:prop:sprite \
  --asset-root . \
  --output pixelforge.manifest.json
pixelforge project show --manifest pixelforge.manifest.json
pixelforge project validate --manifest pixelforge.manifest.json
```

`init` 不覆盖已有文件，也不创建像素工程，也不把色板抄进清单。`--canvas` 是 `name:WxH`，可重复。`--layout` 是 `name:dir:kind` 或 `name:dir:kind:canvas`，可重复。`--asset-root` 缺省 `.`。

`show` 只打印清单。不读像素，也不检查磁盘。

`validate` 不改文件。退出码：

| 情况 | 退出码 |
| --- | --- |
| 清单打不开，或不是这份格式 | `2` |
| 检查跑完，至少一条 error | `1` |
| 没有 error。可以有 warning | `0` |

stdout 仍是一行 JSON。检查跑完时 `ok` 为 true，`result.errorCount` 和 `result.warningCount` 在结果里。这和 `direction validate` 一样：工具调用成功，门槛看 `errorCount`。`project validate` 再把 error 映射成退出码 1。

*`init` refuses to replace an existing file. It does not create a pixel project and it does not copy palette colors into the manifest. Repeat `--canvas name:WxH` and `--layout name:dir:kind[:canvas]`. `--asset-root` defaults to `.`.*

*`show` prints the manifest. It does not read pixels and it does not touch the disk.*

*`validate` does not edit the file. Exit 2 means the file did not open. Exit 1 means the checklist has an error. Exit 0 means no errors; warnings may remain. Stdout is one JSON line. A finished check has `ok: true` and reports `errorCount`, the same shape as `direction validate`. `project validate` then exits 1 when `errorCount` is not 0.*

核对规则：

- 资产根必须是目录，否则 `assetRoot`。根不在时不再逐条报子路径。
- 色板缺失是 `paletteMissing`。目录、读不出的 JSON、或格式不是 `pixelforge-project` 的 `.json`，是 `paletteFormat`。`.png` 也是 `paletteFormat`。
- 目录缺失是 `layoutMissing`。路径是文件则是 `layoutKind`。
- 写了 `canvas` 的目录里，只看直接子级的 `*.json`。`format` 为 `pixelforge-project` 的，宽高必须等于该档，否则 `canvasSize`。尺寸读不出是 `canvasUnread`。不是 JSON 对象是 `projectFile`。
- `pixelforge-atlas`、`pixelforge-atlas-rules` 和其它格式跳过，不记成资产。
- 该目录里一份工程都没有，是 warning `layoutEmpty`，退出码仍可以是 0。
- `result.checks.layout[].projects` 只是数量。结果里没有文件名单。

*`assetRoot` means the root is not a directory; child paths are not also reported. `paletteMissing` and `paletteFormat` cover a missing palette, a directory, unreadable JSON, a `.json` that is not a `pixelforge-project`, and a `.png`. `layoutMissing` / `layoutKind` cover the directory. A layout that names a canvas checks only the `*.json` files directly inside it. A `pixelforge-project` must match that tier (`canvasSize`, or `canvasUnread`). A non-object is `projectFile`. Atlas JSON and other formats are skipped and are not registered. An empty directory is a `layoutEmpty` warning. `projects` is a count. The result has no file list.*

## Tool API

名字在 `pixelforge tools` 的 `manifestTools` 里，不在像素文档的 `tools` 里。`pixelforge call --project pixelforge.manifest.json` 看到这份 `format` 就走这里。`--checkpoint` 和 `--preview` 不能用。

| tool | 作用 |
| --- | --- |
| `inspectManifest` | 无参数。`result` 就是清单对象 |
| `validateManifest` | 不读磁盘。调用方传入已经看到的文件 |

`validateManifest` 的 `params`：

```json
{
  "rootExists": true,
  "files": [
    { "path": "zombie", "kind": "dir" },
    {
      "path": "zombie/zombie.json",
      "kind": "file",
      "format": "pixelforge-project",
      "width": 64,
      "height": 64
    }
  ]
}
```

`kind` 是 `file`、`dir` 或 `missing`。`width` 和 `height` 要成对，并且只放在文件上。漏了某条路径的探针是 `probeMissing`，所以空的 `files` 不会被当成通过。`project validate` 会自己收集这些探针。直接 `call` 时要自己传。

*The names are `manifestTools` from `pixelforge tools`, not the pixel-document `tools` list. `pixelforge call --project pixelforge.manifest.json` dispatches them when the format matches. `--checkpoint` and `--preview` are refused. `inspectManifest` takes no params and returns the manifest object. `validateManifest` does not read the disk; the caller passes `rootExists` and `files`. `kind` is `file`, `dir`, or `missing`. `width` and `height` are a pair and belong on a file. A path with no probe is `probeMissing`, so an empty `files` array does not pass. `project validate` collects the probes. A raw `call` has to pass them.*

错误：文件本身是 `ManifestFile`。请求信封是 `InvalidRequest`、`InvalidParams`、`UnknownTool`、`UnsupportedVersion`。`paint` 打到这份文件上是 `UnknownTool`。
