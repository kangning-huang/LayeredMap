// In-app feedback (Supabase Edge Function `feedback`; docs/lm/feedback.md): a message from the
// app's Help › Send Feedback… sheet, checked here and posted as a GitHub Discussion on the public
// repository. Nobody signs in. Abuse is held back by daily limits per install, per network address
// and overall (counted under keyed hashes, never the values), a honeypot field, length and link
// bounds. The text is quoted with mentions, issue references, images, links and HTML neutralised,
// so a message can't ping anyone or embed anything. A reply address, when given, is never posted:
// it goes to the private `feedback_contacts` table.
// Every outside service is injected (Deno, supabase-js and GitHub in feedback/index.ts; fakes in
// the Node tests).

import { errorResponse } from "./handler.ts";
import { secondsUntilReset, utcDay } from "./limits.ts";

export type Kind = "idea" | "problem" | "other";

export const KINDS: readonly Kind[] = ["idea", "problem", "other"];

export interface FeedbackOptions {
  /** The repository's GraphQL node id. */
  repositoryId: string;
  /** The Discussions category each kind goes to (GraphQL node ids). */
  categories: Record<Kind, string>;
  /** The repository's Discussions page (what a honeypot hit is told it posted to). */
  discussionsUrl: string;
  /** Messages per install per UTC day. */
  deviceDaily: number;
  /** Messages per network address per UTC day (a school or office shares one). */
  ipDaily: number;
  /** Messages from everyone per UTC day: a ceiling on what a spammer with many addresses can do. */
  globalDaily: number;
  /** Bounds on the message, in characters (code points), after cleaning. */
  minMessage: number;
  maxMessage: number;
  /** Bound on a title the user typed, in characters. */
  maxTitle: number;
  /** Links allowed in the title and message together. */
  maxLinks: number;
  /** Bound on the request body, in bytes. */
  maxBytes: number;
}

// The defaults: kangning-huang/NimbleMap and its Ideas, Q&A and General categories (owner,
// 2026-10-20). Problems go to Q&A, where an answer can be marked, titled "[Problem] …".
export const DEFAULT_FEEDBACK: FeedbackOptions = {
  repositoryId: "R_kgDOVO_7YA",
  categories: {
    idea: "DIC_kwDOVO_7YM4DJIF5",
    problem: "DIC_kwDOVO_7YM4DJIF4",
    other: "DIC_kwDOVO_7YM4DJIF3",
  },
  discussionsUrl: "https://github.com/kangning-huang/NimbleMap/discussions",
  deviceDaily: 5,
  ipDaily: 20,
  globalDaily: 200,
  minMessage: 5,
  maxMessage: 4000,
  maxTitle: 120,
  maxLinks: 3,
  maxBytes: 32 * 1024,
};

/** Reads `FEEDBACK_REPOSITORY_ID`, `FEEDBACK_CATEGORY_IDEA`, `FEEDBACK_CATEGORY_PROBLEM`,
 * `FEEDBACK_CATEGORY_OTHER`, `FEEDBACK_DISCUSSIONS_URL`, `FEEDBACK_DEVICE_DAILY`,
 * `FEEDBACK_IP_DAILY` and `FEEDBACK_GLOBAL_DAILY`; a missing or unreadable value keeps the default. */
export function feedbackOptionsFromEnv(get: (name: string) => string | undefined): FeedbackOptions {
  const d = DEFAULT_FEEDBACK;
  const id = (name: string, fallback: string) => {
    const v = (get(name) ?? "").trim();
    return /^[A-Za-z0-9_-]{4,100}$/.test(v) ? v : fallback;
  };
  const url = (get("FEEDBACK_DISCUSSIONS_URL") ?? "").trim();
  return {
    ...d,
    repositoryId: id("FEEDBACK_REPOSITORY_ID", d.repositoryId),
    categories: {
      idea: id("FEEDBACK_CATEGORY_IDEA", d.categories.idea),
      problem: id("FEEDBACK_CATEGORY_PROBLEM", d.categories.problem),
      other: id("FEEDBACK_CATEGORY_OTHER", d.categories.other),
    },
    discussionsUrl: url.startsWith("https://github.com/") ? url : d.discussionsUrl,
    deviceDaily: positiveInt(get("FEEDBACK_DEVICE_DAILY")) ?? d.deviceDaily,
    ipDaily: positiveInt(get("FEEDBACK_IP_DAILY")) ?? d.ipDaily,
    globalDaily: positiveInt(get("FEEDBACK_GLOBAL_DAILY")) ?? d.globalDaily,
  };
}

function positiveInt(v: string | undefined): number | undefined {
  if (v === undefined || !/^\s*\d+\s*$/.test(v)) return undefined;
  const n = Number(v.trim());
  return Number.isSafeInteger(n) && n > 0 ? n : undefined;
}

// ── reading the request ─────────────────────────────────────────────────────────────────────────

/** A checked message, cleaned. */
export interface Feedback {
  kind: Kind;
  message: string;
  /** What the user typed, cleaned; empty when they left it out. */
  title: string;
  installId: string;
  /** Kept only when the user ticked "Include app version and macOS version". */
  appVersion: string | null;
  osVersion: string | null;
  /** The interface language (a code such as `de`): never personal, always kept when valid. */
  language: string | null;
  contactEmail: string | null;
}

export type Read =
  | { ok: true; feedback: Feedback }
  | { ok: true; honeypot: true }
  | { ok: false; status: number; code: string; message: string };

function refuse(status: number, code: string, message: string): Read {
  return { ok: false, status, code, message };
}

// Control characters except line feed and tab; carriage returns are turned into line feeds first.
const CONTROL = /[\u0000-\u0008\u000B-\u001F\u007F-\u009F]/g;
// Bidirectional overrides and isolates (text that displays in another order than it reads).
const BIDI = /[‪-‮⁦-⁩]/g;

/** The text with line breaks normalised, control and bidi-override characters removed, trailing
 * spaces dropped, at most two blank lines in a row, trimmed. */
export function cleanText(s: string): string {
  return s
    .replace(/\r\n?/g, "\n")
    .replace(CONTROL, "")
    .replace(BIDI, "")
    .split("\n")
    .map((l) => l.trimEnd())
    .join("\n")
    .replace(/\n{4,}/g, "\n\n\n")
    .trim();
}

/** One line: whitespace runs (line breaks included) become one space. */
export function oneLine(s: string): string {
  return cleanText(s).replace(/\s+/g, " ").trim();
}

/** Length in characters (code points), as the app counts. */
export function chars(s: string): number {
  return [...s].length;
}

/** Links in the text: `http://`, `https://` and `www.` addresses. */
export function countLinks(s: string): number {
  return (s.match(/\bhttps?:\/\/|\bwww\./gi) ?? []).length;
}

const INSTALL_ID = /^[A-Za-z0-9-]{8,64}$/;
// What the app sends: "0.7.3", "macOS 14.5", "Windows 11 (26100)". Anything else is dropped.
const VERSION = /^[A-Za-z0-9][A-Za-z0-9 .+()_-]{0,47}$/;
const LANGUAGE = /^[A-Za-z]{2,3}(?:[-_][A-Za-z0-9]{2,8}){0,2}$/;
const EMAIL = /^[^\s@<>()[\]\\,;:"]+@[^\s@<>()[\]\\,;:"]+\.[^\s@<>()[\]\\,;:"]{2,}$/;

/** Checks and cleans a request body (the raw text, so its size is checked first). */
export function readFeedback(raw: string, opts: FeedbackOptions): Read {
  if (new TextEncoder().encode(raw).length > opts.maxBytes) {
    return refuse(413, "too_long", "The message is too long.");
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return refuse(400, "invalid", "The request isn't JSON.");
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    return refuse(400, "invalid", "The request isn't a JSON object.");
  }
  const r = parsed as Record<string, unknown>;
  // A field people never see: only a form-filling robot writes in it.
  if (typeof r.website === "string" && r.website.trim() !== "") return { ok: true, honeypot: true };
  const kind = r.kind;
  if (typeof kind !== "string" || !(KINDS as readonly string[]).includes(kind)) {
    return refuse(400, "invalid", "kind is idea, problem or other.");
  }
  if (typeof r.install_id !== "string" || !INSTALL_ID.test(r.install_id)) {
    return refuse(400, "invalid", "install_id is missing.");
  }
  if (typeof r.message !== "string") return refuse(400, "invalid", "message is missing.");
  const message = cleanText(r.message);
  if (chars(message) < opts.minMessage) return refuse(400, "invalid", "The message is empty or too short.");
  if (chars(message) > opts.maxMessage) return refuse(413, "too_long", `The message is longer than ${opts.maxMessage} characters.`);
  if (r.title !== undefined && r.title !== null && typeof r.title !== "string") return refuse(400, "invalid", "title isn't text.");
  const title = typeof r.title === "string" ? oneLine(r.title) : "";
  if (chars(title) > opts.maxTitle) return refuse(413, "too_long", `The title is longer than ${opts.maxTitle} characters.`);
  if (countLinks(`${title}\n${message}`) > opts.maxLinks) {
    return refuse(400, "invalid", `At most ${opts.maxLinks} links, please.`);
  }
  if (r.include_diagnostics !== undefined && typeof r.include_diagnostics !== "boolean") {
    return refuse(400, "invalid", "include_diagnostics isn't true or false.");
  }
  const diagnostics = r.include_diagnostics === true;
  const version = (v: unknown) => (typeof v === "string" && VERSION.test(v.trim()) ? v.trim() : null);
  let contactEmail: string | null = null;
  if (r.contact_email !== undefined && r.contact_email !== null) {
    if (typeof r.contact_email !== "string") return refuse(400, "invalid", "contact_email isn't text.");
    const e = r.contact_email.trim();
    if (e !== "") {
      if (e.length > 254 || !EMAIL.test(e)) return refuse(400, "invalid", "contact_email isn't an email address.");
      contactEmail = e;
    }
  }
  return {
    ok: true,
    feedback: {
      kind: kind as Kind,
      message,
      title,
      installId: r.install_id,
      appVersion: diagnostics ? version(r.app_version) : null,
      osVersion: diagnostics ? version(r.os_version) : null,
      language: typeof r.language === "string" && LANGUAGE.test(r.language.trim()) ? r.language.trim() : null,
      contactEmail,
    },
  };
}

// ── what is posted ──────────────────────────────────────────────────────────────────────────────

const ZWSP = "​";

/** Breaks what GitHub would turn into a notification or a cross-reference: `@name` and `#123`
 * (also `owner/repo#123`) get a zero-width space after the sign. */
export function defuse(s: string): string {
  return s.replace(/@/g, `@${ZWSP}`).replace(/#(?=\d)/g, `#${ZWSP}`);
}

/** One line of user text made literal: markdown that could link, embed an image, add HTML or an
 * entity, start a heading or a table, or format text is backslash-escaped, then defused. */
export function escapeLine(s: string): string {
  return defuse(s.replace(/[\\`*_[\]<>!#|~&]/g, "\\$&"));
}

/** The text as a quote block, every line literal. */
export function quoted(text: string): string {
  return text
    .split("\n")
    .map((l) => (l === "" ? ">" : `> ${escapeLine(l)}`))
    .join("\n");
}

/** The discussion's title: the user's, else the message's first line, shortened at a word to
 * about 80 characters; problems start with "[Problem]". Titles are plain text on GitHub, but
 * mentions are defused anyway. */
export function discussionTitle(f: Feedback): string {
  let t = f.title;
  if (t === "") {
    const first = oneLine(f.message.split("\n").find((l) => l.trim() !== "") ?? "");
    t = shorten(first, 80);
  }
  if (f.kind === "problem") {
    t = t.replace(/^\s*\[problem\]\s*/i, "");
    t = `[Problem] ${t}`;
  }
  return defuse(t);
}

function shorten(s: string, max: number): string {
  const cp = [...s];
  if (cp.length <= max) return s;
  const cut = cp.slice(0, max - 1).join("");
  const space = cut.lastIndexOf(" ");
  return `${(space > max / 2 ? cut.slice(0, space) : cut).trimEnd()}…`;
}

/** The discussion's body: the message quoted, the app details the user agreed to send, and where
 * it came from. Never the reply address or the install id. */
export function discussionBody(f: Feedback): string {
  const details = [f.appVersion && `App: ${f.appVersion}`, f.osVersion, f.language].filter((x): x is string => !!x);
  const parts = [quoted(f.message), "---"];
  if (details.length > 0) parts.push(escapeLine(details.join(" · ")));
  parts.push("Sent from the NimbleMap app");
  return parts.join("\n\n");
}

export const CREATE_DISCUSSION = `mutation($input: CreateDiscussionInput!) {
  createDiscussion(input: $input) { discussion { url } }
}`;

/** The created discussion's address from a GraphQL answer; null on errors or anything unexpected. */
export function discussionUrlFrom(answer: unknown): string | null {
  if (answer === null || typeof answer !== "object") return null;
  const a = answer as Record<string, unknown>;
  if (Array.isArray(a.errors) && a.errors.length > 0) return null;
  const url = (a.data as { createDiscussion?: { discussion?: { url?: unknown } } } | undefined)?.createDiscussion?.discussion?.url;
  return typeof url === "string" && url.startsWith("https://github.com/") ? url : null;
}

// ── hashing ─────────────────────────────────────────────────────────────────────────────────────

/** HMAC-SHA-256 of `value` under `key`, as hex. A keyed hash: install ids and IPv4 addresses are
 * few enough to try them all against a plain one. */
export async function hmacHex(key: string, value: string): Promise<string> {
  const enc = new TextEncoder();
  const k = await crypto.subtle.importKey("raw", enc.encode(key), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const sig = new Uint8Array(await crypto.subtle.sign("HMAC", k, enc.encode(value)));
  return Array.from(sig, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** The caller's address as the platform's proxy reports it (the first `X-Forwarded-For` entry). */
export function clientIp(req: Request): string {
  const fwd = req.headers.get("x-forwarded-for");
  const first = fwd?.split(",")[0]?.trim();
  if (first) return first;
  return req.headers.get("x-real-ip")?.trim() || req.headers.get("cf-connecting-ip")?.trim() || "unknown";
}

// ── the handler ─────────────────────────────────────────────────────────────────────────────────

/** What `feedback_reserve` answered. */
export interface FeedbackReservation {
  allowed: boolean;
  blocked: "device" | "ip" | "all" | null;
}

/** The row `feedback_reserve` returns (an array of one through PostgREST). */
export function feedbackReservationFromRpc(data: unknown): FeedbackReservation | null {
  const row = Array.isArray(data) ? data[0] : data;
  if (row === null || typeof row !== "object") return null;
  const r = row as Record<string, unknown>;
  if (typeof r.allowed !== "boolean") return null;
  const blocked = r.blocked === "device" || r.blocked === "ip" || r.blocked === "all" ? r.blocked : null;
  return { allowed: r.allowed, blocked };
}

export interface FeedbackDeps {
  now(): Date;
  /** One line per request: the kind and the outcome. Never the text, the address or a hash. */
  log(line: string): void;
  /** The keyed hash of an install id or an address. */
  hash(value: string): Promise<string>;
  /** Counts one message for the install, the address and everyone today, unless one is used up. */
  reserve(device: string, ip: string, day: string, opts: FeedbackOptions): Promise<FeedbackReservation | null>;
  /** Sends a GraphQL request to GitHub with the token. */
  github(query: string, variables: Record<string, unknown>): Promise<Response>;
  /** Keeps a reply address beside the discussion it belongs to (private table). */
  saveContact(discussionUrl: string, email: string): Promise<boolean>;
  options: FeedbackOptions;
}

/** `POST /feedback`: checks the message, counts it, posts it, answers `{ url }`. */
export async function handleFeedback(req: Request, deps: FeedbackDeps): Promise<Response> {
  if (req.method !== "POST") return errorResponse(405, "method", "Use POST.");
  const opts = deps.options;
  const read = readFeedback(await req.text(), opts);
  if (!read.ok) {
    deps.log(`feedback status=${read.status} ${read.code}`);
    return errorResponse(read.status, read.code, read.message);
  }
  if ("honeypot" in read) {
    // Told it worked, so a robot has nothing to learn from; nothing is posted or counted.
    deps.log("feedback status=200 honeypot");
    return json({ url: opts.discussionsUrl });
  }
  const f = read.feedback;
  const now = deps.now();
  let device: string;
  let ip: string;
  try {
    device = `device:${await deps.hash(`device:${f.installId}`)}`;
    ip = `ip:${await deps.hash(`ip:${clientIp(req)}`)}`;
  } catch {
    return errorResponse(503, "upstream", "Feedback can't be sent right now. Try again later.");
  }
  const r = await deps.reserve(device, ip, utcDay(now), opts).catch(() => null);
  if (!r) return errorResponse(503, "upstream", "Feedback can't be sent right now. Try again later.");
  if (!r.allowed) {
    deps.log(`feedback kind=${f.kind} status=429 ${r.blocked ?? "limit"}`);
    return errorResponse(429, "rate_limited", "Too much feedback from here today. Try again tomorrow.", {
      resets_in_seconds: secondsUntilReset(now),
    });
  }
  const input = {
    repositoryId: opts.repositoryId,
    categoryId: opts.categories[f.kind],
    title: discussionTitle(f),
    body: discussionBody(f),
  };
  let url: string | null = null;
  let status = 0;
  try {
    const up = await deps.github(CREATE_DISCUSSION, { input });
    status = up.status;
    const answer = await up.json().catch(() => null);
    url = up.ok ? discussionUrlFrom(answer) : null;
  } catch {
    url = null;
  }
  if (!url) {
    // GitHub's own message may name the token or the repository settings: it stays in the log.
    deps.log(`feedback kind=${f.kind} status=502 upstream github=${status}`);
    return errorResponse(502, "upstream", "GitHub didn't take the message. Try again later.");
  }
  let contact = "none";
  if (f.contactEmail) {
    contact = (await deps.saveContact(url, f.contactEmail).catch(() => false)) ? "kept" : "lost";
  }
  deps.log(`feedback kind=${f.kind} status=200 contact=${contact}`);
  return json({ url });
}

function json(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json", "Cache-Control": "no-store" },
  });
}
