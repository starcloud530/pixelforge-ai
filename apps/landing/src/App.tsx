import { useEffect, useState } from "react";
import { assets, links, mediaFor } from "./assets";
import { copy, type Copy, type Lang, readStoredLang, storeLang } from "./i18n";

function applyDocumentMeta(lang: Lang): void {
  const t = copy[lang];
  const media = mediaFor(lang);
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  document.title = t.metaTitle;

  const description = document.querySelector('meta[name="description"]');
  if (description) {
    description.setAttribute("content", t.metaDescription);
  }

  const ogTitle = document.querySelector('meta[property="og:title"]');
  if (ogTitle) {
    ogTitle.setAttribute("content", `PixelForge — ${t.brandTagline}`);
  }

  const ogDescription = document.querySelector('meta[property="og:description"]');
  if (ogDescription) {
    ogDescription.setAttribute("content", t.metaDescription);
  }

  const ogImage = document.querySelector('meta[property="og:image"]');
  if (ogImage) {
    ogImage.setAttribute("content", media.og);
  }

  const twitterTitle = document.querySelector('meta[name="twitter:title"]');
  if (twitterTitle) {
    twitterTitle.setAttribute("content", `PixelForge — ${t.brandTagline}`);
  }

  const twitterDescription = document.querySelector('meta[name="twitter:description"]');
  if (twitterDescription) {
    twitterDescription.setAttribute("content", t.metaDescription);
  }

  const twitterImage = document.querySelector('meta[name="twitter:image"]');
  if (twitterImage) {
    twitterImage.setAttribute("content", media.og);
  }
}

function initialLang(): Lang {
  return readStoredLang() ?? "en";
}

export function App() {
  const [lang, setLang] = useState<Lang>(initialLang);
  const t = copy[lang];
  const media = mediaFor(lang);

  useEffect(() => {
    applyDocumentMeta(lang);
    storeLang(lang);
  }, [lang]);

  return (
    <div className="mx-auto min-h-screen w-full max-w-6xl px-5 pb-16 pt-6 sm:px-8 lg:px-10">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded focus:bg-accent focus:px-3 focus:py-2 focus:text-ink"
      >
        Skip to content
      </a>

      <header className="mb-10 flex items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <img src={assets.logo} alt="" width={28} height={28} className="pixelated size-7" />
          <div>
            <p className="text-lg font-semibold tracking-tight text-cream">PixelForge</p>
            <p className="text-xs text-cream-dim">{t.brandTagline}</p>
          </div>
        </div>

        <div
          className="inline-flex items-center gap-1 rounded-md border border-white/10 bg-ink-elevated p-1"
          role="group"
          aria-label={t.langSwitchLabel}
        >
          <LangButton active={lang === "en"} onClick={() => setLang("en")} label={t.langEn} />
          <LangButton active={lang === "zh"} onClick={() => setLang("zh")} label={t.langZh} />
        </div>
      </header>

      <main id="main" className="flex flex-col gap-20">
        <Hero t={t} media={media} />
        <DemoStrip t={t} lang={lang} media={media} />
        <Comparison t={t} media={media} />
        <VideoBlock t={t} media={media} />
        <Status t={t} />
      </main>

      <footer className="mt-20 border-t border-white/10 pt-8 text-sm text-cream-dim">
        <p className="max-w-3xl text-cream-muted">{t.footerQualification}</p>
        <p className="mt-4 flex flex-wrap gap-x-3 gap-y-1">
          <span>{t.footerLicense}</span>
          <span aria-hidden="true">·</span>
          <span>{t.footerAssets}</span>
          <span aria-hidden="true">·</span>
          <a className="text-cream underline-offset-4 hover:text-accent hover:underline" href={links.docs}>
            {t.ctaDocs}
          </a>
          <span aria-hidden="true">·</span>
          <a
            className="text-cream underline-offset-4 hover:text-accent hover:underline"
            href={links.github}
          >
            {t.ctaGitHub}
          </a>
        </p>
      </footer>
    </div>
  );
}

function LangButton({
  active,
  onClick,
  label,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={
        active
          ? "rounded px-2.5 py-1 text-xs font-medium text-ink bg-accent"
          : "rounded px-2.5 py-1 text-xs font-medium text-cream-muted hover:text-cream"
      }
    >
      {label}
    </button>
  );
}

type LocaleMedia = ReturnType<typeof mediaFor>;

/**
 * Locale hero composites already include headline / benefits art.
 * Keep a compact DOM layer for real CTAs, SEO heading, and screen readers.
 */
function Hero({ t, media }: { t: Copy; media: LocaleMedia }) {
  return (
    <section aria-labelledby="hero-heading" className="space-y-6">
      <div className="max-w-3xl">
        <p className="mb-2 font-mono text-[11px] font-medium uppercase tracking-[0.18em] text-accent">
          {t.heroEyebrow}
        </p>
        <h1
          id="hero-heading"
          className="text-3xl font-semibold leading-[1.15] tracking-tight text-cream sm:text-4xl"
        >
          {t.headline}
        </h1>
        <p className="mt-3 text-base leading-relaxed text-cream-muted sm:text-lg">{t.subheading}</p>
      </div>

      <div className="overflow-hidden rounded-2xl border border-white/8 bg-ink-panel p-2 sm:p-3">
        <picture>
          <source srcSet={media.heroStatic} media="(prefers-reduced-motion: reduce)" />
          <img
            src={media.heroAnimated}
            alt={t.heroVisualAlt}
            width={1600}
            height={820}
            className="h-auto w-full rounded-xl"
            decoding="async"
          />
        </picture>
      </div>

      <ol className="grid gap-4 sm:grid-cols-3">
        {t.benefits.map((benefit, index) => (
          <li key={benefit.title} className="max-w-xl">
            <p className="font-mono text-[11px] font-medium uppercase tracking-[0.14em] text-accent">
              {String(index + 1).padStart(2, "0")} / {benefit.title}
            </p>
            <p className="mt-1 text-sm leading-relaxed text-cream-muted">{benefit.body}</p>
          </li>
        ))}
      </ol>

      <nav
        className="flex flex-wrap items-center gap-x-3 gap-y-2 text-base font-medium text-cream"
        aria-label="Primary"
      >
        <a className="underline-offset-4 hover:text-accent hover:underline" href={links.install}>
          {t.ctaInstall}
        </a>
        <span className="text-cream-dim" aria-hidden="true">
          /
        </span>
        <a className="underline-offset-4 hover:text-accent hover:underline" href={links.docs}>
          {t.ctaDocs}
        </a>
        <span className="text-cream-dim" aria-hidden="true">
          /
        </span>
        <a className="underline-offset-4 hover:text-accent hover:underline" href={links.github}>
          {t.ctaGitHub}
        </a>
      </nav>
    </section>
  );
}

function DemoStrip({
  t,
  lang,
  media,
}: {
  t: Copy;
  lang: Lang;
  media: LocaleMedia;
}) {
  return (
    <section aria-labelledby="demo-heading" className="space-y-6">
      <div className="max-w-3xl">
        <h2 id="demo-heading" className="text-2xl font-semibold tracking-tight text-cream sm:text-3xl">
          {t.demoHeading}
        </h2>
        <p className="mt-3 text-base leading-relaxed text-cream-muted">{t.demoCaption}</p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <figure className="overflow-hidden rounded-2xl border border-white/8 bg-ink-panel p-3">
          <WalkStrip
            alt={t.walkAlt}
            unlabeled={lang === "zh"}
            className="mx-auto w-full max-w-[320px]"
            loading="lazy"
          />
        </figure>

        {lang === "en" ? (
          <figure className="overflow-hidden rounded-2xl border border-white/8 bg-ink-panel p-3 md:col-span-2">
            <img
              src={assets.propsPreview}
              alt={t.propsAlt}
              width={1200}
              height={430}
              className="h-auto w-full"
              loading="lazy"
              decoding="async"
            />
          </figure>
        ) : (
          <figure className="overflow-hidden rounded-2xl border border-white/8 bg-ink-panel p-4 md:col-span-2">
            <p className="font-mono text-[11px] font-medium uppercase tracking-[0.14em] text-accent">
              {t.propsHeading}
            </p>
            <p className="mt-2 text-sm text-cream-muted">{t.propsSub}</p>
            <ul className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-4">
              {t.propLabels.map((label) => (
                <li
                  key={label}
                  className="rounded-lg border border-white/8 bg-ink px-3 py-4 text-center text-sm text-cream"
                >
                  {label}
                </li>
              ))}
            </ul>
          </figure>
        )}
      </div>

      <figure className="overflow-hidden rounded-2xl border border-white/8 bg-ink-panel p-3">
        <div className="grid gap-3 md:grid-cols-[1.4fr_1fr] md:items-center">
          <img
            src={media.localEditingStrip}
            alt={t.localEditAlt}
            width={1400}
            height={510}
            className="h-auto w-full"
            loading="lazy"
            decoding="async"
          />
          <div>
            <CroppedLocalPreview alt={t.localEditAlt} />
            <p className="mt-3 text-sm leading-relaxed text-cream-muted">{t.localEditCaption}</p>
            {lang === "zh" ? (
              <p className="mt-2 text-xs leading-relaxed text-cream-dim">{t.localEditNote}</p>
            ) : null}
          </div>
        </div>
      </figure>
    </section>
  );
}

/**
 * Walk GIF is 1024×352 with English title + SOUTH/WEST/EAST/NORTH labels.
 * Crop to the sprite band (~y 63–274) for ZH so demo strip stays unlabeled.
 */
function WalkStrip({
  alt,
  unlabeled,
  className = "",
  loading,
}: {
  alt: string;
  unlabeled: boolean;
  className?: string;
  loading?: "lazy" | "eager";
}) {
  if (!unlabeled) {
    return (
      <img
        src={assets.walkFourDirections}
        alt={alt}
        width={1024}
        height={352}
        className={`pixelated h-auto w-full ${className}`}
        loading={loading}
        decoding="async"
      />
    );
  }

  return (
    <div className={`relative w-full overflow-hidden ${className}`} style={{ aspectRatio: "1024 / 211" }}>
      <img
        src={assets.walkFourDirections}
        alt={alt}
        width={1024}
        height={352}
        className="pixelated absolute left-0 w-full max-w-none"
        style={{ top: "-29.86%" }}
        loading={loading}
        decoding="async"
      />
    </div>
  );
}

/** Hide the baked “BASE / FIXTURE” label in the top-left of the GIF. */
function CroppedLocalPreview({ alt }: { alt: string }) {
  return (
    <div className="mx-auto aspect-[640/520] w-full max-w-xs overflow-hidden rounded-xl bg-ink">
      <img
        src={assets.localEditingPreview}
        alt={alt}
        width={640}
        height={520}
        className="h-[118%] w-[118%] max-w-none -translate-x-[6%] -translate-y-[10%] object-cover"
        loading="lazy"
        decoding="async"
      />
    </div>
  );
}

function Comparison({ t, media }: { t: Copy; media: LocaleMedia }) {
  return (
    <section aria-labelledby="comparison-heading" className="space-y-5">
      <div className="max-w-3xl">
        <h2
          id="comparison-heading"
          className="text-2xl font-semibold tracking-tight text-cream sm:text-3xl"
        >
          {t.comparisonHeading}
        </h2>
        <p className="mt-3 text-sm leading-relaxed text-cream-dim sm:text-base">{t.comparisonDisclaimer}</p>
      </div>
      <figure className="overflow-hidden rounded-2xl border border-white/8 bg-ink-panel p-2 sm:p-3">
        <img
          src={media.gridComparison}
          alt={t.comparisonAlt}
          width={1400}
          height={640}
          className="h-auto w-full"
          loading="lazy"
          decoding="async"
        />
        <figcaption className="sr-only">{t.comparisonDisclaimer}</figcaption>
      </figure>
    </section>
  );
}

function VideoBlock({ t, media }: { t: Copy; media: LocaleMedia }) {
  return (
    <section aria-labelledby="video-heading" className="space-y-5">
      <div className="max-w-3xl">
        <h2 id="video-heading" className="text-2xl font-semibold tracking-tight text-cream sm:text-3xl">
          {t.videoHeading}
        </h2>
        <p className="mt-3 text-base leading-relaxed text-cream-muted">{t.videoCaption}</p>
      </div>
      <div className="overflow-hidden rounded-2xl border border-white/8 bg-ink-panel p-2 sm:p-3">
        <video
          className="aspect-video h-auto w-full rounded-xl bg-ink"
          controls
          preload="metadata"
          poster={media.localEditingPoster}
          playsInline
        >
          <source src={assets.localEditingVideo} type="video/mp4" />
        </video>
      </div>
    </section>
  );
}

function Status({ t }: { t: Copy }) {
  return (
    <section
      aria-labelledby="status-heading"
      className="rounded-2xl border border-white/8 bg-ink-elevated px-5 py-6 sm:px-8"
    >
      <h2 id="status-heading" className="text-xl font-semibold tracking-tight text-cream sm:text-2xl">
        {t.statusHeading}
      </h2>
      <p className="mt-3 max-w-3xl text-base leading-relaxed text-cream-muted">{t.statusBody}</p>
      <ul className="mt-5 grid gap-2 sm:grid-cols-2">
        {t.notClaims.map((item) => (
          <li key={item} className="flex items-start gap-2 text-sm text-cream-muted">
            <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-accent" aria-hidden="true" />
            <span>{item}</span>
          </li>
        ))}
      </ul>
      <p className="mt-5 font-mono text-xs text-cream-dim">
        aestheticApproval: false · v0.1.0-rc.5
      </p>
    </section>
  );
}
