// The account function (Supabase Edge Function `account`): GET shows Pro and today's use; DELETE
// deletes the signed-in user (profiles and usage rows go with it, `on delete cascade`).

import { createClient } from "npm:@supabase/supabase-js@2";

import { handleAccount } from "../_shared/handler.ts";
import { limitsFromEnv } from "../_shared/limits.ts";

const env = (name: string) => Deno.env.get(name);
const admin = createClient(env("SUPABASE_URL") ?? "", env("SUPABASE_SERVICE_ROLE_KEY") ?? "", {
  auth: { persistSession: false, autoRefreshToken: false },
});
const limits = limitsFromEnv(env);

Deno.serve((req) =>
  handleAccount(req, {
    verify: async (token) => {
      const { data, error } = await admin.auth.getClaims(token);
      return error || !data ? null : data.claims;
    },
    now: () => new Date(),
    log: (line) => console.log(line),
    profile: async (id) => {
      const { data, error } = await admin.from("profiles").select("pro, plan").eq("id", id).maybeSingle();
      return error || !data ? null : { pro: data.pro === true, plan: data.plan ?? null };
    },
    usageToday: async (id, day) => {
      const { data } = await admin
        .from("usage_daily")
        .select("requests, prompt_tokens, completion_tokens")
        .eq("user_id", id)
        .eq("day", day)
        .maybeSingle();
      return {
        requests: Number(data?.requests ?? 0),
        tokens: Number(data?.prompt_tokens ?? 0) + Number(data?.completion_tokens ?? 0),
      };
    },
    deleteUser: async (id) => {
      const { error } = await admin.auth.admin.deleteUser(id);
      return !error;
    },
    limits,
  })
);
