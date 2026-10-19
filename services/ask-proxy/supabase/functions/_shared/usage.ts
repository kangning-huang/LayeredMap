// The tokens an answer reports (OpenRouter's `usage`), read while the answer streams through.

export interface TokenUsage {
  prompt: number;
  completion: number;
}

function fromUsage(u: unknown): TokenUsage | null {
  if (u === null || typeof u !== "object") return null;
  const r = u as Record<string, unknown>;
  const prompt = Number(r.prompt_tokens ?? 0);
  const completion = Number(r.completion_tokens ?? 0);
  if (!Number.isFinite(prompt) || !Number.isFinite(completion)) return null;
  return { prompt: Math.max(0, Math.floor(prompt)), completion: Math.max(0, Math.floor(completion)) };
}

/** The usage of a whole JSON answer. */
export function usageFromJson(text: string): TokenUsage | null {
  try {
    return fromUsage((JSON.parse(text) as Record<string, unknown>)?.usage);
  } catch {
    return null;
  }
}

/** The usage of a server-sent-events answer: the last `data:` event that carries one. */
export function usageFromSse(text: string): TokenUsage | null {
  let found: TokenUsage | null = null;
  for (const line of text.split("\n")) {
    const l = line.trim();
    if (!l.startsWith("data:")) continue;
    const data = l.slice(5).trim();
    if (data === "[DONE]" || data === "") continue;
    try {
      const u = fromUsage((JSON.parse(data) as Record<string, unknown>)?.usage);
      if (u) found = u;
    } catch {
      // A comment or a partial line: not usage.
    }
  }
  return found;
}

/**
 * Passes an answer through unchanged while keeping enough of it to read the usage at the end:
 * the whole of a JSON answer (up to `cap` bytes), the tail of a stream (the usage is in its last
 * event). `onEnd` runs once the answer has been passed on, before the stream closes.
 */
export function usageTap(
  stream: boolean,
  onEnd: (u: TokenUsage | null) => Promise<void>,
  cap = 2 * 1024 * 1024,
): TransformStream<Uint8Array, Uint8Array> {
  const decoder = new TextDecoder();
  let kept = "";
  let overflow = false;
  return new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, controller) {
      controller.enqueue(chunk);
      kept += decoder.decode(chunk, { stream: true });
      if (kept.length > cap) {
        if (stream) {
          kept = kept.slice(kept.length - 64 * 1024);
        } else {
          overflow = true;
          kept = "";
        }
      }
    },
    async flush() {
      kept += decoder.decode();
      const u = overflow ? null : stream ? usageFromSse(kept) : usageFromJson(kept);
      try {
        await onEnd(u);
      } catch {
        // Counting must never break an answer already sent.
      }
    },
  });
}
