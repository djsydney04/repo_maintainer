import assert from "node:assert/strict";
import { mkdtemp, mkdir, symlink, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { allowedEdit, budgetedStream, configuration, profiles, redact } from "./agent-policy.mjs";
import { guardedPath } from "./agent.mjs";
const policy = JSON.parse(readFileSync(new URL("../examples/rust-sdks.json", import.meta.url))).editable;

test("switching providers selects its own key and default model", () => {
  const config = configuration({ MAINTENANCE_PROVIDER: "groq", GEMINI_API_KEY: "wrong-provider", GROQ_API_KEY: "groq-key" });
  assert.equal(config.apiKey, "groq-key");
  assert.equal(config.model, "openai/gpt-oss-120b");
  assert.throws(() => configuration({ MAINTENANCE_PROVIDER: "groq", GEMINI_API_KEY: "google-key" }), /Add GROQ_API_KEY/);
  assert.throws(() => configuration({ MAINTENANCE_PROVIDER: "groq", MAINTENANCE_MODEL: "gemini-3.8-flash" }), /Unsupported/);
  assert.equal(configuration({ MAINTENANCE_PROVIDER: "groq", SDK_MAINTENANCE_API_KEY: "replacement" }).apiKey, "replacement");
});

test("quota errors never switch providers or keys", async () => {
  let calls = 0;
  const stream = budgetedStream(() => { calls++; throw new Error("429 quota"); }, profiles.groq);
  await assert.rejects(stream({}, {}), /429 quota/);
  assert.equal(calls, 1);
});

test("every inference including summaries shares the request and pacing budget", async () => {
  let clock = 1000, calls = 0;
  const waits = [];
  const stream = budgetedStream((model, context, options) => { calls++; return options; }, profiles.groq, {
    maxRequests: 2, now: () => clock, sleep: async (ms) => { waits.push(ms); clock += ms; },
  });
  assert.equal((await stream({}, {}, { maxTokens: 9999 })).maxTokens, 1536);
  await stream({}, {});
  await assert.rejects(stream({}, {}), /2-request/);
  assert.deepEqual(waits, [61_000]);
  assert.equal(calls, 2);
});

test("large context is stopped before spending an API request", async () => {
  let calls = 0;
  const stream = budgetedStream(() => { calls++; }, profiles.groq);
  await assert.rejects(stream({}, { text: "x".repeat(18_001) }), /input budget/);
  assert.equal(calls, 0);
});

test("source tools cannot escape through paths or symlinks", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "prism-path-test-"));
  try {
    // macOS /var is a symlink; use the canonical workspace path like the runner.
    const { realpath } = await import("node:fs/promises");
    const source = await realpath(temporary);
    await mkdir(join(source, "crates/core"), { recursive: true });
    await guardedPath(source, "crates/core/new.rs", true, false, policy);
    await assert.rejects(guardedPath(source, "../outside.txt", false, false, policy), /boundary/);
    await assert.rejects(guardedPath(source, ".git/config", false, false, policy), /boundary/);
    await assert.rejects(guardedPath(source, "Cargo.toml", true, false, policy), /approval/);
    await symlink(tmpdir(), join(source, "crates/core/escape"));
    await assert.rejects(guardedPath(source, "crates/core/escape/new.rs", true, false, policy), /Symlink/);
    assert.equal(allowedEdit(".github/workflows/ci.yml", false, policy), false);
    assert.equal(allowedEdit("src/lib.rs", true), true);
    assert.equal(allowedEdit("Cargo.toml", true), false);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});

test("diagnostics redact both the selected key and unused provider credentials", () => {
  assert.equal(redact("groq-key and google-key", ["google-key", "groq-key"]), "[redacted] and [redacted]");
});
