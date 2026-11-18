---
name: ringside-design-loop
description: Design-Schleife für Ringside – EINEN UI-Bereich der App in Runden verbessern (Screenshots im Browser-Demo → neutrale Prüfer bewerten 10 Kriterien 0–10 → Mehrheits-Punkte umsetzen → neue Runde), Ziel in Stufen ≥ 9, ≥ 9,5, 10. Vergleichsbild ist der Kit-Katalog (?view=kit); jeder Loop zieht seinen Bereich auf src/lib/ui/ um und senkt die Ratsche. Prüfer gestaffelt: zuerst GPT-6.1 Sol, ab ≥ 8 überall zusätzlich ein frischer Opus, dann GPT-6.1 Terra; kurz vor und beim Ziel je einmal Fable 5.2. Nutzen, wenn ein Auftrag „Design-Loop: <Bereich>“ lautet.
---

# Design-Loop – 1 Auftrag = 1 UI-Bereich

Du bist der Bauer, nicht der Prüfer. Du baust, fotografierst und lässt neutral bewerten. Prüfer kennen nie ein Ziel.
Alles auf Deutsch: Antworten, sichtbares Denken, Tool-Beschreibungen, Bericht. UI-Texte in Du-Form, echte Umlaute.

## Grundregeln
- **Eigener Checkout:** Arbeite in einem eigenen Git-Worktree auf einem Branch `design/<bereich>` von `main` (andere Helfer arbeiten im Haupt-Checkout). Nach dem Anlegen `pnpm install`. Kleine Commits (Conventional Commits, Ende `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`), Branch pushen; zusammenführen macht der Koordinator. Nie `git stash`, `reset`, `checkout` auf fremde Arbeit, kein breites `pkill`.
- **Nur Optik, Anordnung, Bedienung, Texte.** Keine neue Rechenlogik, keine Änderung an Engine-Ergebnissen. Fehlt für eine bessere Bedienung ein Datum aus dem Backend, kleinste Ergänzung mit Test und im Bericht nennen.
- **Zielgruppe:** der Gelegenheitsspieler – zahlt mal 20 € ein, spielt kleine Turniere zum Spaß und will besser werden, ohne Fachbegriffe zu pauken; keine Profis, keine Optionsflut. Dieselben Daten wie Profi-Tools, aber verständlich. Antwort und nächster Schritt zuerst, Details auf Wunsch; Klartext, Pokerbegriffe dort erklärt, wo sie stehen (Tooltip oder ein Satz), keine unerklärten Abkürzungen; geführte Abläufe und gute Voreinstellungen statt Konfiguration; jede Zahl bekommt Bedeutung (gut/schlecht, warum, was tun) und einen Weg zum Üben oder Nachlesen.
- **UI-Standard des Nutzers:** visuell und smart statt Formularwüsten. Eingaben über Tisch, Karten, Charts, Schieber und Auswahlkarten; Werte ableiten statt abfragen; Expertenfelder nur aufklappbar. Jede Hand überall mit einem Klick analysierbar. Lade für die Arbeit das Skill `frontend-design`.
- **Feste Entscheide (nicht umwerfen, auch wenn Prüfer es wollen – dann ENTSCHEID-Zeile im Bericht):**
  - Dunkles Theme mit den Tokens aus `src/lib/theme.css` (Flächen `--surface-*`, Linien `--line-*`, Text `--text-*`, Status `--pos` grün, `--neg` rot, `--amber` Hinweis). Keine neuen Farbsysteme; neue Tokens nur dort.
  - HUD am Tisch (`src/lib/hud/`): grafischer Modus = Ring mit vier Segmenten um den Avatar plus Platte über dem PokerStars-Namensschild; klassischer Modus = Textbox mit Presets. PokerStars-Regeln: höchstens drei Farben je Wert, keine Spieler-Etiketten, keine situationsabhängigen Werte.
  - Ehrlichkeit bleibt sichtbar: Kennzeichnungen „Modell“, „Gleichgewicht“, „Solver“, „Einschätzung“, Annahmen-Hinweise und „gesperrt, solange PokerStars läuft“ werden nie für die Optik entfernt, nur besser gestaltet.
  - Seitenleiste mit Gruppen bleibt; Seiten sind zentriert (`.page`, max. 1360 px – eine Seite darf breiter, wenn sie es braucht). Mindestbreite der App 980 px.
  - Logo (Vier-Phasen-Ring mit Pik) und Name „Ringside“ unverändert.
- **Rechner schonen:** Dev-Server nur für die Bilderrunde (das Skript startet und beendet ihn selbst), eigene freie Ports, nie Video, nur Einzelbilder.

## Vorbild GTO Wizard
Der Nutzer hält GTO Wizard für das beste Vorbild an Bedienung und Feinschliff („schau viel mehr bei GTO Wizard, immer und immer wieder“). Vor jeder Runde:
- Referenzbilder ansehen: `.refs/gtowizard/` im Repo-Wurzelverzeichnis (git-ignoriert, vom Nutzer gesammelt; nie einchecken, nie an Prüfer als „Ziel“ geben). Wenn für deinen Bereich nichts dabei ist, recherchiere öffentlich (Hilfeseiten, Blog, Bildersuche) und lege gefundene Bilder dort ab.
- Übernommen wird, was gut funktioniert: Hierarchie, Abläufe, Dichte, Zustände, Bewertungs-Darstellung, Tastenbedienung. Nicht übernommen werden Marke, Logo, Farben, Texte und Layout eins zu eins; Ringside behält sein eigenes Designsystem (`src/lib/ui/`, Katalog `?view=kit`).
- Im Bericht kurz nennen, welche Muster aus dem Vorbild du übernommen hast und welche bewusst nicht.

## Kit und Ratsche
- **Bausteine statt Eigenbau:** Jede Runde zieht den Bereich weiter auf `src/lib/ui/` um (Button, Tabs, Card, Stat, GradeBadge, Verdict, SourceNote, RangeGrid, PokerTable, PageHeader, MiniCard …). Fehlt ein Baustein oder eine Variante, kommt sie ins Kit und in den Katalog (`src/lib/ui/Kit.svelte`), nicht lokal in den Bereich.
- **Eine Überschrift:** Titel, Zeile und Bereiche einer Seite gehören in den `PageHeader` (Seite meldet sie mit `usePageHead` an); nie ein eigenes `<h1>` oder Intro-Block, der Titel oder Beschreibung wiederholt.
- **Ratsche:** `node scripts/check-ui.mjs` (läuft in `pnpm check`) zählt feste Farben, freie Schriftgrößen, eigene Knopf- und `kbd`-Stile, freie Bewegung, eigene Seitenüberschriften und lokale Regeln auf globalen Klassen. Mehr als die Baseline ist rot. Am Ende jedes Loops `pnpm ui:ratchet` und die gesunkene `scripts/ui-baseline.json` mit committen. Funde: `node scripts/check-ui.mjs --list`.
- **Kontrast:** `scripts/check-contrast.mjs` misst die Raster-Beschriftung über allen Aktionsfarben; neue Füllfarben für Raster nur als Token.

## Bilder
`.claude/skills/ringside-design-loop/shots.sh <ordner> name='?query' …` macht je Ansicht 980×700, 1440×900 und 2400×1300 in doppelter Auflösung aus dem Browser-Demo (Fallback-Daten aus `src/lib/api.ts`).
- Ansichten: `?tab=dashboard|overview|reports|tournaments|leaks|hands|coach|players|live|training|charts|solver|hud|settings` (weitere Reiter in `src/lib/app/Main.svelte`), Overlay `?view=overlay`, klassisch `?view=overlay&mode=classic&preset=klartext`, Zustände `&locked=1` (PokerStars läuft), `&nohistory=1` (erster Start); Rückblick: `&sub=luck` (Glück & Pech), `&data=few|pending|empty`.
- Ist ein Zustand nicht per URL erreichbar (Unterreiter, Dialog, Ergebnis), ergänze einen kleinen Vorschau-Parameter, der nur im Browser-Demo wirkt (wie `locked`), statt Zustände von Hand zu klicken.
- Je Runde 2–8 Bilder deines Bereichs plus **der Katalog als Vergleichsbild**: `?view=kit` oder der passende Abschnitt `?view=kit&only=<abschnitt>` (`seitenkopf`, `grundlagen`, `bewegung`, `knoepfe`, `auswahl`, `flaechen`, `schilder`, `bewertung`, `karten`, `tisch`, `raster`, `popups`, `zustaende`); `&gray=1` zeigt ihn in Graustufen, `&zoom=<n>` vergrößert. Ablage `/tmp/ringside-shots/<bereich>/r<n>/`. Selbst ansehen, bevor Prüfer sie bekommen.
- **Zustände gehören dazu**, nicht nur die Ruhe-Ansicht: Hover und Tooltips (auch am rechten und unteren Fensterrand), Tastaturfokus, angeheftete Popups und Dialoge, leer (keine Daten), lädt, Fehler, gesperrt (`&locked=1`). Was per URL nicht erreichbar ist, bekommt einen Vorschau-Parameter (nur Browser-Demo). Für Hover-Zustände reicht ein kleines Skript mit dem lokalen ChromeDriver (`/tmp/chromedriver/chromedriver/unz/chromedriver-mac-arm64/chromedriver --port=<frei>`), das die Maus bewegt und ein Bild macht; Skripte bleiben in `/tmp`.
- **Abläufe gehören dazu**, nicht nur Einzelbilder: Was in Schritten passiert (eine Hand am Tisch, ein Solve, eine Analyse), wird Schritt für Schritt fotografiert, als nummerierte Bildfolge an die Prüfer. Man muss in jedem Bild verstehen, was gerade passiert ist, wer noch dabei ist, wer dran ist und worauf sich eine Bewertung bezieht.
- **Winzige Details zählen** (Nutzer: „wirklich auf die winzigen Details achten“): Chips und Einsätze liegen auf der Linie zwischen Platz und Tischmitte, gleich weit für alle Plätze, nie außerhalb des Filzes oder über Platz, Board und Pot; Aktions-Schilder sind für alle Spieler gleich; Zahlenformat (Komma, „BB“), Ausrichtung, Abstände und Schriftgrößen sind einheitlich. Positionen bei 6- und 9-Spieler-Tischen und in allen Breiten mit einem kleinen Skript nachmessen (Überlappungen, Symmetrie).
- **Bewegung zählt** (Nutzer: „Animationen und Transitions sind wichtig, nicht linear, sondern cubic – Kleinigkeiten sind wichtig“): Übergänge nutzen die Motion-Tokens des Kits (Dauern und `cubic-bezier`-Kurven aus `src/lib/theme.css`), nie `linear` oder `ease` für Bewegung von Elementen. Ein-/Ausblenden, Chips, Karten, Popups, Seiten- und Zustandswechsel wirken ruhig und gleichartig; `prefers-reduced-motion` schaltet Bewegung ab. Für Prüfer: Zwischenstände einer Animation als Bildfolge festhalten (z. B. 0 %, 50 %, 100 %), damit Kurve und Ablauf beurteilbar sind.
- **Randfälle gehören in jede Runde:** Hover am Fensterrand, Überlauf bei langen Namen und großen Zahlen (z. B. „1.234.567 BB“, 20-stellige Turniernamen), leere Listen, sehr viele Einträge, schmale und breite Fenster, Tastaturfokus.
- Prüfe bei jeder Runde selbst: kein horizontales Scrollen, nichts abgeschnitten, Popups bleiben im Fenster, bei 2400 px kein verlorener Leerraum.

## Ablauf je Runde
1. **Bilder** wie oben.
2. **Prüfer** (nur Analyse, gleiche Bilder, Pfade im Auftrag; jede Runde frische Aufrufe, nie einen alten Prüfer weiterfragen):
   - **Stufe 1:** nur GPT-6.1 Sol – T3 `delegate_task` mit `target: { providerInstanceId: "codex", model: "gpt-6.1-sol", options: { reasoningEffort: "high" } }`, `role: "review"`, `mode: "wait"` – bis Sol in **allen 10 Kriterien ≥ 8** gibt.
   - **Stufe 2:** zusätzlich ein frischer Opus (Agent-Tool, `general-purpose`, ohne Kontext außer dem Prüfer-Auftrag).
   - **Stufe 3:** zusätzlich GPT-6.1 Terra (`model: "gpt-6.1-terra"`, sonst wie Sol).
   - **Stufe 4 – Fable 5.2 als finaler Geschmacks-Prüfer**, genau zweimal je Zielstufe: (a) wenn der Schnitt kurz vor der aktuellen Zielstufe liegt (die meisten Kriterien schon dort), (b) beim Erreichen als letzter Blick. Aufruf: `delegate_task` mit `target: { providerInstanceId: "claude_vibeproxy", model: "claude-fable-5-2", options: { effort: "high" } }`, gleicher Prüfer-Auftrag. Fables konkrete Punkte zählen bei Geschmack und Feinschliff wie eine Mehrheit. **Plateau:** Bewegen sich die Noten drei Runden kaum und hängen Restpunkte nur an festen Entscheiden, darf (a) auch früher kommen; danach umsetzen und abschließen, (b) entfällt.
3. **Auswerten:** Was die Mehrheit der beteiligten Prüfer konkret nennt (Stufe 1: was Sol nennt) oder wo eine Note deutlich unter dem Rest liegt, zuerst verbessern. Einzelstimmen nur, wenn billig und regelkonform. Widerspricht ein Vorschlag einem festen Entscheid oder einer früheren Mehrheit → nicht umsetzen, ENTSCHEID-Zeile (sonst kippt die Schleife hin und her).
4. **Fix → neue Bilder → neue Runde.** Kleine CSS-Fixes ohne eigene Runde. Das Ziel steigt in Stufen (Nutzer: „wenn wir 9 erreichen, dann auf 9,5 hoch und dann auf 10“): zuerst geben in Stufe 3 alle Prüfer in allen Kriterien ≥ 9,0, dann ≥ 9,5, dann 10. Erst danach ist der Bereich fertig. Keine Runden- oder Zeitgrenze. Die Plateau-Regel gilt je Stufe: hängen die Restpunkte nur an festen Entscheiden, die Stufe mit ENTSCHEID-Zeilen abschließen und die nächste versuchen.

## Prüfer-Auftrag (wörtlich so verwenden, Platzhalter füllen)
> Du bekommst Screenshots einer Desktop-App für Pokerspieler, die gerade besser werden (keine Profis); Pfade unten, je Ansicht in drei Fensterbreiten. Sieh dir alle Bilder an. Bewerte die Oberfläche in diesen 10 Kriterien jeweils mit einer Note von 0 bis 10 (Kommastellen erlaubt):
> 1. Visuelle Hierarchie (was ist wichtig, was nicht)
> 2. Lesbarkeit (Schriftgrößen, Kontrast, Zeilen)
> 3. Klarheit für die Zielgruppe: Versteht ein Gelegenheitsspieler (kleine Turniere, kein Fachwissen) sofort, was er hier tun kann, was die Zahlen bedeuten und was er daraus machen soll – und bei Bildfolgen in jedem Schritt, was gerade passiert ist?
> 4. Einheitliches Designsystem (gleiche Farben, Schriftrollen, Radien, Abstände wie im Vergleichsbild)
> 5. Knopf- und Aktions-Hierarchie (Hauptaktion, Nebenaktionen, leise Aktionen, Gefahr klar unterscheidbar)
> 6. Ordnung und Ausrichtung (Raster, Bündigkeit, Symmetrie)
> 7. Dichte und Leerraum (nicht überladen, nicht leer; großer Bildschirm sinnvoll genutzt)
> 8. Visuelle Bedienung statt Formular (Eingaben über Tisch, Karten, Charts und Auswahl statt Feldern; Werte abgeleitet statt abgefragt)
> 9. Funktioniert auf allen gezeigten Fensterbreiten und in allen gezeigten Zuständen (Hover, Fokus, leer, Fehler; nichts abgeschnitten, nichts überläuft)
> 10. Gesamteindruck und Wertigkeit (wirkt es wie ein hochwertiges, eigenständiges Werkzeug oder austauschbar?)
>
> Du musst nichts finden. „Passt so“ ist eine gültige Antwort. Nur wenn dir etwas wirklich auffällt: sag kurz und konkret, was du ändern würdest (welches Element, wie). Antworte mit einer Tabelle der 10 Noten und danach höchstens 5 konkreten Punkten.
> Bilder: <Pfade> · Vergleichsbild: <Pfad>

**Verboten im Prüfer-Auftrag:** jedes Ziel oder Zielnote, „wir haben gebaut/verbessert“, vorherige Noten, frühere Runden, Vorher-Bilder als „alt“, Hinweise auf gewünschte Ergebnisse, die festen Entscheide.

## Abschluss
- Vor dem Abschluss den lokalen E2E-Durchlauf gegen die Browser-Demo laufen lassen (`E2E_URL=http://localhost:<port>/ E2E_WEBDRIVER=http://localhost:<port> node e2e/run.mjs`, Vite und ChromeDriver auf freien Ports, danach beenden). Wenn du Texte oder Selektoren änderst, die `e2e/run.mjs` benutzt, passe den Test im selben Branch an.
- Grün vor jedem Push: `pnpm check` (0 Fehler, 0 Warnungen, Ratsche und Kontrast grün, Baseline mit `pnpm ui:ratchet` gesenkt) und `pnpm build`; bei Rust-Änderungen zusätzlich `cargo clippy --workspace --all-targets -- -D warnings` und `cargo test --workspace`. Volle Logs in Dateien umleiten und die roten Stellen lesen, nie gekürzte Ausgaben als grün werten.
- Bildskripte und Screenshots nicht einchecken (bleiben in `/tmp`).
- Bericht an den Koordinator (Deutsch, knapp): Branch und letzter Commit, Noten je Runde und Prüfer als Tabelle, wichtigste Änderungen mit Vorher/Nachher-Bildpfaden, ENTSCHEID-Zeilen, was offen bleibt.
