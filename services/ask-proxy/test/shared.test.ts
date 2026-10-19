// The proxy's logic under Node's built-in test runner (no Deno, no Supabase): `npm test` here, or
// `node --test test/*.test.ts` (Node 22.18+ runs TypeScript by stripping its types).

import assert from "node:assert/strict";
import { test } from "node:test";

import { bearer, userFromClaims } from "../supabase/functions/_shared/auth.ts";
import { type AccountDeps, type AskDeps, handleAccount, handleAsk } from "../supabase/functions/_shared/handler.ts";
import {
  DEFAULT_LIMITS,
  type Limits,
  limitsFromEnv,
  type Reservation,
  reservationFromRpc,
  secondsUntilMonthReset,
  secondsUntilReset,
  utcDay,
} from "../supabase/functions/_shared/limits.ts";
import { DEFAULT_SHAPE, shapeFromEnv, shapeRequest } from "../supabase/functions/_shared/shape.ts";
import { type TokenUsage, usageFromJson, usageFromSse } from "../supabase/functions/_shared/usage.ts";

const USER = "6f1d2c3b-4a59-4e8f-9a0b-1c2d3e4f5a6b";
const NOW = new Date("2026-10-19T22:30:00Z");
const NOW_S = Math.floor(NOW.getTime() / 1000);
const MODEL = DEFAULT_SHAPE.models[0];

// ── limits ──────────────────────────────────────────────────────────────────────────────────────

test("limits come from the secrets, with defaults for missing or bad values", () => {
  assert.deepEqual(limitsFromEnv(() => undefined), DEFAULT_LIMITS);
  const env: Record<string, string> = { ASK_DAILY_REQUESTS: " 30 ", ASK_DAILY_TOKENS: "-5" };
  assert.deepEqual(limitsFromEnv((k) => env[k]), { ...DEFAULT_LIMITS, requests: 30 });
  const month: Record<string, string> = { ASK_MONTHLY_REQUESTS: "900", ASK_MONTHLY_TOKENS: "x" };
  assert.deepEqual(limitsFromEnv((k) => month[k]), { ...DEFAULT_LIMITS, monthRequests: 900 });
  // About 300 questions a month at 2–4 model calls each (owner, 2026-10-19).
  assert.deepEqual([DEFAULT_LIMITS.monthRequests, DEFAULT_LIMITS.monthTokens], [1_000, 3_000_000]);
  assert.equal(limitsFromEnv((k) => (k === "ASK_DAILY_REQUESTS" ? "1e3" : undefined)).requests, DEFAULT_LIMITS.requests);
  assert.equal(limitsFromEnv((k) => (k === "ASK_DAILY_REQUESTS" ? "0" : undefined)).requests, DEFAULT_LIMITS.requests);
});

test("the day is the UTC day, and the allowance resets at UTC midnight", () => {
  assert.equal(utcDay(NOW), "2026-10-19");
  assert.equal(utcDay(new Date("2026-10-19T23:59:59.999-08:00")), "2026-10-20");
  assert.equal(secondsUntilReset(NOW), 90 * 60);
  assert.equal(secondsUntilReset(new Date("2026-10-19T23:59:59.500Z")), 1);
  assert.equal(secondsUntilMonthReset(new Date("2026-10-31T23:00:00Z")), 3600);
  assert.equal(secondsUntilMonthReset(new Date("2026-12-31T23:59:59.500Z")), 1);
});

test("reserve_request's row reads as a reservation", () => {
  assert.deepEqual(reservationFromRpc([{ allowed: true, requests: 3, tokens: "1200", month_requests: 40, month_tokens: "9000", blocked: null }]), {
    allowed: true, requests: 3, tokens: 1200, monthRequests: 40, monthTokens: 9000, blocked: null,
  });
  assert.deepEqual(reservationFromRpc({ allowed: false, requests: 5, tokens: 0, month_requests: 1000, blocked: "month" }), {
    allowed: false, requests: 5, tokens: 0, monthRequests: 1000, monthTokens: 0, blocked: "month",
  });
  assert.equal(reservationFromRpc({ allowed: false, requests: 60, tokens: 0, blocked: "week" })?.blocked, null);
  assert.equal(reservationFromRpc([]), null);
  assert.equal(reservationFromRpc([{ requests: 1 }]), null);
});

// ── request shaping ─────────────────────────────────────────────────────────────────────────────

function appBody(extra: Record<string, unknown> = {}) {
  // The shape crates/lm-ask/src/session.rs sends.
  return {
    model: MODEL,
    messages: [
      { role: "system", content: [{ type: "text", text: "rules", cache_control: { type: "ephemeral" } }] },
      { role: "user", content: "schools within 500 m of a river" },
    ],
    tools: [{ type: "function", function: { name: "submit_plan", parameters: { type: "object" } } }],
    tool_choice: "required",
    temperature: 0,
    max_tokens: 2500,
    usage: { include: true },
    ...extra,
  };
}

test("the app's body passes unchanged, apart from the forced usage flag", () => {
  const r = shapeRequest(JSON.stringify(appBody()), DEFAULT_SHAPE);
  assert.ok(r.ok);
  assert.deepEqual(r.body, appBody());
  assert.equal(r.stream, false);
});

test("keys the app doesn't use are dropped, max_tokens and temperature are clamped", () => {
  const r = shapeRequest(
    JSON.stringify(
      appBody({
        provider: { order: ["expensive"] },
        plugins: [{ id: "web" }],
        models: ["openai/o3-pro"],
        transforms: ["middle-out"],
        max_tokens: 1_000_000,
        temperature: 9,
        usage: { include: false },
        stream: true,
      }),
    ),
    DEFAULT_SHAPE,
  );
  assert.ok(r.ok);
  for (const k of ["provider", "plugins", "models", "transforms"]) assert.equal(k in r.body, false, k);
  assert.equal(r.body.max_tokens, DEFAULT_SHAPE.maxTokens);
  assert.equal(r.body.temperature, 2);
  assert.deepEqual(r.body.usage, { include: true });
  assert.equal(r.stream, true);
});

test("bad requests are refused with a reason", () => {
  const cases: [string, number, string][] = [
    ["not json", 400, "bad_request"],
    ["[1,2]", 400, "bad_request"],
    [JSON.stringify(appBody({ model: "openai/o3-pro" })), 400, "model_not_allowed"],
    [JSON.stringify(appBody({ messages: [] })), 400, "bad_request"],
    [JSON.stringify(appBody({ messages: [{ role: "developer", content: "x" }] })), 400, "bad_request"],
    [JSON.stringify(appBody({ tools: "all" })), 400, "bad_request"],
    [JSON.stringify(appBody({ tool_choice: 3 })), 400, "bad_request"],
    [JSON.stringify(appBody({ temperature: "hot" })), 400, "bad_request"],
    ["x".repeat(DEFAULT_SHAPE.maxBytes + 1), 413, "too_large"],
  ];
  for (const [raw, status, code] of cases) {
    const r = shapeRequest(raw, DEFAULT_SHAPE);
    assert.ok(!r.ok, raw.slice(0, 40));
    assert.equal(r.status, status, raw.slice(0, 40));
    assert.equal(r.code, code, raw.slice(0, 40));
  }
});

test("ASK_MODELS replaces the allowed models", () => {
  assert.deepEqual(shapeFromEnv((k) => (k === "ASK_MODELS" ? " a/b , c/d ," : undefined)).models, ["a/b", "c/d"]);
  assert.deepEqual(shapeFromEnv(() => undefined).models, DEFAULT_SHAPE.models);
});

// ── usage ───────────────────────────────────────────────────────────────────────────────────────

test("usage is read from a JSON answer and from the last event of a stream", () => {
  assert.deepEqual(usageFromJson('{"choices":[],"usage":{"prompt_tokens":1200,"completion_tokens":85,"cost":0.001}}'), {
    prompt: 1200,
    completion: 85,
  });
  assert.equal(usageFromJson('{"choices":[]}'), null);
  assert.equal(usageFromJson("cut off {"), null);
  const sse = [
    ": OPENROUTER PROCESSING",
    'data: {"choices":[{"delta":{"content":"Hi"}}]}',
    'data: {"choices":[{"delta":{}}],"usage":{"prompt_tokens":10,"completion_tokens":2}}',
    "data: [DONE]",
    "",
  ].join("\n");
  assert.deepEqual(usageFromSse(sse), { prompt: 10, completion: 2 });
  assert.equal(usageFromSse("data: [DONE]\n"), null);
});

// ── auth ────────────────────────────────────────────────────────────────────────────────────────

test("only a bearer token of token characters is taken", () => {
  assert.equal(bearer("Bearer abc.def-ghi_jkl"), "abc.def-ghi_jkl");
  assert.equal(bearer("bearer abc"), null);
  assert.equal(bearer("Basic abc"), null);
  assert.equal(bearer("Bearer a b"), null);
  assert.equal(bearer(null), null);
});

test("claims must be a signed-in user's, unexpired, with a user id", () => {
  const ok = { sub: USER, aud: "authenticated", role: "authenticated", exp: NOW_S + 60 };
  assert.equal(userFromClaims(ok, NOW_S), USER);
  assert.equal(userFromClaims({ ...ok, aud: ["authenticated", "x"] }, NOW_S), USER);
  assert.equal(userFromClaims({ ...ok, exp: NOW_S }, NOW_S), null);
  assert.equal(userFromClaims({ ...ok, role: "anon", aud: "authenticated" }, NOW_S), null);
  assert.equal(userFromClaims({ ...ok, role: "service_role" }, NOW_S), null);
  assert.equal(userFromClaims({ ...ok, sub: "not-a-uuid" }, NOW_S), null);
  assert.equal(userFromClaims({ ...ok, exp: "later" }, NOW_S), null);
  assert.equal(userFromClaims(null, NOW_S), null);
});

// ── the ask handler ─────────────────────────────────────────────────────────────────────────────

interface Seen {
  upstream: Record<string, unknown>[];
  recorded: TokenUsage[];
  logs: string[];
}

function askDeps(over: Partial<AskDeps> = {}, seen: Seen = { upstream: [], recorded: [], logs: [] }): AskDeps {
  return {
    verify: async (t) => (t === "good" ? { sub: USER, aud: "authenticated", role: "authenticated", exp: NOW_S + 3600 } : null),
    now: () => NOW,
    log: (l) => seen.logs.push(l),
    isPro: async () => true,
    reserve: async (): Promise<Reservation> => ({ allowed: true, requests: 1, tokens: 0 }),
    record: async (_u, _d, u) => {
      seen.recorded.push(u);
    },
    upstream: async (body) => {
      seen.upstream.push(body);
      return new Response(JSON.stringify({ choices: [{ message: { content: "ok" } }], usage: { prompt_tokens: 7, completion_tokens: 3 } }), {
        status: 200,
      });
    },
    limits: DEFAULT_LIMITS,
    shape: DEFAULT_SHAPE,
    ...over,
  };
}

function askRequest(token: string | null, body: unknown = appBody(), method = "POST") {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers.Authorization = `Bearer ${token}`;
  return new Request("https://example.supabase.co/functions/v1/ask", {
    method,
    headers,
    body: method === "GET" ? undefined : typeof body === "string" ? body : JSON.stringify(body),
  });
}

async function errorCode(r: Response) {
  return ((await r.json()) as { error: { code: string } }).error.code;
}

test("a signed-in Pro user's request is forwarded and its tokens recorded", async () => {
  const seen: Seen = { upstream: [], recorded: [], logs: [] };
  const r = await handleAsk(askRequest("good"), askDeps({}, seen));
  assert.equal(r.status, 200);
  const answer = (await r.json()) as { choices: { message: { content: string } }[] };
  assert.equal(answer.choices[0].message.content, "ok");
  assert.deepEqual(seen.upstream, [appBody()]);
  assert.deepEqual(seen.recorded, [{ prompt: 7, completion: 3 }]);
  // The log names the user and the counts, never what was asked.
  assert.ok(seen.logs.every((l) => !l.includes("river")));
});

test("a stream is passed through and counted from its last event", async () => {
  const seen: Seen = { upstream: [], recorded: [], logs: [] };
  const sse = 'data: {"choices":[{"delta":{"content":"a"}}]}\n\ndata: {"usage":{"prompt_tokens":5,"completion_tokens":1}}\n\ndata: [DONE]\n\n';
  const deps = askDeps(
    {
      upstream: async (b) => {
        seen.upstream.push(b);
        return new Response(sse, { status: 200 });
      },
    },
    seen,
  );
  const r = await handleAsk(askRequest("good", appBody({ stream: true })), deps);
  assert.equal(r.headers.get("Content-Type"), "text/event-stream");
  assert.equal(await r.text(), sse);
  assert.deepEqual(seen.recorded, [{ prompt: 5, completion: 1 }]);
});

test("no token, a bad token, a GET: refused before anything is spent", async () => {
  const seen: Seen = { upstream: [], recorded: [], logs: [] };
  const deps = askDeps({}, seen);
  assert.equal((await handleAsk(askRequest(null), deps)).status, 401);
  assert.equal((await handleAsk(askRequest("forged"), deps)).status, 401);
  assert.equal((await handleAsk(askRequest("good", undefined, "GET"), deps)).status, 405);
  const throws = askDeps({ verify: async () => Promise.reject(new Error("jwks down")) }, seen);
  assert.equal((await handleAsk(askRequest("good"), throws)).status, 401);
  assert.equal(seen.upstream.length, 0);
});

test("not Pro, no profile, a bad body: refused", async () => {
  const seen: Seen = { upstream: [], recorded: [], logs: [] };
  let r = await handleAsk(askRequest("good"), askDeps({ isPro: async () => false }, seen));
  assert.equal(r.status, 403);
  assert.equal(await errorCode(r), "not_pro");
  r = await handleAsk(askRequest("good"), askDeps({ isPro: async () => null }, seen));
  assert.equal(await errorCode(r), "no_profile");
  r = await handleAsk(askRequest("good", appBody({ model: "x/y" })), askDeps({}, seen));
  assert.equal(await errorCode(r), "model_not_allowed");
  assert.equal(seen.upstream.length, 0);
});

test("over the daily allowance: 429 with when it resets, and nothing forwarded", async () => {
  const seen: Seen = { upstream: [], recorded: [], logs: [] };
  const r = await handleAsk(askRequest("good"), askDeps({ reserve: async () => ({ allowed: false, requests: 60, tokens: 9 }) }, seen));
  assert.equal(r.status, 429);
  const e = ((await r.json()) as { error: Record<string, unknown> }).error;
  assert.equal(e.code, "daily_limit");
  assert.equal(e.resets_in_seconds, 90 * 60);
  assert.equal(e.limit_requests, DEFAULT_LIMITS.requests);
  assert.equal(seen.upstream.length, 0);
});

test("over the monthly allowance: 429 monthly_limit, reset on the first of next month", async () => {
  const seen: Seen = { upstream: [], recorded: [], logs: [] };
  const deps = askDeps({ reserve: async () => ({ allowed: false, requests: 4, tokens: 9, monthRequests: 1_000, blocked: "month" as const }) }, seen);
  const r = await handleAsk(askRequest("good"), deps);
  assert.equal(r.status, 429);
  const e = ((await r.json()) as { error: Record<string, unknown> }).error;
  assert.equal(e.code, "monthly_limit");
  assert.equal(e.resets_in_seconds, secondsUntilMonthReset(NOW));
  assert.equal(e.limit_requests, DEFAULT_LIMITS.monthRequests);
  assert.equal(seen.upstream.length, 0);
  assert.ok(seen.logs.some((l) => l.includes("status=429 month")));
});

test("the allowance counts each user's calls separately and stops at the limit", async () => {
  // An in-memory reserve_request with the SQL function's rule.
  const rows = new Map<string, { requests: number; tokens: number }>();
  const reserve = async (u: string, d: string, l: Limits) => {
    const k = `${u}/${d}`;
    const row = rows.get(k) ?? { requests: 0, tokens: 0 };
    rows.set(k, row);
    const month = [...rows].filter(([key]) => key.startsWith(`${u}/${d.slice(0, 7)}`)).map(([, v]) => v);
    const mReq = month.reduce((a, v) => a + v.requests, 0);
    const mTok = month.reduce((a, v) => a + v.tokens, 0);
    if (row.requests >= l.requests || row.tokens >= l.tokens) {
      return { allowed: false, requests: row.requests, tokens: row.tokens, blocked: "day" as const };
    }
    if (mReq >= l.monthRequests || mTok >= l.monthTokens) {
      return { allowed: false, requests: row.requests, tokens: row.tokens, blocked: "month" as const };
    }
    row.requests += 1;
    return { allowed: true, requests: row.requests, tokens: row.tokens, blocked: null };
  };
  const record = async (u: string, d: string, t: TokenUsage) => {
    const row = rows.get(`${u}/${d}`);
    if (row) row.tokens += t.prompt + t.completion;
  };
  // Tokens are recorded once the answer has been read through, as the app reads it.
  const ask = async (deps: AskDeps) => {
    const r = await handleAsk(askRequest("good"), deps);
    await r.text();
    return r.status;
  };
  const deps = askDeps({ reserve, record, limits: { ...DEFAULT_LIMITS, requests: 3, tokens: 1_000 } });
  const statuses = [];
  for (let i = 0; i < 4; i++) statuses.push(await ask(deps));
  assert.deepEqual(statuses, [200, 200, 200, 429]);
  // 30 tokens used of 1,000: the request count stopped it. Then a token cap stops it too.
  const tight = askDeps({ reserve, record, limits: { ...DEFAULT_LIMITS, requests: 100, tokens: 35 } });
  assert.equal(await ask(tight), 200);
  assert.equal(await ask(tight), 429);
  // The month counts every day of it: earlier days' calls use up the monthly allowance.
  rows.clear();
  rows.set(`${USER}/2026-10-02`, { requests: 5, tokens: 0 });
  const monthly = askDeps({ reserve, record, limits: { ...DEFAULT_LIMITS, monthRequests: 6 } });
  assert.equal(await ask(monthly), 200);
  assert.equal(await ask(monthly), 429);
});

test("the model service's failures become the proxy's own words, never its message", async () => {
  const cases: [number, number, string][] = [
    [402, 503, "upstream_credit"],
    [429, 503, "upstream_busy"],
    [400, 502, "upstream_rejected"],
    [401, 502, "upstream"],
    [500, 502, "upstream"],
  ];
  for (const [up, status, code] of cases) {
    const deps = askDeps({
      upstream: async () => new Response('{"error":{"message":"key sk-or-v1-secret for account x"}}', { status: up }),
    });
    const r = await handleAsk(askRequest("good"), deps);
    assert.equal(r.status, status, String(up));
    const text = await r.text();
    assert.ok(text.includes(code) && !text.includes("sk-or"), text);
  }
  const down = askDeps({ upstream: async () => Promise.reject(new Error("dns")) });
  assert.equal((await handleAsk(askRequest("good"), down)).status, 502);
});

// ── the account handler ─────────────────────────────────────────────────────────────────────────

function accountDeps(over: Partial<AccountDeps> = {}, deleted: string[] = []): AccountDeps {
  return {
    verify: askDeps().verify,
    now: () => NOW,
    log: () => {},
    profile: async () => ({ pro: true, plan: null }),
    usageToday: async () => ({ requests: 4, tokens: 5000 }),
    deleteUser: async (id) => {
      deleted.push(id);
      return true;
    },
    limits: DEFAULT_LIMITS,
    ...over,
  };
}

test("GET /account shows Pro and today's use", async () => {
  const r = await handleAccount(new Request("https://x/account", { headers: { Authorization: "Bearer good" } }), accountDeps());
  assert.equal(r.status, 200);
  assert.deepEqual(await r.json(), {
    pro: true,
    plan: null,
    today: { requests: 4, tokens: 5000 },
    limits: { requests: DEFAULT_LIMITS.requests, tokens: DEFAULT_LIMITS.tokens },
    month_limits: { requests: DEFAULT_LIMITS.monthRequests, tokens: DEFAULT_LIMITS.monthTokens },
  });
});

test("DELETE /account deletes the caller, and only a signed-in caller", async () => {
  const deleted: string[] = [];
  const del = (token: string | null) =>
    new Request("https://x/account", { method: "DELETE", headers: token ? { Authorization: `Bearer ${token}` } : {} });
  assert.equal((await handleAccount(del(null), accountDeps({}, deleted))).status, 401);
  assert.equal((await handleAccount(del("forged"), accountDeps({}, deleted))).status, 401);
  assert.deepEqual(deleted, []);
  assert.equal((await handleAccount(del("good"), accountDeps({}, deleted))).status, 204);
  assert.deepEqual(deleted, [USER]);
  const failing = accountDeps({ deleteUser: async () => false });
  assert.equal((await handleAccount(del("good"), failing)).status, 502);
  const post = new Request("https://x/account", { method: "POST", headers: { Authorization: "Bearer good" } });
  assert.equal((await handleAccount(post, accountDeps())).status, 405);
});
