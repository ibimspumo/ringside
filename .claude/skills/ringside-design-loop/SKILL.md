---
name: ringside-design-loop
description: Design-Schleife für Ringside – EINEN UI-Bereich der App in Runden verbessern (Screenshots im Browser-Demo → neutrale Prüfer bewerten 10 Kriterien 0–10 → Mehrheits-Punkte umsetzen → neue Runde), Ziel in Stufen ≥ 9, ≥ 9,5, 10. Vergleichsbild ist der Kit-Katalog (?view=kit); jeder Loop zieht seinen Bereich auf src/lib/ui/ um und senkt die Ratsche. Prüfer gestaffelt: zuerst GPT-6.1 Sol, ab ≥ 8 überall zusätzlich ein frischer Opus, dann GPT-6.1 Terra; kurz vor und beim Ziel je einmal Fable 5.2. Nutzen, wenn ein Auftrag „Design-Loop: <Bereich>“ lautet.
---

# Design-Loop – 1 Auftrag = 1 UI-Bereich

Du bist der Bauer, nicht der Prüfer: bauen, fotografieren, neutral bewerten lassen. Prüfer kennen nie ein Ziel.
Alles auf Deutsch (Antworten, sichtbares Denken, Tool-Beschreibungen, Bericht). UI-Texte in Du-Form, echte Umlaute.

## Grundregeln
- **Eigener Checkout:** eigener Git-Worktree, Branch `design/<bereich>` von `main`, danach `pnpm install`. Kleine Commits (Conventional Commits, Ende `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`), Branch pushen; zusammenführen macht der Koordinator. Nie `git stash`, `reset`, `checkout` auf fremde Arbeit, kein breites `pkill`.
- **Nur Optik, Anordnung, Bedienung, Texte.** Keine neue Rechenlogik, keine Änderung an Engine-Ergebnissen. Fehlt ein Datum aus dem Backend: kleinste Ergänzung mit Test, im Bericht nennen.
- **Zielgruppe:** Gelegenheitsspieler (20 € Buy-in, kleine Turniere, will besser werden ohne Fachbegriffe zu pauken). Antwort und nächster Schritt zuerst, Details auf Wunsch; Klartext, Pokerbegriffe dort erklärt, wo sie stehen; gute Voreinstellungen statt Konfiguration; jede Zahl bekommt Bedeutung (gut/schlecht, warum, was tun) und einen Weg zum Üben oder Nachlesen.
- **Visuell statt Formular:** Eingaben über Tisch, Karten, Charts, Schieber und Auswahlkarten; Werte ableiten statt abfragen; Expertenfelder nur aufklappbar. Jede Hand überall mit einem Klick analysierbar. Skill `frontend-design` laden.
- **Feste Entscheide** (nicht umwerfen, auch wenn Prüfer es wollen – dann ENTSCHEID-Zeile im Bericht):
  - Dunkles Theme mit den Tokens aus `src/lib/theme.css`; neue Farben nur als Token dort.
  - HUD (`src/lib/hud/`): grafisch = Ring mit vier Segmenten plus Platte über dem PokerStars-Namensschild; klassisch = Textbox mit Presets. PokerStars-Regeln: höchstens drei Farben je Wert, keine Spieler-Etiketten, keine situationsabhängigen Werte.
  - Ehrlichkeit bleibt sichtbar: „Modell“, „Gleichgewicht“, „Solver“, „Einschätzung“, Annahmen-Hinweise und „gesperrt, solange PokerStars läuft“ werden nie für die Optik entfernt.
  - Navigation über das Menü der Kopfleiste; eine Seitenbreite (`--page-max`, 1440 px). Mindestbreite der App 980 px.
  - Logo (Vier-Phasen-Ring mit Pik) und Name „Ringside“ unverändert.
- **Rechner schonen:** Dev-Server nur für die Bilderrunde, freie Ports, nur Einzelbilder, nie Video.

## Vorbild GTO Wizard
Der Nutzer hält GTO Wizard für das beste Vorbild an Bedienung und Feinschliff. Vor jeder Runde Referenzbilder in `.refs/gtowizard/` ansehen (git-ignoriert; nie einchecken, nie Prüfern als „Ziel“ geben); fehlt etwas für deinen Bereich, öffentlich recherchieren und dort ablegen. Übernehmen: Hierarchie, Abläufe, Dichte, Zustände, Bewertungs-Darstellung, Tastenbedienung. Nicht übernehmen: Marke, Farben, Texte, Layout eins zu eins. Im Bericht nennen, was übernommen wurde und was bewusst nicht.

## Kit und Ratsche
- **Bausteine statt Eigenbau:** Jede Runde zieht den Bereich weiter auf `src/lib/ui/` um. Fehlt ein Baustein oder eine Variante, kommt sie ins Kit und in den Katalog (`src/lib/ui/Kit.svelte`).
- **Eine Überschrift:** Titel, Zeile und Bereiche gehören in den `PageHeader` (`usePageHead`); nie ein eigenes `<h1>` oder Intro-Block.
- **Ratsche:** `node scripts/check-ui.mjs` (Teil von `pnpm check`) zählt feste Farben, freie Schriftgrößen, eigene Knopf- und `kbd`-Stile, freie Bewegung, eigene Seitenüberschriften und lokale Regeln auf globalen Klassen; mehr als die Baseline ist rot. Am Ende jedes Loops `pnpm ui:ratchet` und die gesunkene `scripts/ui-baseline.json` committen. Funde: `--list`.
- **Kontrast:** `scripts/check-contrast.mjs` misst die Raster-Beschriftung über allen Aktionsfarben; neue Rasterfarben nur als Token.

## Bilder
`.claude/skills/ringside-design-loop/shots.sh <ordner> name='?query' …` macht je Ansicht 980×700, 1440×900 und 2400×1300 in doppelter Auflösung aus dem Browser-Demo (startet und beendet Vite selbst; Optionen im Skriptkopf). Ablage `/tmp/ringside-shots/<bereich>/r<n>/`; selbst ansehen, bevor Prüfer sie bekommen.
- **Ansichten:** `?tab=<seite>` (Seiten in `src/lib/nav/pages.ts`), Overlay `?view=overlay`, klassisch `&mode=classic&preset=klartext`; Zustände `&locked=1` (PokerStars läuft), `&nohistory=1` (erster Start), `&sub=luck`, `&data=few|pending|empty`. Nicht per URL erreichbare Zustände bekommen einen kleinen Vorschau-Parameter, der nur im Browser-Demo wirkt.
- **Vergleichsbild:** `?view=kit` oder `?view=kit&only=<abschnitt>` (`seitenkopf`, `grundlagen`, `bewegung`, `knoepfe`, `auswahl`, `flaechen`, `schilder`, `bewertung`, `karten`, `tisch`, `raster`, `popups`, `zustaende`); `&gray=1` Graustufen, `&zoom=<n>`.
- Je Runde 2–8 Bilder plus Vergleichsbild. Dazu gehören:
  - **Zustände:** Hover und Tooltips (auch am Fensterrand), Tastaturfokus, Popups, Dialoge, leer, lädt, Fehler, gesperrt. Für Hover ein kleines Skript in `/tmp` mit dem lokalen ChromeDriver (`/tmp/chromedriver/chromedriver/unz/chromedriver-mac-arm64/chromedriver --port=<frei>`).
  - **Abläufe** als nummerierte Bildfolge (eine Hand, ein Solve, eine Analyse): in jedem Bild muss klar sein, was passiert ist, wer dran ist und worauf sich eine Bewertung bezieht.
  - **Bewegung:** nur Motion-Tokens aus `src/lib/theme.css` (`cubic-bezier`, nie `linear`/`ease`), `prefers-reduced-motion` schaltet ab; Zwischenstände (0 %, 50 %, 100 %) als Bildfolge.
  - **Randfälle:** lange Namen, große Zahlen („1.234.567 BB“), leere und sehr lange Listen, schmale und breite Fenster.
- **Winzige Details zählen:** Chips und Einsätze auf der Linie Platz → Tischmitte, gleich weit für alle, nie außerhalb des Filzes oder über Platz, Board und Pot; Aktions-Schilder einheitlich; Zahlenformat, Ausrichtung, Abstände einheitlich. Tische mit 6 und 9 Spielern in allen Breiten nachmessen (`e2e/table-geometry.mjs`).
- Selbst prüfen: kein horizontales Scrollen, nichts abgeschnitten, Popups im Fenster, bei 2400 px kein verlorener Leerraum.

## Ablauf je Runde
1. **Bilder** wie oben.
2. **Prüfer** (nur Analyse, gleiche Bilder; jede Runde frische Aufrufe, nie einen alten Prüfer weiterfragen):
   - **Stufe 1:** nur GPT-6.1 Sol – T3 `delegate_task` mit `target: { providerInstanceId: "codex", model: "gpt-6.1-sol", options: { reasoningEffort: "high" } }`, `role: "review"`, `mode: "wait"` – bis Sol in **allen 10 Kriterien ≥ 8** gibt.
   - **Stufe 2:** zusätzlich ein frischer Opus (Agent-Tool, `general-purpose`, ohne Kontext außer dem Prüfer-Auftrag).
   - **Stufe 3:** zusätzlich GPT-6.1 Terra (`model: "gpt-6.1-terra"`, sonst wie Sol).
   - **Stufe 4 – Fable 5.2** als finaler Geschmacks-Prüfer, genau zweimal je Zielstufe: (a) kurz vor der Zielstufe, (b) beim Erreichen. `delegate_task` mit `target: { providerInstanceId: "claude_vibeproxy", model: "claude-fable-5-2", options: { effort: "high" } }`, gleicher Auftrag. Fables konkrete Punkte zählen bei Geschmack und Feinschliff wie eine Mehrheit. **Plateau:** bewegen sich die Noten drei Runden kaum und hängen Restpunkte nur an festen Entscheiden, darf (a) früher kommen; danach umsetzen und abschließen, (b) entfällt.
3. **Auswerten:** Was die Mehrheit konkret nennt (Stufe 1: was Sol nennt) oder wo eine Note deutlich unter dem Rest liegt, zuerst. Einzelstimmen nur, wenn billig und regelkonform. Widerspricht ein Vorschlag einem festen Entscheid oder einer früheren Mehrheit: nicht umsetzen, ENTSCHEID-Zeile.
4. **Fix → neue Bilder → neue Runde.** Kleine CSS-Fixes ohne eigene Runde. Ziel in Stufen: alle Prüfer der Stufe 3 in allen Kriterien ≥ 9,0, dann ≥ 9,5, dann 10. Keine Runden- oder Zeitgrenze. Die Plateau-Regel gilt je Stufe.

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

**Verboten im Prüfer-Auftrag:** Ziel oder Zielnote, „wir haben gebaut/verbessert“, vorherige Noten oder Runden, Vorher-Bilder als „alt“, Hinweise auf gewünschte Ergebnisse, die festen Entscheide.

## Abschluss
- Lokaler E2E gegen das gebaute Browser-Demo (`pnpm build`, dann `pnpm exec vite preview --port <port>` und ChromeDriver auf freien Ports, danach beenden): `E2E_URL=http://localhost:<port>/ E2E_WEBDRIVER=http://localhost:<port> node e2e/run.mjs`. Gegen `vite dev` verweigert der Lauf den Start. Geänderte Texte oder Selektoren, die `e2e/run.mjs` nutzt, im selben Branch nachziehen.
- Grün vor jedem Push: `pnpm check` (0 Fehler, 0 Warnungen, Baseline gesenkt) und `pnpm build`; bei Rust-Änderungen zusätzlich `cargo clippy --workspace --all-targets -- -D warnings` und `cargo test --workspace`. Volle Logs in Dateien, rote Stellen lesen.
- Bildskripte und Screenshots bleiben in `/tmp`.
- Bericht an den Koordinator (Deutsch, knapp): Branch und letzter Commit, Noten je Runde und Prüfer als Tabelle, wichtigste Änderungen mit Vorher/Nachher-Bildpfaden, ENTSCHEID-Zeilen, was offen bleibt.
