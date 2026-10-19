// What the proxy forwards: the chat-completions body NimbleMap's Ask box builds (crates/lm-ask,
// session.rs), reduced to the keys it uses. Anything else (provider routing, plugins such as web
// search, fallback model lists) is dropped, so a token can't be used to buy other things with the
// key. The body itself is not stored or logged.

export interface ShapeOptions {
  /** Models the app may ask for (the evaluated one, docs/lm/llm-eval.md). */
  models: string[];
  /** Upper bound on `max_tokens`. */
  maxTokens: number;
  /** Upper bound on the request body, in bytes. */
  maxBytes: number;
  /** Upper bound on the number of messages. */
  maxMessages: number;
  /** Upper bound on the number of tools. */
  maxTools: number;
}

export const DEFAULT_SHAPE: ShapeOptions = {
  models: ["anthropic/claude-haiku-5.5"],
  maxTokens: 4000,
  maxBytes: 512 * 1024,
  maxMessages: 64,
  maxTools: 64,
};

/** `ASK_MODELS` (comma-separated) replaces the allowed models. */
export function shapeFromEnv(get: (name: string) => string | undefined): ShapeOptions {
  const models = (get("ASK_MODELS") ?? "")
    .split(",")
    .map((m) => m.trim())
    .filter((m) => m.length > 0);
  return { ...DEFAULT_SHAPE, models: models.length > 0 ? models : DEFAULT_SHAPE.models };
}

export type Shaped =
  | { ok: true; body: Record<string, unknown>; stream: boolean }
  | { ok: false; status: number; code: string; message: string };

const ROLES = new Set(["system", "user", "assistant", "tool"]);

function refuse(status: number, code: string, message: string): Shaped {
  return { ok: false, status, code, message };
}

/** Checks and reduces a request body (the raw text, so its size can be checked first). */
export function shapeRequest(raw: string, opts: ShapeOptions): Shaped {
  if (new TextEncoder().encode(raw).length > opts.maxBytes) {
    return refuse(413, "too_large", "The request is too large.");
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return refuse(400, "bad_request", "The request isn't JSON.");
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    return refuse(400, "bad_request", "The request isn't a JSON object.");
  }
  const req = parsed as Record<string, unknown>;
  const model = req.model;
  if (typeof model !== "string" || !opts.models.includes(model)) {
    return refuse(400, "model_not_allowed", "That model isn't offered.");
  }
  const messages = req.messages;
  if (!Array.isArray(messages) || messages.length === 0 || messages.length > opts.maxMessages) {
    return refuse(400, "bad_request", "The request has no messages, or too many.");
  }
  for (const m of messages) {
    if (m === null || typeof m !== "object" || !ROLES.has(String((m as Record<string, unknown>).role))) {
      return refuse(400, "bad_request", "A message has no known role.");
    }
  }
  const body: Record<string, unknown> = { model, messages };
  if (req.tools !== undefined) {
    if (!Array.isArray(req.tools) || req.tools.length > opts.maxTools) {
      return refuse(400, "bad_request", "The tools aren't a list, or there are too many.");
    }
    body.tools = req.tools;
  }
  if (req.tool_choice !== undefined) {
    const tc = req.tool_choice;
    if (!(typeof tc === "string" || (tc !== null && typeof tc === "object" && !Array.isArray(tc)))) {
      return refuse(400, "bad_request", "tool_choice is neither a word nor an object.");
    }
    body.tool_choice = tc;
  }
  if (req.temperature !== undefined) {
    if (typeof req.temperature !== "number" || !Number.isFinite(req.temperature)) {
      return refuse(400, "bad_request", "temperature isn't a number.");
    }
    body.temperature = Math.min(2, Math.max(0, req.temperature));
  }
  const max = typeof req.max_tokens === "number" && Number.isFinite(req.max_tokens) ? Math.floor(req.max_tokens) : opts.maxTokens;
  body.max_tokens = Math.min(opts.maxTokens, Math.max(1, max));
  const stream = req.stream === true;
  if (stream) body.stream = true;
  // The proxy counts tokens from the answer, so it always asks for them.
  body.usage = { include: true };
  return { ok: true, body, stream };
}
