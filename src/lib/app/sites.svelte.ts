import type { ImportStatus, Site } from "../types";

/** Stored hands per poker client, from the latest import status. */
export const storedSites = $state<{ counts: Partial<Record<Site, number>> }>({ counts: {} });

export function noteSites(status: ImportStatus | null) {
  if (status?.sites) storedSites.counts = status.sites;
}

/** Client marks and the client filter only show once hands of more than one client are stored. */
export function severalSites(): boolean {
  return Object.values(storedSites.counts).filter((n) => (n ?? 0) > 0).length > 1;
}
