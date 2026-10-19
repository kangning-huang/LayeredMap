// The sign-in emails (supabase/templates) and their wiring in supabase/config.toml: both emails a
// first or a returning sign-in can get show the 6-digit code and the link, name the app, and say
// the email can be ignored; config.toml points at files that exist and asks for 6-digit codes.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const config = readFileSync(join(root, "supabase/config.toml"), "utf8");

/** The key = value lines of one [table] in config.toml (enough TOML for this file). */
function table(name: string): Map<string, string> {
  const out = new Map<string, string>();
  let inside = false;
  for (const raw of config.split("\n")) {
    const line = raw.trim();
    if (line.startsWith("[")) {
      inside = line === `[${name}]`;
    } else if (inside && line && !line.startsWith("#")) {
      const [k, ...v] = line.split("=");
      out.set(k.trim(), v.join("=").trim());
    }
  }
  return out;
}

for (const name of ["magic_link", "confirmation"]) {
  test(`the ${name} email shows the code and the link`, () => {
    const t = table(`auth.email.template.${name}`);
    assert.match(t.get("subject") ?? "", /^"Your NimbleMap sign-in code"$/);
    const path = JSON.parse(t.get("content_path") ?? '""');
    // The CLI reads email templates relative to the directory holding supabase/.
    assert.ok(path.startsWith("./supabase/templates/"), path);
    assert.ok(existsSync(join(root, path)), `${path} exists`);
    const html = readFileSync(join(root, path), "utf8");
    assert.ok(html.includes("{{ .Token }}"), "the code");
    assert.ok(html.includes('href="{{ .ConfirmationURL }}"'), "the link");
    assert.ok(html.includes("NimbleMap"));
    assert.match(html, /If you didn't ask to sign in to NimbleMap, you can ignore this email\./);
    assert.ok(!/<script|<img|src=|https?:\/\//i.test(html), "nothing loaded from elsewhere");
  });
}

test("codes are 6 digits for an hour, and Confirm email stays on", () => {
  const e = table("auth.email");
  assert.equal(e.get("otp_length"), "6");
  assert.equal(e.get("otp_expiry"), "3600");
  assert.equal(e.get("enable_signup"), "true");
  assert.equal(e.get("enable_confirmations"), "true");
});
