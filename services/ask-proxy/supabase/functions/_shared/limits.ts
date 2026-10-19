// The per-user allowances, daily and monthly: limits from the function's secrets, and the UTC day
// and month they count in.

export interface Limits {
  /** Model calls per user per UTC day (one question is usually 2–4 calls): a burst guard. */
  requests: number;
  /** Prompt plus completion tokens per user per UTC day. */
  tokens: number;
  /** Model calls per user per UTC month: about 300 questions (owner, 2026-10-19). */
  monthRequests: number;
  /** Prompt plus completion tokens per user per UTC month. */
  monthTokens: number;
}

export const DEFAULT_LIMITS: Limits = { requests: 60, tokens: 400_000, monthRequests: 1_000, monthTokens: 3_000_000 };

/** Reads `ASK_DAILY_REQUESTS`, `ASK_DAILY_TOKENS`, `ASK_MONTHLY_REQUESTS` and `ASK_MONTHLY_TOKENS`;
 * a missing or unreadable value keeps the default. */
export function limitsFromEnv(get: (name: string) => string | undefined): Limits {
  return {
    requests: positiveInt(get("ASK_DAILY_REQUESTS")) ?? DEFAULT_LIMITS.requests,
    tokens: positiveInt(get("ASK_DAILY_TOKENS")) ?? DEFAULT_LIMITS.tokens,
    monthRequests: positiveInt(get("ASK_MONTHLY_REQUESTS")) ?? DEFAULT_LIMITS.monthRequests,
    monthTokens: positiveInt(get("ASK_MONTHLY_TOKENS")) ?? DEFAULT_LIMITS.monthTokens,
  };
}

function positiveInt(v: string | undefined): number | undefined {
  if (v === undefined || !/^\s*\d+\s*$/.test(v)) return undefined;
  const n = Number(v.trim());
  return Number.isSafeInteger(n) && n > 0 ? n : undefined;
}

/** The UTC day as `YYYY-MM-DD`, the key of `usage_daily`. */
export function utcDay(now: Date): string {
  return now.toISOString().slice(0, 10);
}

/** Seconds until the allowance resets (next UTC midnight). */
export function secondsUntilReset(now: Date): number {
  const next = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1);
  return Math.max(1, Math.ceil((next - now.getTime()) / 1000));
}

/** Seconds until the monthly allowance resets (the first of next month, UTC). */
export function secondsUntilMonthReset(now: Date): number {
  const next = Date.UTC(now.getUTCFullYear(), now.getUTCMonth() + 1, 1);
  return Math.max(1, Math.ceil((next - now.getTime()) / 1000));
}

/** What `reserve_request` answered. `blocked` says which allowance stopped it. */
export interface Reservation {
  allowed: boolean;
  requests: number;
  tokens: number;
  monthRequests?: number;
  monthTokens?: number;
  blocked?: "day" | "month" | null;
}

/** The row `reserve_request` returns (an array of one through PostgREST), as a `Reservation`. */
export function reservationFromRpc(data: unknown): Reservation | null {
  const row = Array.isArray(data) ? data[0] : data;
  if (row === null || typeof row !== "object") return null;
  const r = row as Record<string, unknown>;
  if (typeof r.allowed !== "boolean") return null;
  const blocked = r.blocked === "day" || r.blocked === "month" ? r.blocked : null;
  return {
    allowed: r.allowed,
    requests: Number(r.requests ?? 0),
    tokens: Number(r.tokens ?? 0),
    monthRequests: Number(r.month_requests ?? 0),
    monthTokens: Number(r.month_tokens ?? 0),
    blocked,
  };
}
