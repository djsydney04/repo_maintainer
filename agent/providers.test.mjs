import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { ModelRuntime } from "@earendil-works/pi-coding-agent";
import { profiles } from "./agent-policy.mjs";

const text = "SDK transport verified";
const item = { id: "msg_test", type: "message", role: "assistant", status: "completed", content: [{ type: "output_text", text, annotations: [] }] };
const events = {
  openai: [
    { type: "response.created", response: { id: "resp_test", status: "in_progress" } },
    { type: "response.output_item.added", output_index: 0, item: { ...item, content: [] } },
    { type: "response.output_text.delta", item_id: "msg_test", output_index: 0, content_index: 0, delta: text },
    { type: "response.output_item.done", output_index: 0, item },
    { type: "response.completed", response: { id: "resp_test", status: "completed", output: [item], usage: { input_tokens: 5, output_tokens: 4, total_tokens: 9, input_tokens_details: { cached_tokens: 0 }, output_tokens_details: { reasoning_tokens: 0 } } } },
  ],
  anthropic: [
    { type: "message_start", message: { id: "msg_test", type: "message", role: "assistant", model: "claude-sonnet-4-6", content: [], stop_reason: null, usage: { input_tokens: 5, output_tokens: 0 } } },
    { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } },
    { type: "content_block_delta", index: 0, delta: { type: "text_delta", text } },
    { type: "content_block_stop", index: 0 },
    { type: "message_delta", delta: { stop_reason: "end_turn", stop_sequence: null }, usage: { output_tokens: 4 } },
    { type: "message_stop" },
  ],
};

for (const provider of ["openai", "anthropic"]) {
  test(`${provider} native SDK sends the expected API request and reads a streamed response`, async () => {
    const directory = await mkdtemp(join(tmpdir(), "maintainer-sdk-test-"));
    try {
      const runtime = await ModelRuntime.create({ authPath: join(directory, "auth.json"), modelsPath: null, modelsStorePath: join(directory, "models.json"), refreshOnCreate: false, allowModelNetwork: false });
      await runtime.setRuntimeApiKey(provider, "sk-test-only-never-a-real-key");
      const model = runtime.getModel(provider, profiles[provider].defaultModel);
      assert.ok(model, "default model exists in the pinned Pi catalog");
      let requests = 0;
      const result = await runtime.completeSimple(model, { messages: [{ role: "user", content: "Verify this transport", timestamp: Date.now() }] }, {
        maxTokens: 100, maxRetries: 0,
        fetch: async (input, init) => {
          requests++;
          const request = new Request(input, init);
          assert.equal(new URL(request.url).pathname, provider === "openai" ? "/v1/responses" : "/v1/messages");
          assert.equal(request.headers.get(provider === "openai" ? "authorization" : "x-api-key"), provider === "openai" ? "Bearer sk-test-only-never-a-real-key" : "sk-test-only-never-a-real-key");
          const body = await request.json();
          assert.equal(body.model, model.id);
          assert.equal(body.stream, true);
          assert.equal(provider === "openai" ? body.max_output_tokens : body.max_tokens, 100);
          return new Response(events[provider].map((event) => `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`).join(""), { headers: { "content-type": "text/event-stream" } });
        },
      });
      assert.equal(result.stopReason, "stop", result.errorMessage);
      assert.equal(result.content.filter((part) => part.type === "text").map((part) => part.text).join(""), text);
      assert.equal(requests, 1);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
}
