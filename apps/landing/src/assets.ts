/** Marketing CDN — do not commit large media into git. */
export const CDN = "https://resources.pixelforge.torzsi.com/marketing/v1" as const;

export const DEMO_CDN = "https://resources.pixelforge.torzsi.com/demo-zombie/v1" as const;

export const assets = {
  logo: `${CDN}/logo-existing.svg`,
  og: `${CDN}/og-en-1200x630.png`,
  heroAnimated: `${CDN}/hero-en-animated.gif`,
  heroStatic: `${CDN}/hero-en-static.png`,
  gridComparison: `${CDN}/grid-comparison-illustrative.png`,
  propsPreview: `${CDN}/props-preview.png`,
  localEditingStrip: `${CDN}/local-editing-strip.png`,
  localEditingPreview: `${CDN}/local-editing-preview.gif`,
  localEditingPoster: `${CDN}/02-local-editing-poster.png`,
  localEditingVideo: `${CDN}/02-local-editing.mp4`,
  walkFourDirections: `${DEMO_CDN}/a/walk-four-directions.gif`,
} as const;

export const links = {
  install: "https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.2",
  docs: "https://starcloud530.github.io/pixelforge-ai/docs/",
  github: "https://github.com/starcloud530/pixelforge-ai",
} as const;
