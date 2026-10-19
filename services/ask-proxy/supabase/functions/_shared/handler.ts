// The two functions' request handling, with every outside service injected (Deno and supabase-js
// in index.ts; fakes in the Node tests). Errors are JSON `{"error": {"code", "message"}}`, which
// the app turns into its own words (crates/lm-ask/src/proxy.rs).

import { bearer, type Claims, userFromClaims } from "./auth.ts";
import { type Limits, type Reservation, secondsUntilReset, utcDay } from "./limits.ts";
import { type ShapeOptions, shapeRequest } from "./shape.ts";
import { type TokenUsage, usageTap } from "./usage.ts";

export interface Deps {
  /** Checks the token's signature; the claims, or null when it isn't valid. */
  verify(token: string): Promise<Claims | null>;
  now(): Date;
  /** One line per request: user id, status, tokens. Never the body. */
  log(line: string): void;
}

export interface AskDeps extends Deps {
  /** `profiles.pro` for the user; null when there is no profile. */
  isPro(userId: string): Promise<boolean | null>;
  reserve(userId: string, day: string, limits: Limits): Promise<Reservation | null>;
  record(userId: string, day: string, usage: TokenUsage): Promise<void>;
  /** Sends the shaped body to the model service (OpenRouter, with the key). */
  upstream(body: Record<string, unknown>): Promise<Response>;
  limits: Limits;
  shape: ShapeOptions;
}

export interface AccountDeps extends Deps {
  profile(userId: string): Promise<{ pro: boolean; plan: string | null } | null>;
  usageToday(userId: string, day: string): Promise<{ requests: number; tokens: number }>;
  deleteUser(userId: string): Promise<boolean>;
  limits: Limits;
}

export function errorResponse(status: number, code: string, message: string, extra: Record<string, unknown> = {}): Response {
  return new Response(JSON.stringify({ error: { code, message, ...extra } }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

async function caller(req: Request, deps: Deps): Promise<string | null> {
  const token = bearer(req.headers.get("Authorization"));
  if (!token) return null;
  let claims: Claims | null = null;
  try {
    claims = await deps.verify(token);
  } catch {
    claims = null;
  }
  return userFromClaims(claims, Math.floor(deps.now().getTime() / 1000));
}

/** `POST /ask`: one chat-completions call for a signed-in Pro user within today's allowance. */
export async function handleAsk(req: Request, deps: AskDeps): Promise<Response> {
  if (req.method !== "POST") return errorResponse(405, "method", "Use POST.");
  const user = await caller(req, deps);
  if (!user) return errorResponse(401, "unauthorized", "Sign in again.");
  const raw = await req.text();
  const shaped = shapeRequest(raw, deps.shape);
  if (!shaped.ok) return errorResponse(shaped.status, shaped.code, shaped.message);
  const pro = await deps.isPro(user).catch(() => null);
  if (pro === null) return errorResponse(403, "no_profile", "This account has no profile yet.");
  if (!pro) return errorResponse(403, "not_pro", "Pro isn't on for this account.");
  const now = deps.now();
  const day = utcDay(now);
  const r = await deps.reserve(user, day, deps.limits).catch(() => null);
  if (!r) return errorResponse(503, "unavailable", "The allowance couldn't be checked. Try again shortly.");
  if (!r.allowed) {
    deps.log(`ask user=${user} status=429 requests=${r.requests} tokens=${r.tokens}`);
    return errorResponse(429, "daily_limit", "Today's allowance is used up.", {
      resets_in_seconds: secondsUntilReset(now),
      limit_requests: deps.limits.requests,
      limit_tokens: deps.limits.tokens,
    });
  }
  let up: Response;
  try {
    up = await deps.upstream(shaped.body);
  } catch {
    return errorResponse(502, "upstream", "The model service couldn't be reached.");
  }
  if (!up.ok || !up.body) {
    // The model service's own message may name the account or key: don't pass it on.
    deps.log(`ask user=${user} status=${up.status} upstream`);
    await up.body?.cancel().catch(() => {});
    if (up.status === 402) return errorResponse(503, "upstream_credit", "The assistant service is out of credit.");
    if (up.status === 429) return errorResponse(503, "upstream_busy", "The model service is busy. Try again shortly.");
    if (up.status === 400) return errorResponse(502, "upstream_rejected", "The model service refused the request.");
    return errorResponse(502, "upstream", `The model service answered ${up.status}.`);
  }
  const tap = usageTap(shaped.stream, async (u) => {
    const used = u ?? { prompt: 0, completion: 0 };
    deps.log(`ask user=${user} status=200 prompt=${used.prompt} completion=${used.completion}`);
    if (u) await deps.record(user, day, u);
  });
  return new Response(up.body.pipeThrough(tap), {
    status: 200,
    headers: {
      "Content-Type": shaped.stream ? "text/event-stream" : "application/json",
      "Cache-Control": "no-store",
    },
  });
}

/** `GET /account`: Pro and today's use. `DELETE /account`: deletes the user and their rows. */
export async function handleAccount(req: Request, deps: AccountDeps): Promise<Response> {
  if (req.method !== "GET" && req.method !== "DELETE") return errorResponse(405, "method", "Use GET or DELETE.");
  const user = await caller(req, deps);
  if (!user) return errorResponse(401, "unauthorized", "Sign in again.");
  if (req.method === "DELETE") {
    const ok = await deps.deleteUser(user).catch(() => false);
    deps.log(`account user=${user} delete=${ok}`);
    if (!ok) return errorResponse(502, "delete_failed", "The account couldn't be deleted. Try again shortly.");
    return new Response(null, { status: 204 });
  }
  const p = await deps.profile(user).catch(() => null);
  if (!p) return errorResponse(404, "no_profile", "This account has no profile yet.");
  const day = utcDay(deps.now());
  const today = await deps.usageToday(user, day).catch(() => ({ requests: 0, tokens: 0 }));
  return new Response(
    JSON.stringify({ pro: p.pro, plan: p.plan, today, limits: { requests: deps.limits.requests, tokens: deps.limits.tokens } }),
    { status: 200, headers: { "Content-Type": "application/json", "Cache-Control": "no-store" } },
  );
}
