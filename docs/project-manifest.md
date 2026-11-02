# 项目清单 Project manifest

`pixelforge-manifest` 是游戏美术的项目记忆。它记下画布档位、视角、目录约定、默认色板路径、一段短说明、资产根和相对路径。可选的 `assets` 数组是资产登记：稳定 id，以及这些文件之间的关系。Agent 用 `pixelforge project show` / `project validate` 和 `pixelforge asset` 读回来。

它不是画布。像素仍在 `pixelforge-project` 里，画笔、图层、帧、导出都不读这份文件。登记层只保存生产上下文：它不画像素，不扫描目录来发明新 id，不查角色动画缺哪一格，也不做导出版本清单。

*A `pixelforge-manifest` is project memory for game art. It stores canvas tiers, perspective, directory rules, the default palette path, a short note, and the asset root with its relative paths. The optional `assets` array is the registry: stable ids and the relations between those files. An agent reads it back with `pixelforge project show`, `project validate`, and `pixelforge asset`.*

*It is not a canvas. Pixels stay in `pixelforge-project` files. Paint, layers, frames, and export do not read the manifest. The registry stores production context. It does not paint, it does not scan a directory into new ids, it does not look for a missing animation cell, and it does not version a delivery.*

## 不在这里 Out of scope

- 可视化素材管理，或任何素材 GUI
- 地图 GUI
- 嵌在应用里的聊天
- 团队账号与权限
- 自动 AI 生图
- 角色动画矩阵、缺帧报告。`tags` 只记下这个资产上已经有的帧标签名，`asset validate` 不打开像素文件去对朝向 × 动作
- 草稿 / 待审 / 已导出这类状态。没有命令读它。交付快照是另一条，不写进这条登记
- 带版本的导出或交付清单

灯塔试用（`examples/lighthouse-outpost/`）是 32×64 的单体，地图在另一个目录 `examples/base-tiles/`。这份格式拒绝 `..`，所以不把这两处收成同一个资产根。丧尸包才是下面的例子。

*Out of this file: a visual asset manager or any asset GUI, a map GUI, an embedded chat, team accounts and permissions, automatic image generation, a character animation matrix or a missing-frame report (`tags` names frame tags that are already on the asset; `asset validate` does not open the pixel file), a draft / review / exported status (no command reads one; a delivery snapshot is a different record), and a versioned export or delivery list.*

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
| `assets` | 否 | 资产登记，0 到 256 条。省略或 `[]` 都表示没有登记，写回时丢掉空数组 |

路径只用 `/`。不能是绝对路径，不能有 `..`、`.`、空段、`\` 或 `:`。这样相对路径出不了资产根。符号链接不另做沙箱。

`layout` 的每一条：

| 字段 | 必填 | 含义 |
| --- | --- | --- |
| `name` | 是 | 规则名，同一文件里不重复 |
| `dir` | 是 | 资产根下的目录，不重复 |
| `kind` | 是 | `character`、`prop`、`tile`、`portrait`、`ui`、`effect`。这是目录角色，不是资产编号 |
| `canvas` | 否 | 指向 `canvases` 里的 `name`。写了才核对该目录里的工程尺寸 |

*`name` is 1 to 64 characters after trim, with no control characters. `note` is 1 to 400 characters, also with no control characters. `perspective` is `top`, `side`, `front`, or `oblique`. `assetRoot` is relative to the manifest file; `.` means that directory. `palette.path` is relative to the asset root. A `.json` palette must be a `pixelforge-project`. A `.gpl`, `.hex`, or `.txt` palette only has to exist. `palette.ref` is an optional label and is omitted when unset. `canvases` is 1 to 32 tiers, each `{ "name", "width", "height" }` with edges `1..=256`. `layout` is 1 to 64 directory rules, not a file list.*

*`assets` is optional, 0 to 256 records. A missing key and `[]` both mean no registry, and the writer drops an empty array.*

*Paths use `/` only. Absolute paths, `..`, `.`, empty segments, `\`, and `:` are refused, so a stored path cannot leave the asset root. Symlinks are not jailed further. A layout `kind` is `character`, `prop`, `tile`, `portrait`, `ui`, or `effect`: a directory role, not an asset id. `canvas` names a tier. The size check runs only when that field is set.*

下面是约定字段，不是字节级样例，也省略了 `assets`。写入器按 JSON 对象的键序输出，并在末尾加换行。已登记的例子是 [`examples/game-pack-zombie/pixelforge.manifest.json`](../examples/game-pack-zombie/pixelforge.manifest.json)：顶视，角色和道具都是 64×64，色板指向 `zombie/zombie.json`，`assets` 里有 `zombie`、`zombie.walk` 和道具。

*The object below lists the convention fields, not a byte-for-byte sample, and it omits `assets`. The writer emits JSON-map key order and a trailing newline. The checked-in example is [`examples/game-pack-zombie/pixelforge.manifest.json`](../examples/game-pack-zombie/pixelforge.manifest.json): top-down, characters and props at 64×64, palette path `zombie/zombie.json`, with `zombie`, `zombie.walk`, and the props in `assets`.*

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

## 资产登记 Asset registry

`assets` 的每一条是一个稳定 id，不是第二次像素缓冲。换一次会话，从同一份清单读回的还是这个 id 和这些关系。没有数据库。空的 `owner`、`dependsOn`、`produces`、`tags` 不写进文件。

*Each `assets` record is a stable id, not a second pixel buffer. A later session reads the same id and the same relations from this file. There is no database. Empty `owner`, `dependsOn`, `produces`, and `tags` are omitted.*

| 字段 | 必填 | 谁读它，为什么留着 |
| --- | --- | --- |
| `id` | 是 | 跨会话不变的名字，例如 `zombie.walk`。`list` / `show` / `graph` / `dependents` 用它查找。`validate` 拒绝重复。1 到 64 个字符，以字母开头，其余是字母、数字、`.`、`_`、`-`，点号不能造出空段 |
| `kind` | 是 | `asset list --kind` 的过滤。闭集：`character`、`group`、`gear`、`prop`、`tile`、`clip`、`sheet`。这不是目录规则的 `layout.kind` |
| `path` | 是 | 资产根下的文件。`validate` 报缺失（`assetMissing`）、目录（`assetPath`）或没探针（`probeMissing`）。几条记录可以共用一个路径：待机和走路都在 `zombie/zombie.json` |
| `owner` | 否 | 这条记录属于哪个角色或分组。`show` 的 `owns` 和 `graph` 的 `owner` 边读它。`validate` 要求目标存在、不是自己、目标的 `kind` 是 `character` 或 `group`，并且 owner 链不成环（`assetOwner`） |
| `dependsOn` | 否 | 先得有的资产 id。`asset dependents`、`graph` 和环检测读它。指到没有的 id 是 `assetRef`。成环是 `assetCycle` |
| `produces` | 否 | 这条记录是哪些 id 的来源。`show` / `graph` 读它。`validate` 要求对方的 `dependsOn` 含有自己（`assetProduces`）。反过来不要求：道具可以依赖一份共享色板，而色板并不「生产」这个道具 |
| `tags` | 否 | `asset list --tag` 的过滤。规则和动画标签 `tag add` 相同：去掉首尾空白后 1 到 64 个字符，不能有控制字符。空格、非 ASCII 和开头的数字都可以，例如 `walk down`、`行走`、`01_walk`。同一条里去空白后不能重复。不打开像素文件 |

*`id` is the stable name, for example `zombie.walk`. `list`, `show`, `graph`, and `dependents` look it up, and `validate` rejects a duplicate. It is 1 to 64 characters, starts with a letter, and then uses letters, digits, `.`, `_`, or `-`, with no empty dot segment.*

*`kind` is what `asset list --kind` filters. The closed set is `character`, `group`, `gear`, `prop`, `tile`, `clip`, `sheet`. It is not a layout rule's `kind`.*

*`path` is a file under the asset root. `validate` reports `assetMissing`, `assetPath` when the path is a directory, or `probeMissing`. Several records may share one path: idle and walk both live in `zombie/zombie.json`.*

*`owner` is the character or group this record belongs to. `show` lists `owns`, and `graph` emits an `owner` edge. `validate` requires the target to exist, to not be this id, to have kind `character` or `group`, and the owner chain to be acyclic (`assetOwner`).*

*`dependsOn` is the ids that have to exist first. `asset dependents`, `graph`, and the cycle check read it. An unknown id is `assetRef`. A loop is `assetCycle`.*

*`produces` is the ids this record is the source of. `show` and `graph` read it. `validate` requires that product to list this id in `dependsOn` (`assetProduces`). The converse is not required: a prop may depend on a shared palette asset without that palette producing the prop.*

*`tags` is what `asset list --tag` filters. The rule is the animation-tag rule from `tag add`: after trim, 1 to 64 characters and no control character. A space, a non-ASCII name, and a leading digit are allowed, such as `walk down`, `行走`, and `01_walk`. A repeated tag on one record is refused. The check does not open the pixel file.*

`kind` 里多出来的三个值是为了让 id 说的是真话：

- `gear` 是装备。本仓库的层角色已经用这个词。丧尸包没有单独的装备文件，所以没有 `gear` 记录。
- `group` 是「拥有者不是角色」的那一份工程。`props/props.json` 是分组，路障才是 `prop`。
- `clip` 是工程内部的一段帧，不是另一个角色。`sheet` 是给游戏导入的图集 JSON。旁边的 PNG 不另给一个 id。

*Three extra kinds keep the id honest. `gear` is equipment; this repo already uses that word for the layer role. The zombie pack has no separate gear file, so it has no `gear` record. `group` is the project that owns records when the owner is not a character: `props/props.json` is the group, and the barricade is a `prop`. `clip` is a frame span inside a project, not a second character. `sheet` is the atlas JSON the game imports. The PNG beside it is not a second id.*

丧尸包登记了 `zombie`（角色工程）、`zombie.idle`、`zombie.walk`（帧段，标签就是工程里的 `idle_*` / `walk_*`）、`zombie.sheet`（图集），以及 `props` 和四个道具、`props.sheet`。道具工程里的调色板和丧尸不是同一份，所以 `props` 不 `dependsOn` `zombie`。清单上的 `palette.path` 仍是默认角色色板，不是「每个文件都嵌了这份色板」。

*The zombie pack registers `zombie` (the character project), `zombie.idle` and `zombie.walk` (frame spans whose tags are the project's `idle_*` / `walk_*`), `zombie.sheet` (the atlas), plus `props`, the four props, and `props.sheet`. The props project embeds a different palette, so `props` does not `dependsOn` `zombie`. `palette.path` stays the default character palette. It does not claim that every file embeds that palette.*

`remove` 在还有别的记录用 `owner`、`dependsOn` 或 `produces` 指着这个 id 时拒绝。先改掉那些引用，再删。文件没有撤销栈。

*`remove` refuses while another record names the id in `owner`, `dependsOn`, or `produces`. Clear those references first. The file has no undo stack.*

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
pixelforge asset add --manifest pixelforge.manifest.json \
  --id zombie --kind character --path zombie/zombie.json \
  --produces zombie.idle --produces zombie.walk --produces zombie.sheet
pixelforge asset add --manifest pixelforge.manifest.json \
  --id zombie.walk --kind clip --path zombie/zombie.json \
  --owner zombie --depends-on zombie \
  --tag walk_down --tag walk_left --tag walk_right --tag walk_up
pixelforge asset list --manifest pixelforge.manifest.json --kind clip
pixelforge asset show --manifest pixelforge.manifest.json zombie
pixelforge asset graph --manifest pixelforge.manifest.json zombie
pixelforge asset dependents --manifest pixelforge.manifest.json zombie
pixelforge asset validate --manifest pixelforge.manifest.json
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

*`validate` does not edit the file. Exit 2 means the file did not open. Exit 1 means the checklist has an error. Exit 0 means no errors; warnings may remain. Stdout is one JSON line. A finished check has `ok: true` and reports `errorCount`, the same shape as `direction validate`. `project validate` then exits 1 when `errorCount` is not 0. That checklist includes registered asset paths and relations. `checks.assets` is the registry count.*

`asset add` 不覆盖已有 id。目标 id 可以先写在 `produces` 里，后补上那条记录；补齐之前 `asset validate` 报 `assetRef`。`asset set` 改 `kind`、`path`、`owner`、`dependsOn`、`produces`、`tags`，不改 id。`--clear-owner` 去掉拥有者。`--clear-depends-on`、`--clear-produces`、`--clear-tags` 清空列表。`asset list` 可按 `--kind`、`--owner`、`--tag` 过滤，三个条件同时成立才留下。`asset show` 返回存着的记录，加上算出来的 `owns` 和 `dependents`（id 列表，不是文件字段）。`asset graph` 是这个 id 的一跳：节点、`owner` / `dependsOn` / `produces` 边，以及这条记录自己指到、但还没登记的 id（`missing`）。`asset dependents` 返回 `dependsOn` 里含有这个 id 的整条记录。

`asset validate` 不改文件。退出码和 `project validate` 一样：打不开是 2，有 error 是 1，没有 error 是 0。空登记通过。它不读像素，所以绿色不表示朝向或动作齐。

*`asset add` refuses a repeated id. A `produces` id may be registered later; until then `asset validate` reports `assetRef`. `asset set` changes `kind`, `path`, `owner`, `dependsOn`, `produces`, and `tags`. It does not change the id. `--clear-owner` drops the owner. `--clear-depends-on`, `--clear-produces`, and `--clear-tags` empty those lists. `asset list` filters with `--kind`, `--owner`, and `--tag` together. `asset show` returns the stored record plus computed `owns` and `dependents` (id lists, not file fields). `asset graph` is one hop: nodes, `owner` / `dependsOn` / `produces` edges, and ids this record names that are not registered (`missing`). `asset dependents` returns the full records whose `dependsOn` includes this id.*

*`asset validate` does not edit the file. Exit 2 means the file did not open, exit 1 means an error, exit 0 means no errors. An empty registry passes. It does not read pixels, so a pass does not mean the facings or actions are complete.*

核对规则：

- 资产根必须是目录，否则 `assetRoot`。根不在时不再逐条报子路径。
- 色板缺失是 `paletteMissing`。目录、读不出的 JSON、或格式不是 `pixelforge-project` 的 `.json`，是 `paletteFormat`。`.png` 也是 `paletteFormat`。
- 目录缺失是 `layoutMissing`。路径是文件则是 `layoutKind`。
- 写了 `canvas` 的目录里，只看直接子级的 `*.json`。`format` 为 `pixelforge-project` 的，宽高必须等于该档，否则 `canvasSize`。尺寸读不出是 `canvasUnread`。不是 JSON 对象是 `projectFile`。
- `pixelforge-atlas`、`pixelforge-atlas-rules` 和其它格式跳过，不记成资产。
- 该目录里一份工程都没有，是 warning `layoutEmpty`，退出码仍可以是 0。
- `result.checks.layout[].projects` 只是数量。结果里没有文件名单。`checks.assets` 是登记条数。
- 登记的关系不看磁盘：悬空 id（`assetRef`、`assetOwner`）、`produces` 对不上 `dependsOn`（`assetProduces`）、`dependsOn` 成环（`assetCycle`）、owner 成环或种类不对（`assetOwner`）。资产根不存在时不逐条报路径，和色板、目录同一条规则。

*`assetRoot` means the root is not a directory; child paths are not also reported. `paletteMissing` and `paletteFormat` cover a missing palette, a directory, unreadable JSON, a `.json` that is not a `pixelforge-project`, and a `.png`. `layoutMissing` / `layoutKind` cover the directory. A layout that names a canvas checks only the `*.json` files directly inside it. A `pixelforge-project` must match that tier (`canvasSize`, or `canvasUnread`). A non-object is `projectFile`. Atlas JSON and other formats are skipped by the layout scan; a sheet is an asset only after it is registered. An empty directory is a `layoutEmpty` warning. `projects` is a count. The result has no file list. `checks.assets` is the registry count. Relation checks do not need the disk: a dangling id is `assetRef` or `assetOwner`, a `produces` edge whose product does not depend on the source is `assetProduces`, a `dependsOn` loop is `assetCycle`, and an owner loop or a bad owner kind is `assetOwner`. A missing asset root does not also report each asset path.*

## Tool API

名字在 `pixelforge tools` 的 `manifestTools` 里，不在像素文档的 `tools` 里。`pixelforge call --project pixelforge.manifest.json` 看到这份 `format` 就走这里。`--checkpoint` 和 `--preview` 不能用。

| tool | 作用 |
| --- | --- |
| `inspectManifest` | 无参数。`result` 就是清单对象。没有登记时不含 `assets` |
| `validateManifest` | 不读磁盘。调用方传入已经看到的文件。关系问题和路径问题都在 `errors` 里 |
| `validateAssets` | 同一份探针，只返回登记检查：`errors`、`errorCount`、`assets` |
| `addAsset` | `id`、`kind`、`path`，可选 `owner`、`dependsOn`、`produces`、`tags`。重复 id 是 `InvalidParams` |
| `setAsset` | `id` 加上要改的字段。`owner: null` 去掉拥有者。列表字段给 `[]` 就是清空。至少改一个字段 |
| `removeAsset` | `id`。仍被引用时是 `InvalidParams`，文件不动 |
| `listAssets` | 可选 `kind`、`owner`、`tag`。`result.assets` 是存着的记录 |
| `showAsset` | `id`。`result.asset` 是存着的记录，`owns` 和 `dependents` 是算出来的 id |
| `graphAsset` | `id`。一跳的 `nodes`、`edges`（`rel` 为 `owner`、`dependsOn` 或 `produces`）、`missing` |
| `dependentsAsset` | `id`。`dependsOn` 含有它的那些记录 |

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

*The names are `manifestTools` from `pixelforge tools`, not the pixel-document `tools` list. `pixelforge call --project pixelforge.manifest.json` dispatches them when the format matches. `--checkpoint` and `--preview` are refused. `inspectManifest` takes no params and returns the manifest object, omitting `assets` when the registry is empty. `validateManifest` and `validateAssets` do not read the disk; the caller passes `rootExists` and `files`. `kind` on a probe is `file`, `dir`, or `missing`. `width` and `height` are a pair and belong on a file. A path with no probe is `probeMissing`, so an empty `files` array does not pass a manifest that names paths. `project validate` and `asset validate` collect the probes. A raw `call` has to pass them. `addAsset`, `setAsset`, and `removeAsset` change this value; the caller writes the file when `save` changed. There is no undo stack.*

错误：文件本身是 `ManifestFile`。请求信封是 `InvalidRequest`、`InvalidParams`、`UnknownTool`、`UnsupportedVersion`。`paint` 打到这份文件上是 `UnknownTool`。
