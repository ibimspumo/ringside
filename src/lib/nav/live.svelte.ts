import { api } from "../api";
import { isActive } from "../jobs/kinds";
import { remaining } from "../ui/format";
import { jobs } from "../jobs/store.svelte";
import { adviceLocked } from "../app/client.svelte";
import { isDue } from "../learn/catalog";
import { libraryApi } from "../library/api";
import type { Filter, HudConfig, TournamentRow } from "../types";
import { int } from "../ui/format";
import { LOCKED_PAGES, type Page } from "./pages";
import { points } from "../skill/labels";
import { skill } from "../skill/store.svelte";

export type Tone = "neutral" | "good" | "warn" | "bad";
/**
 * A live value of a side bar entry in two lengths: `text` fits beside its name ("4"), `long`
 * reads on its own in the tooltip ("4 gefunden").
 */
export interface Live {
  text: string;
  long: string;
  tone: Tone;
  /** A task of this page is running right now. */
  busy?: boolean;
}

/** The value beside an entry, only where it calls for something: a warning, an open table, a running task. */
export const tight = (l: Live | undefined): string | undefined => (l && (l.tone !== "neutral" || l.busy) ? l.text : undefined);

const live = (text: string, long = text, tone: Tone = "neutral"): Live => ({ text, long, tone });

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

/** Live values for the side bar, read from the same calls the pages use; nothing is computed anew. */
class NavLive {
  values = $state<Partial<Record<Page, Live>>>({});

  async load(filter: Filter, tournaments: TournamentRow[], tables: number, hands: number, hud: HudConfig | null) {
    const v: Partial<Record<Page, Live>> = {};
    if (hands) v.reports = live(int(hands), `${int(hands)} Hände`);
    if (hud) v.hud = live(hud.mode === "graphic" ? "grafisch" : "klassisch");
    const last = [...tournaments].sort((a, b) => b.started_at.localeCompare(a.started_at))[0];
    if (last?.place && last.entrants) v.dashboard = live(`Platz ${int(last.place)}`, `Zuletzt Platz ${int(last.place)} von ${int(last.entrants)}`);
    if (tournaments.length) v.tournaments = live(int(tournaments.length), `${int(tournaments.length)} gespielt`);
    v.live = tables > 0 ? live(String(tables), tables === 1 ? "1 Tisch offen" : `${tables} Tische offen`, "good") : live("", "keiner offen");

    // The leaks page lists these steps, numbered; the mark counts the same list.
    const [leaks, summary, progress, library, players] = await Promise.allSettled([
      api.nextSteps(filter),
      api.analysisSummary(filter),
      api.learnProgress(),
      libraryApi.overview(),
      api.players(filter, "", 300),
    ]);
    if (players.status === "fulfilled" && players.value.length) {
      const n = players.value.length >= 300 ? "300+" : int(players.value.length);
      v.players = live(n, `${n} Gegner`);
    }
    if (leaks.status === "fulfilled" && leaks.value.length) {
      const n = leaks.value.length;
      v.leaks = live(String(n), `${n} gefunden`, "warn");
    }
    if (summary.status === "fulfilled" && summary.value.costliest.length) {
      const n = summary.value.costliest.length;
      v.hands = live(String(n), `${n} teure Fehler`, "bad");
    }
    if (progress.status === "fulfilled") {
      const due = progress.value.skills.filter(isDue).length;
      const goal = weeklyGoal();
      const week = Math.min(progress.value.xp_week, goal);
      v.training = live(`${week}/${goal}`, `Wochenziel ${week}/${goal}${due ? ` · ${due} fällig` : ""}`, week >= goal ? "good" : "neutral");
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
      // Names what runs ("Analyse"), and the time left in the same words as the page below.
      const what = job.kind === "analysis" ? "Analyse" : "Rechnung";
      const eta = remaining(job.eta_secs);
      const short = job.status === "paused" ? `${what} pausiert` : `${what} läuft`;
      const text = job.status === "paused" ? short : eta ? `${short} · ${eta}` : `${short}${pct ? ` · ${pct.trim()}` : ""}`;
      return { ...live(short, text), busy: true };
    }
    if (page === "skill" && skill.profile?.rating != null) {
      const r = points(skill.profile.rating);
      return live(r, `${r} · ${skill.profile.tier}`);
    }
    return this.values[page];
  }
}

export const navLive = new NavLive();
