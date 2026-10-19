// The caller: a Supabase access token (JWT) in `Authorization: Bearer …`. The signature is checked
// by the injected verifier (supabase-js `auth.getClaims`, against the project's signing keys);
// these checks then make sure it is a signed-in user's token that hasn't expired.

export interface Claims {
  sub?: unknown;
  aud?: unknown;
  exp?: unknown;
  role?: unknown;
}

/** The token in an `Authorization` header, if it is a bearer token. */
export function bearer(header: string | null): string | null {
  if (!header) return null;
  const m = /^Bearer\s+([A-Za-z0-9._~+/=-]+)$/.exec(header.trim());
  return m ? m[1] : null;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The user id when the verified claims belong to a signed-in user and haven't expired. */
export function userFromClaims(claims: Claims | null, nowSeconds: number): string | null {
  if (!claims) return null;
  const aud = claims.aud;
  const audOk = aud === "authenticated" || (Array.isArray(aud) && aud.includes("authenticated"));
  if (!audOk || claims.role !== "authenticated") return null;
  if (typeof claims.exp !== "number" || claims.exp <= nowSeconds) return null;
  if (typeof claims.sub !== "string" || !UUID.test(claims.sub)) return null;
  return claims.sub;
}
