import { WORDS } from "../ui/words";

/**
 * The pages of the app, sorted by what the player wants to do rather than by tool: the one list
 * for the top bar, its menus, page titles and the layout check.
 */

export type Page =
  | "dashboard"
  | "overview"
  | "reports"
  | "leaks"
  | "ranges"
  | "tournaments"
  | "hands"
  | "coach"
  | "players"
  | "live"
  | "training"
  | "play"
  | "charts"
  | "solver"
  | "library"
  | "hud"
  | "settings";

export type AreaId = "start" | "spiel" | "ueben" | "gegner" | "werkzeuge";

export interface PageInfo {
  label: string;
  /** One sentence for menus and the start page: what you get there. */
  text: string;
  /** A few words for a status line when there is no live value. */
  short?: string;
  icon: string;
}

/** The HUD icon: the four segments of the ring around a seat. */
const RING =
  "M13.3 4.61A7.5 7.5 0 0 1 19.39 10.7M19.39 13.3A7.5 7.5 0 0 1 13.3 19.39M10.7 19.39A7.5 7.5 0 0 1 4.61 13.3M4.61 10.7A7.5 7.5 0 0 1 10.7 4.61";

export const PAGES: Record<Page, PageInfo> = {
  dashboard: {
    label: "Start",
    text: "Deine letzte Session auf einen Blick: Ergebnis, Glück oder Fehler und was du als Nächstes übst.",
    icon: "M4.5 11 12 4.5l7.5 6.5V19a.5.5 0 0 1-.5.5h-4.5v-5h-5v5H5a.5.5 0 0 1-.5-.5z",
  },
  overview: {
    label: "Übersicht",
    text: "Alle Hände im gewählten Zeitraum: Ergebnis, Spielstil und wo du Chips verlierst.",
    icon: "M4 19.5h16M5 15l4.5-4.5 3 3L19 7",
  },
  tournaments: {
    label: "Turniere",
    text: "Jedes Turnier mit Platz und Gewinn. Ein Klick öffnet seinen Bericht.",
    icon: "M8 4.5h8V9a4 4 0 0 1-8 0zM8 6.5H5.5v1a3 3 0 0 0 2.7 3M16 6.5h2.5v1a3 3 0 0 1-2.7 3M12 13v3.5M8.5 19.5h7M10 16.5h4",
  },
  hands: {
    label: "Hände",
    text: "Alle gespielten Hände. Abspielen und jede Entscheidung bewerten lassen.",
    icon: "M10 4h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM6 7.5V19a1 1 0 0 0 1 1h8",
  },
  leaks: {
    label: WORDS.leaks.word,
    text: "Wo du immer wieder Chips verlierst und was dagegen hilft.",
    icon: "M12 4.5c3 4 5.5 6.9 5.5 10a5.5 5.5 0 0 1-11 0c0-3.1 2.5-6 5.5-10zM9.5 15.5a2.5 2.5 0 0 0 2.5 2.5",
  },
  reports: {
    label: "Statistik",
    text: "Starthände, Positionen und Sessions in Zahlen, für jeden Zeitraum.",
    icon: "M4.5 5h15v14h-15zM4.5 10h15M4.5 14.5h15M10 10v9",
  },
  ranges: {
    label: "Meine Starthände",
    text: "Welche Hände du je Position wirklich spielst, neben dem Richtwert.",
    icon: "M4.5 4.5h15v15h-15zM4.5 12h15M12 4.5v15M7 7h2.5v2.5H7zM14.5 14.5H17V17h-2.5z",
  },
  training: {
    label: "Lernen",
    text: "Kurze Runden aus deinen eigenen Fehlern, ein Lernpfad und Wissen zum Nachlesen.",
    short: "Kurze Runden, eigene Fehler",
    icon: "M12 5 3.5 9l8.5 4 8.5-4zM7 10.7v4.3c0 1.3 2.2 2.8 5 2.8s5-1.5 5-2.8v-4.3M20.5 9v5",
  },
  play: {
    label: "Hände spielen",
    text: "Spiel Hände gegen Bots. Jede Entscheidung wird sofort bewertet.",
    short: "Gegen Bots, sofort bewertet",
    icon: "M12 5.5c4.4 0 8 2.9 8 6.5s-3.6 6.5-8 6.5-8-2.9-8-6.5 3.6-6.5 8-6.5zM9 10h2.6v4H9zM12.4 10H15v4h-2.6z",
  },
  coach: {
    label: "KI-Coach",
    text: "Ein Sprachmodell erklärt deine Zahlen und bespricht Turniere mit dir.",
    icon: "M4.5 5.5h15v10.5h-8l-4.5 3.5v-3.5H4.5zM8.5 9.5h7M8.5 12.5h4.5",
  },
  players: {
    label: "Spieler",
    text: "Deine Gegner: wie sie spielen und wie du sie schlägst.",
    icon: "M12 11.5a3.75 3.75 0 1 0 0-7.5 3.75 3.75 0 0 0 0 7.5zM5 20a7 7 0 0 1 14 0",
  },
  live: {
    label: "Live-Tische",
    text: "Wer gerade mit dir am Tisch sitzt, mit allen Werten.",
    icon: "M12 6.5c4.7 0 8.5 2.5 8.5 5.5s-3.8 5.5-8.5 5.5S3.5 15 3.5 12 7.3 6.5 12 6.5zM12 9.5c2.8 0 5 1.1 5 2.5s-2.2 2.5-5 2.5-5-1.1-5-2.5 2.2-2.5 5-2.5z",
  },
  hud: {
    label: "HUD am Tisch",
    text: "Die Anzeige über deinen Gegnern am Tisch: was sie zeigt und wo sie sitzt.",
    icon: RING,
  },
  charts: {
    label: "Starthand-Tabellen",
    text: "Welche Hände du je Stack und Position spielst oder all-in schiebst.",
    icon: "M4.5 4.5h15v15h-15zM4.5 9.5h15M4.5 14.5h15M9.5 4.5v15M14.5 4.5v15",
  },
  solver: {
    label: WORDS.solver.word,
    text: "Eine Situation genau durchrechnen lassen. Läuft nur auf diesem Rechner.",
    icon: "M5 19.5V13M10 19.5V8.5M15 19.5v-8M20 19.5V4.5M3.5 19.5h17",
  },
  library: {
    label: "Gelöste Situationen",
    text: "Fertig gerechnete Situationen zum Nachschlagen, von hier oder aus der Cloud.",
    icon: "M5 4.5h3v15H5zM10 4.5h3v15h-3zM15.2 5.3l2.9-.8 3.6 14.4-2.9.8z",
  },
  settings: {
    label: "Einstellungen",
    text: "Ordner, Updates, Cloud und KI-Coach einrichten.",
    icon: "M4 7.5h9M17 7.5h3M4 16.5h3M11 16.5h9M15 5.5v4M9 14.5v4",
  },
};

export interface Area {
  id: AreaId;
  label: string;
  /** The question the area answers, in the player's words. */
  text: string;
  /** What is inside, in a few words, for a status line. */
  short: string;
  icon: string;
  pages: Page[];
}

export const AREAS: Area[] = [
  { id: "start", label: "Start", text: "Wie lief deine letzte Session?", short: "Deine letzte Session", icon: PAGES.dashboard.icon, pages: ["dashboard"] },
  {
    id: "spiel",
    label: "Mein Spiel",
    text: "Wie läuft es, und wo verlierst du?",
    short: "Ergebnisse und Schwachstellen",
    icon: "M4 19.5h16M5 15l4.5-4.5 3 3L19 7",
    pages: ["overview", "tournaments", "hands", "leaks", "reports", "ranges"],
  },
  { id: "ueben", label: "Üben", text: "Besser werden in kurzen Runden.", short: "Kurze Runden, eigene Hände", icon: PAGES.training.icon, pages: ["training", "play", "coach"] },
  { id: "gegner", label: "Gegner", text: "Wer sitzt am Tisch, und wie spielst du gegen ihn?", short: "Wer mit dir am Tisch sitzt", icon: PAGES.players.icon, pages: ["players", "live", "hud"] },
  {
    id: "werkzeuge",
    label: "Werkzeuge",
    text: "Zum Tüfteln: Tabellen, Nachrechnen, Nachschlagen.",
    short: "Zum Tüfteln",
    icon: "M14.5 5.5a4 4 0 0 0-4.9 5.2L4.5 15.8V19.5h3.7l5.1-5.1a4 4 0 0 0 5.2-4.9l-2.6 2.6-2.3-.6-.6-2.3z",
    pages: ["charts", "solver", "library"],
  },
];

export const ALL_PAGES = Object.keys(PAGES) as Page[];

export const areaOf = (page: Page): Area | undefined => AREAS.find((a) => a.pages.includes(page));

/** Pages PokerStars' rules lock while the client runs. */
export const LOCKED_PAGES: Page[] = ["leaks", "training", "play", "charts", "solver", "library"];
