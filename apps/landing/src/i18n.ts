export type Lang = "en" | "zh";

export const LANG_STORAGE_KEY = "pixelforge-landing-lang";

export type Copy = {
  metaTitle: string;
  metaDescription: string;
  brandTagline: string;
  langSwitchLabel: string;
  langEn: string;
  langZh: string;
  headline: string;
  subheading: string;
  benefits: { title: string; body: string }[];
  ctaInstall: string;
  ctaDocs: string;
  ctaGitHub: string;
  heroVisualAlt: string;
  heroEyebrow: string;
  heroMeta: string;
  demoHeading: string;
  demoCaption: string;
  localEditCaption: string;
  localEditNote: string;
  walkAlt: string;
  propsAlt: string;
  propsHeading: string;
  propsSub: string;
  propLabels: string[];
  localEditAlt: string;
  localEditSteps: string[];
  comparisonHeading: string;
  comparisonDisclaimer: string;
  comparisonAlt: string;
  softLabel: string;
  nearestLabel: string;
  videoHeading: string;
  videoCaption: string;
  statusHeading: string;
  statusBody: string;
  notClaims: string[];
  footerQualification: string;
  footerLicense: string;
  footerAssets: string;
  footerHome: string;
};

export const copy: Record<Lang, Copy> = {
  en: {
    metaTitle: "PixelForge — From agent commands to editable pixels",
    metaDescription:
      "A local CLI for indexed pixel art. Let your external agent edit real grids and layers, inspect changes, and export sprite sheets with timing and tags.",
    brandTagline: "Real pixels. Agent-driven.",
    langSwitchLabel: "Language",
    langEn: "EN",
    langZh: "中文",
    headline: "From agent commands to editable pixels.",
    subheading:
      "A local CLI for indexed pixel art. Let your external agent edit real grids and layers, inspect changes, and export sprite sheets with timing and tags.",
    benefits: [
      {
        title: "True indexed projects",
        body: "A real pixel grid, a shared palette, and editable layers you can inspect.",
      },
      {
        title: "Your agent, the local CLI",
        body: "Edit with commands. Check local changes with preview and diff.",
      },
      {
        title: "Export for your engine",
        body: "PNG sheets and atlases with frame timing, animation tags, and structural validation.",
      },
    ],
    ctaInstall: "Install",
    ctaDocs: "Docs",
    ctaGitHub: "GitHub",
    heroVisualAlt:
      "Port keeper sprite shown in four directions, lantern lit, orange scarf on a blue coat",
    heroEyebrow: "Commands in. Editable pixels out.",
    heroMeta: "64 × 64 / four directions",
    demoHeading: "One character. Local edits you can inspect.",
    demoCaption:
      "One port keeper. Four walking directions, a gear variant, matching props, and three local edits. Authored by an external agent through the local CLI, not built-in one-click generation.",
    localEditCaption:
      "Recolor the scarf. Add a shield. Fix the planted boot. Keep the character, inspect the changes.",
    localEditNote:
      "The initial boot offset is an intentional diagnostic fixture. Native diffs accompany every step.",
    walkAlt: "Four-direction walk cycle of the port keeper",
    propsAlt: "Matching props and gear variants on the shared palette",
    propsHeading: "One palette. A whole harbor.",
    propsSub: "Four props from the same editable demo project.",
    propLabels: ["Bag", "Lantern", "Crate", "Buoy"],
    localEditAlt: "Three local edit steps: scarf recolor, shield, planted boot fix",
    localEditSteps: ["Start", "Recolor scarf", "Add shield", "Fix boot position"],
    comparisonHeading: "A crisp grid is a deliberate choice.",
    comparisonDisclaimer:
      "Soft resampling / simulated vs nearest display / native project. An illustrative rendering comparison, not a generation or conversion benchmark.",
    comparisonAlt:
      "Side-by-side illustrative comparison: soft resampling on the left, nearest-neighbor native project display on the right",
    softLabel: "Soft resampling / simulated",
    nearestLabel: "Nearest display / native project",
    videoHeading: "Local editing in one short take",
    videoCaption: "Optional walkthrough of scarf, shield, and boot edits via the CLI.",
    statusHeading: "Honest status",
    statusBody:
      "Early / WIP. Prebuilt CLI v0.1.0-rc.2 is a prerelease. Prefer that tag over GitHub latest until a stable release exists. Structural validation is not aesthetic approval.",
    notClaims: [
      "Not a full Aseprite clone",
      "Not diffusion fake-pixel",
      "Not unattended L4 generation",
      "Not built-in one-click gen",
    ],
    footerQualification:
      "Structural validation is not an aesthetic or in-game approval. This demo has not passed a pixel-artist blind review; aestheticApproval: false.",
    footerLicense: "Code: MIT OR Apache-2.0",
    footerAssets: "Example art: CC BY 4.0",
    footerHome: "Marketing home",
  },
  zh: {
    metaTitle: "PixelForge — 让 Agent 的命令，成为可编辑的像素",
    metaDescription:
      "本地 CLI 索引色像素工具。由外部 Agent 编辑真实网格与图层，预览差异，导出带时序与标签的精灵图集。",
    brandTagline: "真像素。Agent 驱动。",
    langSwitchLabel: "语言",
    langEn: "EN",
    langZh: "中文",
    headline: "让 Agent 的命令，成为可编辑的像素。",
    subheading:
      "本地 CLI 索引色像素工具。由外部 Agent 编辑真实网格与图层，预览差异，导出带时序与标签的精灵图集。",
    benefits: [
      {
        title: "真实索引工程",
        body: "像素网格、共享调色板和可编辑图层，状态可以检查。",
      },
      {
        title: "Agent → 本地 CLI",
        body: "用命令编辑，用 preview 和 diff 检查局部变化。",
      },
      {
        title: "面向游戏引擎导出",
        body: "输出 PNG sheet 与 atlas，保留动画时长和标签，进行结构校验。",
      },
    ],
    ctaInstall: "下载安装",
    ctaDocs: "阅读文档",
    ctaGitHub: "查看 GitHub",
    heroVisualAlt: "港口守灯人四向立绘：蓝大衣、橙围巾、手提灯笼",
    heroEyebrow: "命令进，可编辑像素出。",
    heroMeta: "64 × 64 / 四向",
    demoHeading: "同一角色。可检查的局部修改。",
    demoCaption:
      "一名港口守灯人，四向行走、装备换装、同色板道具与三步局部修改。示例为外部 Agent 通过本地 CLI 制作，不是内置一键生图。",
    localEditCaption: "改围巾，加盾牌，修落脚。保留同一角色，检查每一次变化。",
    localEditNote: "起始落脚偏移是有意的诊断夹具。每一步都有原生 diff 可检查。",
    walkAlt: "港口守灯人四向行走循环",
    propsAlt: "同色板道具与装备换装预览",
    propsHeading: "同一色板，整座港口。",
    propsSub: "同一可编辑演示工程中的四件道具。",
    propLabels: ["背包", "提灯", "木箱", "浮标"],
    localEditAlt: "三步局部修改：围巾换色、加盾、修落脚",
    localEditSteps: ["起始", "围巾换色", "加盾牌", "修落脚"],
    comparisonHeading: "清晰的网格，是刻意的选择。",
    comparisonDisclaimer:
      "柔化缩放示意 / 原生像素网格的最近邻显示。仅比较显示方式，不是模型输出、扩散前后或转换算法基准。",
    comparisonAlt: "示意对比：左侧柔化缩放，右侧最近邻原生工程显示",
    softLabel: "柔化缩放 / 示意",
    nearestLabel: "最近邻显示 / 原生工程",
    videoHeading: "局部编辑短片",
    videoCaption: "围巾、盾牌与落脚修改的可选 CLI 操作短片。",
    statusHeading: "如实说明",
    statusBody:
      "早期 / WIP。预编译 CLI v0.1.0-rc.2 为预发布。正式版前请优先用该标签，不要依赖 GitHub latest。结构校验不等于审美通过。",
    notClaims: [
      "不是完整 Aseprite 克隆",
      "不是扩散假像素",
      "不是无人值守 L4",
      "不是内置一键生图",
    ],
    footerQualification:
      "结构校验不代表审美或游戏内适配通过。示例尚未经过画师盲评，aestheticApproval: false。",
    footerLicense: "代码：MIT OR Apache-2.0",
    footerAssets: "示例美术：CC BY 4.0",
    footerHome: "营销首页",
  },
};

export function readStoredLang(): Lang | null {
  try {
    const value = localStorage.getItem(LANG_STORAGE_KEY);
    if (value === "en" || value === "zh") return value;
  } catch {
    /* ignore */
  }
  return null;
}

export function storeLang(lang: Lang): void {
  try {
    localStorage.setItem(LANG_STORAGE_KEY, lang);
  } catch {
    /* ignore */
  }
}
