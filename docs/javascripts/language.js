// Both GitHub Pages sites share this preference on the same origin.
// Explicit localized links work without JavaScript, including in a new tab.
const locale = document.documentElement.lang.startsWith("zh") ? "zh" : "en";
const storageKey = "pixelforge.language";
const switches = document.querySelectorAll("a[data-language]");
for (const link of switches) {
  const target = new URL(link.href);
  function syncLocation() {
    target.search = location.search;
    target.hash = location.hash;
    link.href = target.href;
  }
  syncLocation();
  window.addEventListener("hashchange", syncLocation);
  link.addEventListener("click", () => {
    syncLocation();
    try { localStorage.setItem(storageKey, link.dataset.language); } catch {}
  });
}
try {
  if (locale === "zh") localStorage.setItem(storageKey, "zh");
  const isEntry = /^\/pixelforge-(ai|demos)\/(?:index\.html)?$/.test(location.pathname);
  if (isEntry && localStorage.getItem(storageKey) === "zh") {
    const translation = document.querySelector('a[data-language="zh"]');
    if (translation) location.replace(translation.href);
  }
} catch { /* Language links still work when storage is unavailable. */ }
