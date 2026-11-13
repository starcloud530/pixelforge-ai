/** Marketing CDN — do not commit large media into git. */
export const CDN = "https://resources.pixelforge.torzsi.com/marketing/v1" as const;

export const DEMO_CDN = "https://resources.pixelforge.torzsi.com/demo-zombie/v1" as const;

export type Lang = "en" | "zh";

export const assets = {
  logo: `${CDN}/logo-existing.svg`,
  propsPreview: `${CDN}/props-preview.png`,
  localEditingPreview: `${CDN}/local-editing-preview.gif`,
  localEditingVideo: `${CDN}/02-local-editing.mp4`,
  walkFourDirections: `${DEMO_CDN}/a/walk-four-directions.gif`,
} as const;

/** Locale-specific marketing composites (baked copy matches the language). */
export const localeMedia = {
  en: {
    og: `${CDN}/og-en-1200x630.png`,
    heroAnimated: `${CDN}/hero-en-animated.gif`,
    heroStatic: `${CDN}/hero-en-static.png`,
    gridComparison: `${CDN}/grid-comparison-illustrative.png`,
    localEditingStrip: `${CDN}/local-editing-strip.png`,
    localEditingPoster: `${CDN}/02-local-editing-poster.png`,
  },
  zh: {
    og: `${CDN}/og-zh-1200x630.png`,
    heroAnimated: `${CDN}/hero-zh-animated.gif`,
    heroStatic: `${CDN}/hero-zh-static.png`,
    gridComparison: `${CDN}/grid-comparison-zh.png`,
    localEditingStrip: `${CDN}/local-editing-strip-zh.png`,
    localEditingPoster: `${CDN}/02-local-editing-poster-zh.png`,
  },
} as const;

export function mediaFor(lang: Lang) {
  return localeMedia[lang];
}

export const links = {
  install: "https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.4",
  docs: "https://starcloud530.github.io/pixelforge-ai/docs/",
  github: "https://github.com/starcloud530/pixelforge-ai",
} as const;
