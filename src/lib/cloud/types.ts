/** Mirrors `CloudSettings` in src-tauri/src/settings.rs. */
export interface CloudSettings {
  fetch: boolean;
  share: boolean;
  notice_seen: boolean;
  /** Practice takes spots solved already, also by other players. */
  practice_ready: boolean;
}

/** A hand the importer could not read, already anonymized as it would be sent. */
export interface MalformedHand {
  id: number;
  reason: string;
  hand: string;
}
