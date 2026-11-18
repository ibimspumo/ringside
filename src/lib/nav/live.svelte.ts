import { api } from "../api";
import { isActive } from "../jobs/kinds";
import { jobs } from "../jobs/store.svelte";
import { adviceLocked } from "../app/client.svelte";
import { isDue } from "../learn/catalog";
import { libraryApi } from "../library/api";
import type { Filter, HudConfig, TournamentRow } from "../types";
import { int } from "../ui/format";
import { LOCKED_PAGES, PAGES, type Area, type Page } from "./pages";

export type Tone = "neutral" | "good" | "warn" | "bad";
/**
 * A live value next to a menu entry in two lengths: `text` fits beside a name in a narrow bar
 * ("4"), `long` reads on its own in menus and tiles ("4 gefunden"). `line` names the page too, for
 * a line about a whole area ("4 Schwachstellen").
 */
export interface Live {
  text: string;
  long: string;
  line?: string;
  tone: Tone;
  /** A task of this page is running right now. */
  busy?: boolean;
}

/** The short value for a narrow bar, only where it calls for something: a warning, an open table, a running task. */
export const tight = (l: Live | undefined): string | undefined => (l && (l.tone !== "neutral" || l.busy) ? l.text : undefined);

const live = (text: string, long = text, tone: Tone = "neutral", line?: string): Live => ({ text, long, tone, line });

/** Same store and default as the weekly goal on the learning page. */
const GOAL_STORE = "ringside.learn.goal";
function weeklyGoal(): number {
  try {
    const g = Number(localStorage.getItem(GOAL_STORE));
    return [20, 50, 100].includes(g) ? g : 50;
  } catch {
    return 50;
  }
}

/** Live values for the menus, read from the same calls the pages use; nothing is computed anew. */
class NavLive {
  values = $state<Partial<Record<Page, Live>>>({});

  async load(filter: Filter, tournaments: TournamentRow[], tables: number, hands: number, hud: HudConfig | null) {
    const v: Partial<Record<Page, Live>> = {};
    if (hands) v.reports = live(int(hands), `${int(hands)} Hände`);
    if (hud) v.hud = live(hud.mode === "graphic" ? "grafisch" : "klassisch");
    const last = [...tournaments].sort((a, b) => b.started_at.localeCompare(a.started_at))[0];
    if (last?.place && last.entrants) v.dashboard = live(`Platz ${int(last.place)}`, `Zuletzt Platz ${int(last.place)} von ${int(last.entrants)}`);
    if (tournaments.length) v.tournaments = live(int(tournaments.length), `${int(tournaments.length)} gespielt`);
    v.live = tables > 0 ? live(String(tables), tables === 1 ? "1 Tisch offen" : `${tables} Tische offen`, "good", tables === 1 ? "1 Tisch offen" : `${tables} Tische offen`) : live("", "keiner offen");

    const [leaks, summary, progress, library, players] = await Promise.allSettled([
      api.leaks(filter),
      api.analysisSummary(filter),
      api.learnProgress(),
      libraryApi.overview(),
      api.players(filter, "", 300),
    ]);
    if (players.status === "fulfilled" && players.value.length) {
      const n = players.value.length >= 300 ? "300+" : int(players.value.length);
      v.players = live(n, `${n} Gegner`);
    }
    if (leaks.status === "fulfilled" && leaks.value.leaks.length) {
      const n = leaks.value.leaks.length;
      v.leaks = live(String(n), `${n} gefunden`, "warn", n === 1 ? "1 Schwachstelle" : `${n} Schwachstellen`);
    }
    if (summary.status === "fulfilled" && summary.value.costliest.length) {
      const n = summary.value.costliest.length;
      v.hands = live(String(n), `${n} teure Fehler`, "bad", `${n} teure Fehler`);
    }
    if (progress.status === "fulfilled") {
      const due = progress.value.skills.filter(isDue).length;
      const goal = weeklyGoal();
      const week = Math.min(progress.value.xp_week, goal);
      v.training = live(
        `${week}/${goal}`,
        `Wochenziel ${week}/${goal}${due ? ` · ${due} fällig` : ""}`,
        week >= goal ? "good" : "neutral",
        due ? `${due} Übungen fällig` : `Wochenziel ${week}/${goal}`,
      );
    }
    if (library.status === "fulfilled") {
      const solved = library.value.scenarios.reduce((n, s) => n + s.status.filter((x) => x !== null).length, 0);
      if (solved) v.library = live(int(solved), `${int(solved)} gelöst`);
    }
    this.values = v;
  }

  /**
   * What the entry shows now: locked first, then a running task, then the live value. While
   * PokerStars runs nothing rates or advises, so warnings and errors stay away then too.
   */
  of(page: Page, locked: boolean): Live | undefined {
    if (locked && LOCKED_PAGES.includes(page)) return live("", "gesperrt");
    // Ratings wait while the client runs and until the backend said it does not.
    const value = this.values[page];
    if (adviceLocked() && value && (value.tone === "warn" || value.tone === "bad")) return undefined;
    const job = jobs.list.find((j) => isActive(j) && j.target.page === page);
    if (job) {
      const pct = job.progress === null ? "" : ` ${Math.floor(job.progress * 100)} %`;
      const text = job.status === "paused" ? "pausiert" : `rechnet${pct || " …"}`;
      return { ...live(text, text), busy: true };
    }
    return this.values[page];
  }
}

export const navLive = new NavLive();

/**
 * One line for a whole area: what needs attention first (a warning, then something live), else
 * what is inside. Locked pages say so.
 */
export function areaLine(area: Area, locked: boolean): Live {
  const shut = area.pages.filter((p) => locked && LOCKED_PAGES.includes(p));
  if (shut.length === area.pages.length) return live("gesperrt");
  if (shut.length) return live(shut.length > 1 ? "teils gesperrt" : `${PAGES[shut[0]].label} gesperrt`);
  const lives = area.pages.flatMap((p) => {
    const l = navLive.of(p, locked);
    return l ? [{ page: p, ...l }] : [];
  });
  const pick = lives.find((l) => l.tone === "warn" || l.tone === "bad") ?? lives.find((l) => l.tone === "good") ?? (area.id === "ueben" ? lives[0] : undefined);
  if (!pick) return live(area.short);
  const text = pick.line ?? (pick.page === area.pages[0] ? pick.long : `${PAGES[pick.page].label}: ${pick.long}`);
  return live(text, text, pick.tone);
}
