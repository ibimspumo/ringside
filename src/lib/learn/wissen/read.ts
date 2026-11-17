const STORE = "ringside.wissen.read";

/** Articles marked as read, kept in this browser profile only. */
export function loadRead(): Set<string> {
  try {
    const raw = localStorage.getItem(STORE);
    const ids = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(ids) ? ids.filter((x) => typeof x === "string") : []);
  } catch {
    return new Set();
  }
}

export function storeRead(ids: Set<string>) {
  try {
    localStorage.setItem(STORE, JSON.stringify([...ids]));
  } catch {
    // Storage can be unavailable; the marks then last for this session only.
  }
}
