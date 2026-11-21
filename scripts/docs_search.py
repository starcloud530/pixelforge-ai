"""Keep Material search results in the article's selected language."""

import json
from pathlib import Path

from mkdocs.plugins import event_priority
from mkdocs.utils import get_relative_url


def on_page_context(context, page, **kwargs):
    if context.get("i18n_page_locale") == "zh":
        # Material reads its search index and resolves result links against this URL.
        context["base_url"] = get_relative_url("zh/", page.url)
    return context


@event_priority(-110)
def on_post_build(config):
    # The i18n plugin finishes its nested builds and merges the search index at -100.
    if config.plugins["i18n"].building:
        return
    root = Path(config.site_dir)
    relative_index = Path("search/search_index.json")
    index = json.loads((root / relative_index).read_text(encoding="utf-8"))
    for locale in ("en", "zh"):
        documents = []
        for entry in index["docs"]:
            location = entry["location"]
            if location.startswith("zh/") == (locale == "zh"):
                documents.append({**entry, "location": location.removeprefix("zh/")})
        localized = {**index, "config": {**index["config"], "lang": [locale]}, "docs": documents}
        if locale == "zh":
            # Jieba marks Chinese word boundaries with zero-width spaces.
            localized["config"]["separator"] = r"[\s\u200b\u3000\-、。，．？！；]+"
        output = (root if locale == "en" else root / "zh") / relative_index
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(localized, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
