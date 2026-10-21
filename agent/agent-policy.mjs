import { readFileSync } from "node:fs";
export const profiles = JSON.parse(readFileSync(new URL("./providers.json", import.meta.url)));

export function configuration(env, agent = {}) {
  let provider = env.MAINTENANCE_PROVIDER || agent.provider || "google";
  if (provider === "oai") provider = "openai";
  const profile = profiles[provider];
  if (!profile) throw new Error("Choose google, groq, openai or anthropic as the maintenance provider");
  const model = env.MAINTENANCE_MODEL || (env.MAINTENANCE_PROVIDER && provider !== agent.provider ? undefined : agent.model) || profile.defaultModel;
  if (profile.models && !profile.models.includes(model)) throw new Error(`Unsupported ${provider} model: ${model}`);
  const allowPaid = env.MAINTENANCE_ALLOW_PAID ? env.MAINTENANCE_ALLOW_PAID === "true" : agent.allow_paid === true;
  if (profile.paid && !allowPaid) throw new Error(`${provider} requires agent.allow_paid: true; it is never a free-tier fallback`);
  const key = env.SDK_MAINTENANCE_API_KEY || env[profile.key];
  if (!key) throw new Error(`Add ${profile.key} (or SDK_MAINTENANCE_API_KEY) to Actions secrets`);
  return { ...profile, provider, model, apiKey: key, maxRequests: agent.max_requests || 16 };
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
