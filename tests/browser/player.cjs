// Real browser regression for the bundled, read-only player. No remote model.
const { chromium } = require('playwright');
const { spawn, execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { once } = require('node:events');
const readline = require('node:readline');
const ROOT = path.resolve(__dirname, '../..');
const target = path.resolve(ROOT, process.env.CARGO_TARGET_DIR || 'target');
const BIN = process.env.PIXELFORGE_BIN || path.join(target, 'debug', process.platform === 'win32' ? 'pixelforge.exe' : 'pixelforge');
const ARTIFACTS = process.env.PIXELFORGE_TEST_ARTIFACTS && path.resolve(process.env.PIXELFORGE_TEST_ARTIFACTS);
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

(async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'pixelforge-browser-'));
  const project = path.join(work, 'hero.json');
  let server, browser, page;
  const errors = [];
  try {
    const cli = (...args) => execFileSync(BIN, args, { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    cli('new', '--width', '16', '--height', '16', '--output', project);
    const edits = [
      { tool: 'paint', params: { x: 2, y: 3, colorIndex: 1 } },
      { tool: 'duplicateFrame', params: { index: 0 } },
      { tool: 'duplicateFrame', params: { index: 1 } },
      ...[0, 1, 2].map(index => ({ tool: 'setFrameDelay', params: { index, delayMs: 1000 } })),
      { tool: 'addTag', params: { name: 'go', from: 0, to: 2, loop: 'forward' } },
      { tool: 'addTag', params: { name: 'once', from: 0, to: 2, loop: 'once' } },
    ];
    execFileSync(BIN, ['call', '--project', project], { input: JSON.stringify(edits), stdio: ['pipe', 'pipe', 'pipe'] });
    const before = sha(project), files = fs.readdirSync(work).sort();
    server = spawn(BIN, ['play', '--project', project, '--no-open'], { stdio: ['ignore', 'pipe', 'pipe'] });
    let stderr = '';
    server.stderr.on('data', chunk => { stderr += chunk; });
    const lines = readline.createInterface({ input: server.stdout });
    const abort = new AbortController();
    const timer = setTimeout(() => abort.abort(), 10000);
    let url;
    try {
      const [line] = await once(lines, 'line', { signal: abort.signal });
      url = JSON.parse(line).result.url;
    } catch (error) {
      throw new Error(`Player failed to start: ${stderr}`, { cause: error });
    } finally { clearTimeout(timer); lines.close(); }
    browser = await chromium.launch({
      headless: true,
      ...(process.env.PIXELFORGE_BROWSER_EXECUTABLE ? { executablePath: process.env.PIXELFORGE_BROWSER_EXECUTABLE } : {}),
    });
    page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(url);
    await page.waitForFunction(() => document.querySelectorAll('#action option').length === 2);
    await page.locator('#error').waitFor({ state: 'hidden' });
    await page.locator('#play').click();
    await page.waitForFunction(() => parseFloat(document.querySelector('#time-now').textContent) > 0.1);
    await page.locator('#play').click();
    const paused = await page.locator('#time-now').textContent();
    assert(parseFloat(paused) > 0, 'pause must retain elapsed source time');
    await sleep(150);
    assert.equal(await page.locator('#time-now').textContent(), paused);
    await page.selectOption('#zoom', '4');
    assert.equal(await page.locator('#view').getAttribute('width'), '64');
    assert.equal(await page.locator('#view').getAttribute('role'), 'img');
    await page.locator('#step-fwd').click();
    assert.match(await page.locator('#count').textContent(), /2\s*\/\s*3/);
    await page.locator('body').press('Home');
    assert.match(await page.locator('#count').textContent(), /1\s*\/\s*3/);
    await page.locator('body').press('ArrowRight');
    assert.match(await page.locator('#count').textContent(), /2\s*\/\s*3/);
    await page.locator('#bg-open').click();
    await page.locator('[data-background="white"]').click();
    assert.equal(await page.locator('[data-background="white"]').getAttribute('aria-pressed'), 'true');
    assert.equal(await page.locator('[data-background="checker"]').getAttribute('aria-pressed'), 'false');
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('#bg-open').getAttribute('aria-expanded'), 'false');
    await page.selectOption('#action', 'once');
    await page.locator('#play').click();
    await page.locator('#play').click();
    if (ARTIFACTS) { fs.mkdirSync(ARTIFACTS, { recursive: true }); await page.screenshot({ path: path.join(ARTIFACTS, 'desktop.png') }); }
    // Slow responses must not cause overlapping polling requests to pile up.
    let active = 0, maximum = 0;
    const delayed = async route => {
      active += 1; maximum = Math.max(maximum, active);
      try { await sleep(1100); await route.continue(); } finally { active -= 1; }
    };
    await page.route('**/snapshot', delayed);
    await sleep(1800);
    await page.unrouteAll({ behavior: 'wait' });
    assert.equal(maximum, 1, 'only one snapshot poll may be in flight');
    fs.renameSync(project, project + '.away');
    await page.locator('#error').waitFor({ state: 'visible', timeout: 5000 });
    fs.renameSync(project + '.away', project);
    await page.locator('#error').waitFor({ state: 'hidden', timeout: 5000 });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.selectOption('#zoom', 'auto');
    if (ARTIFACTS) await page.screenshot({ path: path.join(ARTIFACTS, 'mobile.png') });
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), 'mobile layout overflow');
    assert.deepEqual(errors, []);
    assert.equal(sha(project), before);
    assert.deepEqual(fs.readdirSync(work).sort(), files);
    console.log(JSON.stringify({ ok: true, browser: await browser.version(), pageErrors: errors,
      checks: ['render', 'pause/resume', 'integer zoom', 'keyboard step/Home/Escape', 'background selection accessibility', 'single-flight polling', 'file error/recovery', 'mobile layout', 'read-only SHA-256 and sidecars'] }));
  } catch (error) {
    if (page && ARTIFACTS) {
      fs.mkdirSync(ARTIFACTS, { recursive: true });
      await page.screenshot({ path: path.join(ARTIFACTS, 'failure.png') }).catch(() => {});
    }
    throw error;
  } finally {
    if (browser) await browser.close();
    if (server && server.exitCode === null && server.signalCode === null) {
      server.kill(); await once(server, 'exit');
    }
    fs.rmSync(work, { recursive: true, force: true });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
