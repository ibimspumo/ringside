/** Entry point of the `ringside-api` Edge Script; edge/deploy.ts bundles it into one file. */
import * as BunnySDK from "npm:@bunny.net/edgescript-sdk@0.13.0";
import process from "node:process";
import { createSql } from "../lib/sql.ts";
import { createHandler } from "./api.ts";

function secret(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`secret ${name} is missing`);
  return v;
}

declare const Bunny: { v1?: { waitUntil?: (p: Promise<unknown>) => void } } | undefined;

const handler = createHandler({
  sql: createSql(secret("BUNNY_DATABASE_URL"), secret("BUNNY_DATABASE_AUTH_TOKEN"), { timeoutMs: 8_000 }),
  salt: secret("RINGSIDE_IP_SALT"),
  background: (p) => {
    if (typeof Bunny !== "undefined" && Bunny?.v1?.waitUntil) Bunny.v1.waitUntil(p);
  },
});

BunnySDK.net.http.serve(handler);
