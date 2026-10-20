// In-app feedback (Supabase Edge Function `feedback`): Help › Send Feedback… posts here without
// signing in; the message becomes a GitHub Discussion (docs/lm/feedback.md).
// Secrets: GITHUB_FEEDBACK_TOKEN (fine-grained, this repository only, Discussions read and write);
// optional FEEDBACK_HASH_KEY (the key of the install-id and address hashes; the service role key
// when unset), FEEDBACK_REPOSITORY_ID, FEEDBACK_CATEGORY_IDEA, FEEDBACK_CATEGORY_PROBLEM,
// FEEDBACK_CATEGORY_OTHER, FEEDBACK_DISCUSSIONS_URL, FEEDBACK_DEVICE_DAILY, FEEDBACK_IP_DAILY,
// FEEDBACK_GLOBAL_DAILY.
// SUPABASE_URL and SUPABASE_SERVICE_ROLE_KEY are provided by Supabase.

import { createClient } from "npm:@supabase/supabase-js@2";

import { feedbackOptionsFromEnv, feedbackReservationFromRpc, handleFeedback, hmacHex } from "../_shared/feedback.ts";
import { errorResponse } from "../_shared/handler.ts";

const env = (name: string) => Deno.env.get(name);
const serviceKey = env("SUPABASE_SERVICE_ROLE_KEY") ?? "";
const admin = createClient(env("SUPABASE_URL") ?? "", serviceKey, {
  auth: { persistSession: false, autoRefreshToken: false },
});
const token = env("GITHUB_FEEDBACK_TOKEN") ?? "";
const hashKey = env("FEEDBACK_HASH_KEY") || serviceKey;
const options = feedbackOptionsFromEnv(env);

Deno.serve((req) => {
  if (!token || !hashKey) {
    console.log("feedback status=503 misconfigured");
    return errorResponse(503, "upstream", "Feedback isn't set up on the server yet.");
  }
  return handleFeedback(req, {
    now: () => new Date(),
    log: (line) => console.log(line),
    hash: (value) => hmacHex(hashKey, value),
    reserve: async (device, ip, day, o) => {
      const { data, error } = await admin.rpc("feedback_reserve", {
        p_device: device,
        p_ip: ip,
        p_day: day,
        p_max_device: o.deviceDaily,
        p_max_ip: o.ipDaily,
        p_max_all: o.globalDaily,
      });
      return error ? null : feedbackReservationFromRpc(data);
    },
    github: (query, variables) =>
      fetch("https://api.github.com/graphql", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "application/json",
          "User-Agent": "NimbleMap-feedback",
        },
        body: JSON.stringify({ query, variables }),
        signal: AbortSignal.timeout(15_000),
      }),
    saveContact: async (url, email) => {
      const { error } = await admin.from("feedback_contacts").insert({ discussion_url: url, email });
      return !error;
    },
    options,
  });
});
