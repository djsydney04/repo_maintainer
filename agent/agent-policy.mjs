// Explicit profiles keep quota changes separate from agent behavior.
export const profiles = {
  google: {
    key: "GEMINI_API_KEY", defaultModel: "gemini-3.8-flash",
    models: ["gemini-3.8-flash", "gemini-3.6-flash", "gemini-3.5-flash-lite"],
    intervalMs: 13_000, contextWindow: 100_000, maxTokens: 4096,
    reserveTokens: 8192, keepRecentTokens: 12_000, maxInputChars: 300_000,
  },
  groq: {
    key: "GROQ_API_KEY", defaultModel: "openai/gpt-oss-120b",
    models: ["openai/gpt-oss-120b", "qwen/qwen3.8-27b"],
    // Leave room below the published 8K TPM for output and token estimation.
    intervalMs: 61_000, contextWindow: 6000, maxTokens: 1536,
    reserveTokens: 2500, keepRecentTokens: 1200, maxInputChars: 18_000,
  },
};

export function configuration(env) {
  const provider = env.MAINTENANCE_PROVIDER || "google";
  const profile = profiles[provider];
  if (!profile) throw new Error("Choose the google or groq maintenance provider");
  const model = env.MAINTENANCE_MODEL || profile.defaultModel;
  if (!profile.models.includes(model)) throw new Error(`Unsupported ${provider} model: ${model}`);
  const key = env.SDK_MAINTENANCE_API_KEY || env[profile.key];
  if (!key) throw new Error(`Add ${profile.key} (or SDK_MAINTENANCE_API_KEY) to Actions secrets`);
  return { ...profile, provider, model, apiKey: key };
}

export function allowedEdit(name, smoke = false, policy) {
  if (smoke) return name === "src/lib.rs";
  const parts = name.split("/");
  if (name.startsWith("/") || name.includes("\\") || parts.some((part) => part.startsWith("."))) return false;
  if (policy.root_files.includes(name)) return true;
  const parent = parts.slice(0, -1).join("/");
  if (policy.markdown_dirs.includes(parent) && name.endsWith(".md")) return true;
  return policy.source_dirs.some((dir) => name.startsWith(dir + "/")) && policy.extensions.some((ext) => name.endsWith(ext));
}

export function redact(text, secrets) {
  for (const secret of secrets.filter(Boolean).sort((a, b) => b.length - a.length)) {
    text = text.replaceAll(secret, "[redacted]");
  }
  return text;
}

// Count every inference request, including compaction. Never try another key/model.
export function budgetedStream(stream, profile, { maxRequests = 16, now = Date.now,
  sleep = (ms, signal) => import("node:timers/promises").then(({ setTimeout }) => setTimeout(ms, null, { signal })),
} = {}) {
  let requests = 0;
  let lastRequest = -Infinity;
  return async (model, context, options = {}) => {
    if (++requests > maxRequests) throw new Error(`Stopped at the ${maxRequests}-request maintenance budget; partial edits retained`);
    const size = JSON.stringify(context).length;
    if (size > profile.maxInputChars) throw new Error("Context exceeds this provider's free-tier input budget; use a larger-context provider or split the task");
    const wait = profile.intervalMs - (now() - lastRequest);
    if (wait > 0) await sleep(wait, options.signal);
    lastRequest = now();
    return stream(model, context, { ...options, maxTokens: Math.min(options.maxTokens || profile.maxTokens, profile.maxTokens) });
  };
}
