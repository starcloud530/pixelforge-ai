/**
 * The local forwarder. Fetch is mocked, so the key never leaves this process.
 */

import assert from "node:assert/strict";
import test from "node:test";
import { forwardChatCompletion, proxyStatus } from "./proxy.js";

const KEY = "server-key-not-real";

test("status reports the endpoint and omits the key", () => {
  const hidden = proxyStatus({
    PIXELFORGE_AI_API_KEY: KEY,
    PIXELFORGE_AI_BASE_URL: "https://astra.example/v1/",
    PIXELFORGE_AI_MODEL: "gpt-6-astra",
  });
  assert.deepEqual(hidden, {
    configured: true,
    model: "gpt-6-astra",
    baseUrl: "https://astra.example/v1",
  });
  assert.equal(JSON.stringify(hidden).includes(KEY), false);
  assert.equal(proxyStatus({}).configured, false);
});

test("forwarding attaches the bearer key and the configured model", async () => {
  const seen: { url: string; init: RequestInit | undefined }[] = [];
  const result = await forwardChatCompletion({
    env: {
      PIXELFORGE_AI_API_KEY: KEY,
      PIXELFORGE_AI_BASE_URL: "https://astra.example/v1",
      PIXELFORGE_AI_MODEL: "gpt-6-astra",
    },
    body: JSON.stringify({
      model: "client-picked-model",
      messages: [{ role: "user", content: "画" }],
    }),
    fetchImpl: (input, init) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      seen.push({ url, init });
      return Promise.resolve(
        new Response(JSON.stringify({ choices: [] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    },
  });
  assert.equal(result.status, 200);
  assert.equal(seen.length, 1);
  const first = seen[0];
  assert.ok(first);
  assert.equal(first.url, "https://astra.example/v1/chat/completions");
  const headers = new Headers(first.init?.headers);
  assert.equal(headers.get("authorization"), `Bearer ${KEY}`);
  const raw = String(first.init?.body);
  assert.equal(raw.includes(KEY), false);
  const body = JSON.parse(raw) as { model: string };
  assert.equal(body.model, "gpt-6-astra");
});

test("a missing key does not call upstream", async () => {
  const result = await forwardChatCompletion({
    env: {},
    body: "{}",
    fetchImpl: () => {
      throw new Error("fetch should not run");
    },
  });
  assert.equal(result.status, 400);
  assert.match(result.body, /PIXELFORGE_AI_API_KEY/);
});
