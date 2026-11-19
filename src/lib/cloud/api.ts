import { call, on } from "../api";
import type { Settings } from "../types";
import * as demo from "./demo";
import type { CloudSettings, MalformedHand } from "./types";

export const cloudApi = {
  settings: () => call<CloudSettings>("cloud_settings", {}, demo.settings),
  saveSettings: (cloud: CloudSettings) =>
    call<void>("cloud_save_settings", { cloud }, () => demo.saveSettings(cloud)),
  ackNotice: () => call<void>("cloud_ack_notice", {}, demo.ackNotice),
  malformed: () => call<MalformedHand[]>("cloud_malformed", {}, demo.malformed),
  dismiss: (id: number) => call<void>("cloud_dismiss_malformed", { id }, () => demo.dismiss(id)),
  sendReport: (id: number) => call<void>("cloud_send_report", { id }, () => demo.sendReport(id)),
};

/** Calls `handler` whenever the backend stored new settings (from any window). */
export const onCloudChanged = (handler: (cloud: CloudSettings) => void) =>
  on<Settings & { cloud: CloudSettings }>("settings-changed", (s) => handler(s.cloud));

export function errorText(e: unknown): string {
  const text = typeof e === "string" ? e : e instanceof Error ? e.message : "Unbekannter Fehler.";
  return text === "gesperrt" ? "Solange ein Poker-Client läuft, sendet Ringside nichts. Bitte danach erneut versuchen." : text;
}
