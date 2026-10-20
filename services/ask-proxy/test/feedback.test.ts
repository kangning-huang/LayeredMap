// The feedback function's logic under Node's built-in test runner, with GitHub and the database
// faked: `npm test` here.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  appLabel,
  cleanText,
  clientIp,
  countLinks,
  CREATE_DISCUSSION,
  DEFAULT_FEEDBACK,
  discussionBody,
  discussionTitle,
  discussionUrlFrom,
  escapeLine,
  type Feedback,
  type FeedbackDeps,
  feedbackOptionsFromEnv,
  type FeedbackReservation,
  feedbackReservationFromRpc,
  handleFeedback,
  hmacHex,
  quoted,
  readFeedback,
  SIGNED_IN_LINE,
  systemLabel,
} from "../supabase/functions/_shared/feedback.ts";

const NOW = new Date("2026-10-20T22:00:00Z");
const INSTALL = "3b241101-e2bb-4255-8caf-4136c566a962";
const DISCUSSION = "https://github.com/kangning-huang/NimbleMap/discussions/42";

/** What the app sends (crates/lm-account/src/feedback.rs). */
function appBody(extra: Record<string, unknown> = {}) {
  return {
    kind: "idea",
    message: "Please add a dark basemap with labels.\nIt would help at night.",
    title: "",
    app_version: "0.7.3",
    os_version: "macOS 14.5",
    language: "de",
    include_diagnostics: true,
    install_id: INSTALL,
    ...extra,
  };
}

function read(body: unknown) {
  return readFeedback(typeof body === "string" ? body : JSON.stringify(body), DEFAULT_FEEDBACK);
}

function feedback(body: unknown): Feedback {
  const r = read(body);
  assert.ok(r.ok && "feedback" in r, JSON.stringify(r));
  return r.feedback;
}

// ── options ─────────────────────────────────────────────────────────────────────────────────────

test("options come from the function's env, with the repository's ids as defaults", () => {
  assert.deepEqual(feedbackOptionsFromEnv(() => undefined), DEFAULT_FEEDBACK);
  assert.deepEqual(DEFAULT_FEEDBACK.categories, {
    idea: "DIC_kwDOVO_7YM4DJIF5",
    problem: "DIC_kwDOVO_7YM4DJIF4",
    other: "DIC_kwDOVO_7YM4DJIF3",
  });
  assert.equal(DEFAULT_FEEDBACK.repositoryId, "R_kgDOVO_7YA");
  const env: Record<string, string> = {
    FEEDBACK_CATEGORY_PROBLEM: " DIC_kwDOVO_7YM4DJIF3 ",
    FEEDBACK_REPOSITORY_ID: "not an id!",
    FEEDBACK_DEVICE_DAILY: "3",
    FEEDBACK_IP_DAILY: "-1",
    FEEDBACK_DISCUSSIONS_URL: "https://evil.example/",
  };
  const o = feedbackOptionsFromEnv((k) => env[k]);
  assert.equal(o.categories.problem, "DIC_kwDOVO_7YM4DJIF3");
  assert.equal(o.repositoryId, DEFAULT_FEEDBACK.repositoryId);
  assert.equal(o.deviceDaily, 3);
  assert.equal(o.ipDaily, DEFAULT_FEEDBACK.ipDaily);
  assert.equal(o.discussionsUrl, DEFAULT_FEEDBACK.discussionsUrl);
});

// ── reading ─────────────────────────────────────────────────────────────────────────────────────

test("the app's body reads as feedback", () => {
  const f = feedback(appBody());
  assert.equal(f.kind, "idea");
  assert.equal(f.message, "Please add a dark basemap with labels.\nIt would help at night.");
  assert.equal(f.title, "");
  assert.deepEqual([f.appVersion, f.osVersion, f.language, f.contactEmail], ["0.7.3", "macOS 14.5", "de", null]);
});

test("without diagnostics the versions are dropped, the language kept", () => {
  const f = feedback(appBody({ include_diagnostics: false }));
  assert.deepEqual([f.appVersion, f.osVersion, f.language], [null, null, "de"]);
  // Missing means no.
  const { include_diagnostics: _, ...rest } = appBody();
  assert.equal(feedback(rest).appVersion, null);
});

test("odd versions and languages are dropped, not refused", () => {
  const f = feedback(appBody({ app_version: "<img src=x>", os_version: "x".repeat(60), language: "de; DROP" }));
  assert.deepEqual([f.appVersion, f.osVersion, f.language], [null, null, null]);
  assert.equal(feedback(appBody({ language: "pt-BR" })).language, "pt-BR");
  assert.equal(feedback(appBody({ os_version: "Windows 11 (26100)" })).osVersion, "Windows 11 (26100)");
});

test("control characters and bidi overrides are stripped, line breaks kept", () => {
  assert.equal(cleanText("a\u0000b\u0007c\r\nd\re‮f⁦g\u0085h\t!  \n\n\n\n\nz "), "abc\nd\nefgh\t!\n\n\nz");
  const f = feedback(appBody({ message: "\u0000\u0000hello\u001b[31m there", title: " Two\nlines\t here " }));
  assert.equal(f.message, "hello[31m there");
  assert.equal(f.title, "Two lines here");
});

test("bad requests are refused with a reason", () => {
  const cases: [unknown, number, string][] = [
    ["not json", 400, "invalid"],
    ["[1]", 400, "invalid"],
    [appBody({ kind: "rant" }), 400, "invalid"],
    [appBody({ kind: undefined }), 400, "invalid"],
    [appBody({ install_id: undefined }), 400, "invalid"],
    [appBody({ install_id: "short" }), 400, "invalid"],
    [appBody({ install_id: "../../etc/passwd/xx" }), 400, "invalid"],
    [appBody({ message: 3 }), 400, "invalid"],
    [appBody({ message: " \n\u0000 hi \n" }), 400, "invalid"],
    [appBody({ title: ["x"] }), 400, "invalid"],
    [appBody({ include_diagnostics: "yes" }), 400, "invalid"],
    [appBody({ contact_email: "not an address" }), 400, "invalid"],
    [appBody({ contact_email: "a@b" }), 400, "invalid"],
    [appBody({ contact_email: `${"a".repeat(250)}@b.com` }), 400, "invalid"],
    [appBody({ contact_email: 5 }), 400, "invalid"],
    [appBody({ message: "x".repeat(4001) }), 413, "too_long"],
    [appBody({ title: "t".repeat(121) }), 413, "too_long"],
    ["x".repeat(DEFAULT_FEEDBACK.maxBytes + 1), 413, "too_long"],
  ];
  for (const [body, status, code] of cases) {
    const r = read(body);
    const label = JSON.stringify(body).slice(0, 60);
    assert.ok(!r.ok, label);
    assert.equal(r.status, status, label);
    assert.equal(r.code, code, label);
  }
});

test("the length bound counts characters, not bytes", () => {
  // 4,000 Chinese characters are 12,000 bytes: still within bounds.
  assert.equal(feedback(appBody({ message: "图".repeat(4000) })).message.length, 4000);
  assert.ok(!read(appBody({ message: "图".repeat(4001) })).ok);
  // Emoji are one character each.
  assert.ok(read(appBody({ message: "🗺️".repeat(1000) })).ok);
});

test("more than three links is refused as spam", () => {
  assert.equal(countLinks("see https://a.example, http://b.example and www.c.example"), 3);
  assert.ok(read(appBody({ message: "https://a.example https://b.example https://c.example" })).ok);
  const r = read(appBody({ message: "https://a.example https://b.example www.c.example", title: "http://d.example" }));
  assert.ok(!r.ok && r.code === "invalid" && /links/.test(r.message));
});

test("an address, when given, is kept trimmed; an empty one is none", () => {
  assert.equal(feedback(appBody({ contact_email: "  kim@example.com " })).contactEmail, "kim@example.com");
  assert.equal(feedback(appBody({ contact_email: "  " })).contactEmail, null);
  assert.equal(feedback(appBody({ contact_email: null })).contactEmail, null);
});

test("the honeypot field marks a robot", () => {
  const r = read(appBody({ website: "http://spam.example" }));
  assert.ok(r.ok && "honeypot" in r);
  assert.ok(read(appBody({ website: "" })).ok && "feedback" in (read(appBody({ website: "" })) as object));
});

// ── what is posted ──────────────────────────────────────────────────────────────────────────────

test("mentions, references, images, links and HTML can't do anything", () => {
  const line = "@octocat see #12 and x/y#3 ![i](https://t.example/p.png) [l](u) <img src=x> &#64;me `c` **b** | ~s~";
  const e = escapeLine(line);
  // No mention: a zero-width space follows every @ and every # before a digit.
  assert.ok(!/@[A-Za-z0-9]/.test(e), e);
  assert.ok(!/#\d/.test(e), e);
  // Every character markdown could act on is escaped.
  assert.ok(!/(?<!\\)[!\[\]<>&`*|~]/.test(e), e);
  assert.ok(e.includes("\\!\\[i\\]") && e.includes("\\<img") && e.includes("\\&\\#"), e);
  // A heading or a nested quote at the start of a line stays text.
  assert.equal(escapeLine("# Big"), "\\# Big");
  assert.equal(escapeLine("> quote"), "\\> quote");
});

test("the message is posted as a quote block, blank lines included", () => {
  assert.equal(quoted("one\n\ntwo @me"), "> one\n>\n> two @​me");
});

test("the body: the quoted message, the app details, where it came from; never the address", () => {
  const f = feedback(appBody({ contact_email: "kim@example.com" }));
  assert.equal(
    discussionBody(f),
    [
      "> Please add a dark basemap with labels.",
      "> It would help at night.",
      "",
      "---",
      "",
      "NimbleMap 0.7.3 · macOS 14.5 · de",
      "",
      "Sent from the NimbleMap app",
    ].join("\n"),
  );
  assert.ok(!discussionBody(f).includes("kim@"));
  assert.ok(!discussionBody(f).includes(INSTALL));
  const bare = feedback(appBody({ include_diagnostics: false, language: undefined }));
  assert.ok(discussionBody(bare).endsWith("---\n\nSent from the NimbleMap app"), discussionBody(bare));
});

test("versions are labelled once: a bare number gets its system's name", () => {
  assert.equal(appLabel("0.7.1"), "NimbleMap 0.7.1");
  assert.equal(appLabel("NimbleMap 0.7.1"), "NimbleMap 0.7.1");
  assert.equal(systemLabel("26.6"), "macOS 26.6");
  assert.equal(systemLabel("14.5.1"), "macOS 14.5.1");
  assert.equal(systemLabel("10.0.26100"), "Windows 10.0.26100");
  for (const v of ["macOS 26.6", "Windows", "Windows 11 (26100)", "linux"]) assert.equal(systemLabel(v), v);
  // The request the release check sent: "App: 0.7.1 · 26.6" before, now named.
  const f = feedback(appBody({ app_version: "0.7.1", os_version: "26.6", language: undefined }));
  assert.ok(discussionBody(f).includes("\n\nNimbleMap 0.7.1 · macOS 26.6\n\n"), discussionBody(f));
  const g = feedback(appBody({ app_version: "0.7.1", os_version: "macOS 26.6", language: "en" }));
  assert.ok(discussionBody(g).includes("\n\nNimbleMap 0.7.1 · macOS 26.6 · en\n\n"), discussionBody(g));
  assert.ok(!discussionBody(g).includes("macOS macOS"));
});

test("the title: the user's, else the first line shortened; problems are marked", () => {
  assert.equal(discussionTitle(feedback(appBody())), "Please add a dark basemap with labels.");
  assert.equal(discussionTitle(feedback(appBody({ title: "Dark basemap" }))), "Dark basemap");
  assert.equal(discussionTitle(feedback(appBody({ kind: "problem", title: "Crash on open" }))), "[Problem] Crash on open");
  assert.equal(discussionTitle(feedback(appBody({ kind: "problem", title: "[problem] Crash" }))), "[Problem] Crash");
  const long = "word ".repeat(40).trim();
  const t = discussionTitle(feedback(appBody({ message: `\n\n${long}\nsecond line` })));
  assert.ok(t.endsWith("…") && [...t].length <= 80 && !t.includes("second"), t);
  assert.equal(discussionTitle(feedback(appBody({ title: "Hi @octocat" }))), "Hi @​octocat");
});

test("the discussion's address is read from GitHub's answer", () => {
  assert.equal(discussionUrlFrom({ data: { createDiscussion: { discussion: { url: DISCUSSION } } } }), DISCUSSION);
  assert.equal(discussionUrlFrom({ data: { createDiscussion: { discussion: { url: "https://evil.example/" } } } }), null);
  assert.equal(discussionUrlFrom({ data: null, errors: [{ type: "FORBIDDEN", message: "Resource not accessible" }] }), null);
  assert.equal(discussionUrlFrom(null), null);
});

// ── hashing ─────────────────────────────────────────────────────────────────────────────────────

test("hashes are keyed and stable; the address is the first forwarded one", async () => {
  const a = await hmacHex("k1", "ip:203.0.113.7");
  assert.match(a, /^[0-9a-f]{64}$/);
  assert.equal(a, await hmacHex("k1", "ip:203.0.113.7"));
  assert.notEqual(a, await hmacHex("k2", "ip:203.0.113.7"));
  const req = (h: Record<string, string>) => new Request("https://x.example/", { headers: h });
  assert.equal(clientIp(req({ "x-forwarded-for": " 203.0.113.7 , 10.0.0.1" })), "203.0.113.7");
  assert.equal(clientIp(req({ "x-real-ip": "198.51.100.2" })), "198.51.100.2");
  assert.equal(clientIp(req({})), "unknown");
});

test("feedback_reserve's row reads as a reservation", () => {
  assert.deepEqual(feedbackReservationFromRpc([{ allowed: true, blocked: null }]), { allowed: true, blocked: null });
  assert.deepEqual(feedbackReservationFromRpc({ allowed: false, blocked: "ip" }), { allowed: false, blocked: "ip" });
  assert.deepEqual(feedbackReservationFromRpc({ allowed: false, blocked: "week" }), { allowed: false, blocked: null });
  assert.equal(feedbackReservationFromRpc([]), null);
});

// ── the handler, GitHub faked ───────────────────────────────────────────────────────────────────

interface Seen {
  github: { query: string; variables: Record<string, unknown> }[];
  reserved: { device: string; ip: string; day: string }[];
  contacts: [string, string][];
  senders: [string, string][];
  verified: string[];
  logs: string[];
}

function seen(): Seen {
  return { github: [], reserved: [], contacts: [], senders: [], verified: [], logs: [] };
}

// A signed-in user's access token, as the fake verifier knows it, and one that has expired.
const USER = "8d3c1e52-7f0a-4b6e-9c2d-5a1b3e4f6a7b";
const GOOD_TOKEN = "eyJhbGciOiJFUzI1NiJ9.good.sig";
const EXPIRED_TOKEN = "eyJhbGciOiJFUzI1NiJ9.expired.sig";
const nowSeconds = Math.floor(NOW.getTime() / 1000);

function deps(s: Seen, over: Partial<FeedbackDeps> = {}): FeedbackDeps {
  return {
    now: () => NOW,
    log: (l) => s.logs.push(l),
    hash: (v) => hmacHex("test-key", v),
    reserve: async (device, ip, day): Promise<FeedbackReservation> => {
      s.reserved.push({ device, ip, day });
      return { allowed: true, blocked: null };
    },
    github: async (query, variables) => {
      s.github.push({ query, variables });
      return new Response(JSON.stringify({ data: { createDiscussion: { discussion: { url: DISCUSSION } } } }), { status: 200 });
    },
    saveContact: async (url, email) => {
      s.contacts.push([url, email]);
      return true;
    },
    verify: async (token) => {
      s.verified.push(token);
      const base = { sub: USER, aud: "authenticated", role: "authenticated" };
      if (token === GOOD_TOKEN) return { ...base, exp: nowSeconds + 600 };
      if (token === EXPIRED_TOKEN) return { ...base, exp: nowSeconds - 1 };
      return null;
    },
    saveSender: async (url, userId) => {
      s.senders.push([url, userId]);
      return true;
    },
    options: DEFAULT_FEEDBACK,
    ...over,
  };
}

function post(body: unknown, method = "POST", ip = "203.0.113.7", authorization?: string) {
  const headers: Record<string, string> = { "Content-Type": "application/json", "X-Forwarded-For": ip };
  if (authorization !== undefined) headers.Authorization = authorization;
  return new Request("https://example.supabase.co/functions/v1/feedback", {
    method,
    headers,
    body: method === "GET" ? undefined : typeof body === "string" ? body : JSON.stringify(body),
  });
}

async function errorOf(r: Response) {
  return ((await r.json()) as { error: { code: string; message: string; resets_in_seconds?: number } }).error;
}

test("feedback is posted as a discussion in its kind's category and the address answered", async () => {
  const s = seen();
  const r = await handleFeedback(post(appBody()), deps(s));
  assert.equal(r.status, 200);
  assert.deepEqual(await r.json(), { url: DISCUSSION });
  assert.equal(s.github.length, 1);
  assert.equal(s.github[0].query, CREATE_DISCUSSION);
  const input = s.github[0].variables.input as Record<string, string>;
  assert.equal(input.repositoryId, "R_kgDOVO_7YA");
  assert.equal(input.categoryId, "DIC_kwDOVO_7YM4DJIF5");
  assert.equal(input.title, "Please add a dark basemap with labels.");
  assert.ok(input.body.startsWith("> Please add"));
  // Counted under hashes, for today (UTC); never the raw id or address.
  assert.equal(s.reserved[0].day, "2026-10-20");
  assert.match(s.reserved[0].device, /^device:[0-9a-f]{64}$/);
  assert.match(s.reserved[0].ip, /^ip:[0-9a-f]{64}$/);
  assert.ok(!JSON.stringify(s.reserved).includes(INSTALL) && !JSON.stringify(s.reserved).includes("203.0.113.7"));
  // The log says what kind and how it went, nothing the user wrote.
  assert.ok(s.logs.every((l) => !l.includes("basemap") && !l.includes("203.0.113")), s.logs.join("\n"));
});

test("each kind goes to its category; problems are titled [Problem]", async () => {
  for (const [kind, cat, prefix] of [
    ["idea", "DIC_kwDOVO_7YM4DJIF5", ""],
    ["problem", "DIC_kwDOVO_7YM4DJIF4", "[Problem] "],
    ["other", "DIC_kwDOVO_7YM4DJIF3", ""],
  ]) {
    const s = seen();
    await handleFeedback(post(appBody({ kind, title: "Title" })), deps(s));
    const input = s.github[0].variables.input as Record<string, string>;
    assert.equal(input.categoryId, cat, kind);
    assert.equal(input.title, `${prefix}Title`, kind);
  }
});

test("a reply address goes to the private table with the discussion, never to GitHub", async () => {
  const s = seen();
  const r = await handleFeedback(post(appBody({ contact_email: "kim@example.com" })), deps(s));
  assert.equal(r.status, 200);
  assert.deepEqual(s.contacts, [[DISCUSSION, "kim@example.com"]]);
  assert.ok(!JSON.stringify(s.github).includes("kim@"));
  assert.ok(s.logs.every((l) => !l.includes("kim@")));
  // Without one, nothing is stored.
  const t = seen();
  await handleFeedback(post(appBody()), deps(t));
  assert.deepEqual(t.contacts, []);
  // A store that fails doesn't fail the feedback, which is already posted.
  const u = seen();
  const failing = deps(u, { saveContact: async () => Promise.reject(new Error("db down")) });
  assert.equal((await handleFeedback(post(appBody({ contact_email: "kim@example.com" })), failing)).status, 200);
  assert.ok(u.logs.some((l) => l.includes("contact=lost")));
});

test("over a limit: rate_limited, with when it resets, and nothing is posted", async () => {
  for (const blocked of ["device", "ip", "all"] as const) {
    const s = seen();
    const r = await handleFeedback(post(appBody()), deps(s, { reserve: async () => ({ allowed: false, blocked }) }));
    assert.equal(r.status, 429);
    const e = await errorOf(r);
    assert.equal(e.code, "rate_limited");
    assert.equal(e.resets_in_seconds, 2 * 3600);
    assert.equal(s.github.length, 0);
  }
});

test("the device limit counts per install: five a day, the sixth refused", async () => {
  // An in-memory feedback_reserve with the migration's rules.
  const counts = new Map<string, number>();
  const reserve = async (device: string, ip: string, day: string): Promise<FeedbackReservation> => {
    const get = (b: string) => counts.get(`${b}|${day}`) ?? 0;
    if (get(device) >= DEFAULT_FEEDBACK.deviceDaily) return { allowed: false, blocked: "device" };
    if (get(ip) >= DEFAULT_FEEDBACK.ipDaily) return { allowed: false, blocked: "ip" };
    for (const b of [device, ip]) counts.set(`${b}|${day}`, get(b) + 1);
    return { allowed: true, blocked: null };
  };
  const s = seen();
  const statuses: number[] = [];
  for (let n = 0; n < 6; n++) statuses.push((await handleFeedback(post(appBody()), deps(s, { reserve }))).status);
  assert.deepEqual(statuses, [200, 200, 200, 200, 200, 429]);
  // Another install on the same network still gets through.
  const other = appBody({ install_id: "0f8fad5b-d9cb-469f-a165-70867728950e" });
  assert.equal((await handleFeedback(post(other), deps(s, { reserve }))).status, 200);
  // The same install from another network is still limited.
  assert.equal((await handleFeedback(post(appBody(), "POST", "198.51.100.9"), deps(s, { reserve }))).status, 429);
  assert.equal(s.github.length, 6);
});

test("a honeypot hit is told it worked; nothing is posted or counted", async () => {
  const s = seen();
  const r = await handleFeedback(post(appBody({ website: "cheap pills" })), deps(s));
  assert.equal(r.status, 200);
  assert.deepEqual(await r.json(), { url: DEFAULT_FEEDBACK.discussionsUrl });
  assert.equal(s.github.length + s.reserved.length, 0);
});

test("refused requests are neither counted nor posted", async () => {
  const s = seen();
  const d = deps(s);
  assert.equal((await handleFeedback(post(null, "GET"), d)).status, 405);
  assert.equal(await errorOf(await handleFeedback(post(appBody({ message: "x".repeat(5000) })), d)).then((e) => e.code), "too_long");
  assert.equal((await errorOf(await handleFeedback(post("{"), d))).code, "invalid");
  assert.equal(s.github.length + s.reserved.length, 0);
});

test("GitHub failing: upstream, without passing GitHub's words on", async () => {
  const answers: (() => Promise<Response>)[] = [
    async () => new Response(JSON.stringify({ message: "Bad credentials for token ghp_x" }), { status: 401 }),
    async () => new Response(JSON.stringify({ data: null, errors: [{ type: "FORBIDDEN", message: "token lacks scope" }] }), { status: 200 }),
    async () => new Response("<html>unicorn</html>", { status: 502 }),
    async () => Promise.reject(new Error("connect timeout")),
  ];
  for (const github of answers) {
    const s = seen();
    const r = await handleFeedback(post(appBody({ contact_email: "kim@example.com" })), deps(s, { github }));
    assert.equal(r.status, 502);
    const e = await errorOf(r);
    assert.equal(e.code, "upstream");
    assert.ok(!e.message.includes("token") && !e.message.includes("ghp"), e.message);
    // No discussion, so no address is kept.
    assert.deepEqual(s.contacts, []);
  }
});

test("the database failing: upstream, nothing posted", async () => {
  const s = seen();
  const r = await handleFeedback(post(appBody()), deps(s, { reserve: async () => Promise.reject(new Error("db")) }));
  assert.equal(r.status, 503);
  assert.equal((await errorOf(r)).code, "upstream");
  assert.equal(s.github.length, 0);
  const t = seen();
  const nul = await handleFeedback(post(appBody()), deps(t, { reserve: async () => null }));
  assert.equal(nul.status, 503);
});

// ── signed-in senders ───────────────────────────────────────────────────────────────────────────

function postedBody(s: Seen): string {
  return (s.github[0].variables.input as Record<string, string>).body;
}

test("a signed-in sender: the verified id is kept privately and the post says only that", async () => {
  const s = seen();
  const r = await handleFeedback(post(appBody(), "POST", "203.0.113.7", `Bearer ${GOOD_TOKEN}`), deps(s));
  assert.equal(r.status, 200);
  assert.deepEqual(await r.json(), { url: DISCUSSION });
  assert.deepEqual(s.verified, [GOOD_TOKEN]);
  assert.deepEqual(s.senders, [[DISCUSSION, USER]]);
  const body = postedBody(s);
  assert.ok(body.endsWith("Sent from the NimbleMap app\n\nFrom a signed-in user"), body);
  assert.equal(SIGNED_IN_LINE, "From a signed-in user");
  // Never the id, a part of it, the token or anything derived from it on GitHub or in the log.
  const github = JSON.stringify(s.github);
  for (const secret of [USER, USER.replace(/-/g, ""), USER.slice(0, 8), GOOD_TOKEN, "good.sig"]) {
    assert.ok(!github.includes(secret), secret);
    assert.ok(s.logs.every((l) => !l.includes(secret)), secret);
  }
  assert.ok(s.logs.some((l) => l.includes("status=200") && l.includes("auth=user") && l.includes("sender=kept")), s.logs.join("\n"));
  // The same per-install limits: one reservation, under the install's and the address's hashes.
  assert.equal(s.reserved.length, 1);
  assert.match(s.reserved[0].device, /^device:[0-9a-f]{64}$/);
});

test("a token that doesn't verify: posted as anonymous, logged auth=invalid without the token", async () => {
  for (const authorization of [`Bearer ${EXPIRED_TOKEN}`, "Bearer eyJ.forged.sig", "Basic dXNlcjpwYXNz", "Bearer"]) {
    const s = seen();
    const r = await handleFeedback(post(appBody(), "POST", "203.0.113.7", authorization), deps(s));
    assert.equal(r.status, 200, authorization);
    assert.equal(s.github.length, 1, authorization);
    assert.deepEqual(s.senders, [], authorization);
    const body = postedBody(s);
    assert.ok(body.endsWith("---\n\nNimbleMap 0.7.3 · macOS 14.5 · de\n\nSent from the NimbleMap app"), body);
    assert.ok(!body.includes(SIGNED_IN_LINE));
    assert.ok(s.logs.some((l) => l.includes("auth=invalid")), s.logs.join("\n"));
    const token = authorization.replace(/^\S+\s*/, "");
    if (token) assert.ok(s.logs.every((l) => !l.includes(token)), authorization);
  }
  // A verifier that fails (network, keys) is the same: anonymous, still posted.
  const s = seen();
  const failing = deps(s, { verify: async () => Promise.reject(new Error("jwks down")) });
  assert.equal((await handleFeedback(post(appBody(), "POST", "203.0.113.7", `Bearer ${GOOD_TOKEN}`), failing)).status, 200);
  assert.deepEqual(s.senders, []);
  assert.ok(!postedBody(s).includes(SIGNED_IN_LINE));
  // An anon key (verified, but not a user) isn't a signed-in user either.
  const t = seen();
  const anon = deps(t, { verify: async () => ({ sub: undefined, aud: "authenticated", role: "anon", exp: nowSeconds + 600 }) });
  assert.equal((await handleFeedback(post(appBody(), "POST", "203.0.113.7", "Bearer eyJ.anon.sig"), anon)).status, 200);
  assert.deepEqual(t.senders, []);
});

test("no token: unchanged, nothing verified or kept", async () => {
  const s = seen();
  const r = await handleFeedback(post(appBody()), deps(s));
  assert.equal(r.status, 200);
  assert.deepEqual(s.verified, []);
  assert.deepEqual(s.senders, []);
  assert.equal(postedBody(s), discussionBody(feedback(appBody())));
  assert.ok(s.logs.some((l) => l.includes("auth=none") && l.includes("sender=none")));
});

test("an id in the body is not trusted: not kept, not posted", async () => {
  const s = seen();
  await handleFeedback(post(appBody({ user_id: USER, sub: USER, signed_in: true })), deps(s));
  assert.deepEqual(s.senders, []);
  assert.ok(!JSON.stringify(s.github).includes(USER));
  assert.ok(!postedBody(s).includes(SIGNED_IN_LINE));
});

test("a signed-in sender over the limit is refused like anyone", async () => {
  const s = seen();
  const r = await handleFeedback(
    post(appBody(), "POST", "203.0.113.7", `Bearer ${GOOD_TOKEN}`),
    deps(s, { reserve: async () => ({ allowed: false, blocked: "device" }) }),
  );
  assert.equal(r.status, 429);
  assert.equal(s.github.length + s.senders.length, 0);
});

test("the sender's row failing doesn't fail the feedback; GitHub failing keeps no row", async () => {
  const s = seen();
  const lost = deps(s, { saveSender: async () => Promise.reject(new Error("db down")) });
  assert.equal((await handleFeedback(post(appBody(), "POST", "203.0.113.7", `Bearer ${GOOD_TOKEN}`), lost)).status, 200);
  assert.ok(s.logs.some((l) => l.includes("sender=lost")));
  const t = seen();
  const down = deps(t, { github: async () => new Response("", { status: 502 }) });
  assert.equal((await handleFeedback(post(appBody(), "POST", "203.0.113.7", `Bearer ${GOOD_TOKEN}`), down)).status, 502);
  assert.deepEqual(t.senders, []);
});
