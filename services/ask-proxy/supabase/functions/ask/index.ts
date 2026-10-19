// The Ask proxy (Supabase Edge Function `ask`): the Ask box's chat-completions call, for a signed-in
// Pro user within today's allowance, forwarded to OpenRouter with the key held here only.
// Secrets: OPENROUTER_API_KEY; optional ASK_DAILY_REQUESTS, ASK_DAILY_TOKENS, ASK_MODELS.
// SUPABASE_URL, SUPABASE_ANON_KEY and SUPABASE_SERVICE_ROLE_KEY are provided by Supabase.

import { createClient } from "npm:@supabase/supabase-js@2";

import { handleAsk } from "../_shared/handler.ts";
import { limitsFromEnv, reservationFromRpc } from "../_shared/limits.ts";
import { shapeFromEnv } from "../_shared/shape.ts";

const env = (name: string) => Deno.env.get(name);
const admin = createClient(env("SUPABASE_URL") ?? "", env("SUPABASE_SERVICE_ROLE_KEY") ?? "", {
  auth: { persistSession: false, autoRefreshToken: false },
});
const key = env("OPENROUTER_API_KEY") ?? "";
const limits = limitsFromEnv(env);
const shape = shapeFromEnv(env);

Deno.serve((req) => {
  if (!key) return new Response(JSON.stringify({ error: { code: "misconfigured", message: "The proxy has no model key." } }), { status: 500 });
  return handleAsk(req, {
    verify: async (token) => {
      const { data, error } = await admin.auth.getClaims(token);
      return error || !data ? null : data.claims;
    },
    now: () => new Date(),
    log: (line) => console.log(line),
    isPro: async (id) => {
      const { data, error } = await admin.from("profiles").select("pro").eq("id", id).maybeSingle();
      return error || !data ? null : data.pro === true;
    },
    reserve: async (id, day, l) => {
      const { data, error } = await admin.rpc("reserve_request", {
        p_user: id,
        p_day: day,
        p_max_requests: l.requests,
        p_max_tokens: l.tokens,
      });
      return error ? null : reservationFromRpc(data);
    },
    record: async (id, day, u) => {
      await admin.rpc("record_tokens", { p_user: id, p_day: day, p_prompt: u.prompt, p_completion: u.completion });
    },
    upstream: (body) =>
      fetch("https://openrouter.ai/api/v1/chat/completions", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${key}`,
          "Content-Type": "application/json",
          "HTTP-Referer": "https://layeredmap.app",
          // Keep in step with lm_core::brand::PRO_NAME (docs/lm/naming.md).
          "X-Title": "NimbleMap Pro",
        },
        body: JSON.stringify(body),
      }),
    limits,
    shape,
  });
});
