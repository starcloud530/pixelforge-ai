#!/usr/bin/env python3
"""Offline checks for local Markdown targets, tool registries and site layout.

This does not check external HTTP links or all anchor/schema semantics.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if not __debug__:
    raise RuntimeError("Run checks without Python -O; assertions are required")
errors=[];files=[]
for p in sorted({*ROOT.glob('*.md'), *ROOT.glob('docs/**/*.md'), *ROOT.glob('skills/**/*.md'), *ROOT.glob('crates/*/tests/fixtures/**/*.md')}):
 files.append(p)
 for n,l in enumerate(p.read_text().splitlines(),1):
  for dest in re.findall(r'\[[^\]]*\]\(([^\s)]+)',l):
   if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:|^#|^/',dest):continue
   target=dest.split('#')[0]
   if target and not (p.parent/target).exists():errors.append({'file':str(p.relative_to(ROOT)),'line':n,'target':dest})

assert not errors, json.dumps(errors, indent=2)
source = (ROOT / "crates/pixelforge-kernel/src/tool_api.rs").read_text()
def registry(name):
    match = re.search(r"pub const " + name + r": &\[&str\] = &\[([\s\S]*?)\];", source)
    assert match, name
    return re.findall(r'"([A-Za-z]+)"', match[1])
tools = registry("TOOL_NAMES")
ts = (ROOT / "apps/web/src/kernel/toolApi.ts").read_text().split("export const TOOL_NAMES = [", 1)[1].split("] as const;", 1)[0]
assert re.findall(r'"([A-Za-z]+)"', ts) == tools, "TypeScript/Rust tool registries differ"
catalog = (ROOT / "apps/web/src/ai/catalog.ts").read_text()
catalog_names = re.findall(r'\w+: tool\(\s*"([A-Za-z]+)"', catalog)
assert len(catalog_names) == len(tools) and set(catalog_names) == set(tools), "AI catalog/Rust registries differ"
pages = set((ROOT / "docs").rglob("*.md"))
english_pages = {p for p in pages if not p.name.endswith(".zh.md")}
translated_pages = {p for p in pages if p.name.endswith(".zh.md")}
expected_translations = {p.with_suffix(".zh.md") for p in english_pages}
assert translated_pages == expected_translations, f"Unmatched translations: {translated_pages ^ expected_translations}"
api_pages = {p.name: p.read_text() for p in (ROOT / "docs").glob("tool-api*.md")}
for filename, doc in api_pages.items():
    for name in tools:
        assert f"`{name}(" in doc, f"{filename} omits the signature for {name}"
# Map/manifest registries live in separate kernel modules.
for module, name in [("tilemap", "TILEMAP_TOOL_NAMES"), ("manifest", "MANIFEST_TOOL_NAMES")]:
    for file in (ROOT / "crates/pixelforge-kernel/src").glob("*.rs"):
        text = file.read_text()
        match = re.search(r"pub const " + name + r": &\[&str\] = &\[([\s\S]*?)\];", text)
        if match:
            for tool in re.findall(r'"([A-Za-z]+)"', match[1]):
                for filename, doc in api_pages.items():
                    assert f"`{tool}`" in doc, f"{filename} omits {tool}"
            break
    else:
        raise AssertionError(f"Missing {name}")
assert 'https://starcloud530.github.io/pixelforge-ai/docs/' not in (ROOT / 'apps/landing/src/assets.ts').read_text()
assert 'path: site' in (ROOT / '.github/workflows/pages.yml').read_text()
assert 'https://starcloud530.github.io/pixelforge-ai/' in (ROOT / 'docs/install.md').read_text()
print(f"PASS: {len(files)} Markdown files, 0 broken local targets; {len(tools)} Rust/TypeScript/AI tools agree; Pages links agree")
