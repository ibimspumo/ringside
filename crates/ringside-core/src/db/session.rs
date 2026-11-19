//! What the session review reads: the hero's sessions and, for one of them, its hands,
//! decisions and preflop facts. The review itself is computed in [`crate::session`].

use rusqlite::params;

use super::ranges::PREFLOP_FILTER_SQL;
use super::{Db, Error, FILTER_SQL, Filter, MINUTES_SQL};
use crate::allin::hero_all_in;
use crate::parser::parse_hand;
use crate::model::Site;
use crate::ranges::History;
use crate::report::report;
use crate::session::{PreflopFact, SessionReview, SessionSpan, Swing, SwingHand, preflop_review, split, swings};

impl Db {
    /// The hero's sessions, newest first, at most `limit`.
    pub fn session_spans(&self, limit: usize) -> Result<Vec<SessionSpan>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT h.played_at, {MINUTES_SQL}, h.tournament_id, h.site FROM hands h
             WHERE h.hero_net IS NOT NULL ORDER BY h.played_at, h.id"
        ))?;
        let rows: Vec<(String, f64, Option<u64>, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    r.get::<_, Option<i64>>(2)?.map(|t| t as u64),
                    r.get(3)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        let index = split(&rows.iter().map(|r| (r.1, r.2)).collect::<Vec<_>>());
        let mut spans: Vec<SessionSpan> = Vec::new();
        for (r, i) in rows.into_iter().zip(index) {
            if i == spans.len() {
                spans.push(SessionSpan { start: r.0.clone(), ..Default::default() });
            }
            let s = &mut spans[i];
            s.end = r.0;
            s.hands += 1;
            if let Some(site) = Site::parse(&r.3)
                && !s.sites.contains(&site)
            {
                s.sites.push(site);
                s.sites.sort_by_key(|s| *s != Site::PokerStars);
            }
            match r.2 {
                Some(t) if !s.tournaments.contains(&t) => s.tournaments.push(t),
                Some(_) => {}
                None => s.cash_hands += 1,
            }
        }
        spans.reverse();
        spans.truncate(limit);
        Ok(spans)
    }

    fn preflop_facts(&self, filter: &Filter) -> Result<Vec<PreflopFact>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT p.hand_id, p.action_index, p.class, p.position, p.vs_position, p.history
             FROM hero_preflop p WHERE {PREFLOP_FILTER_SQL} ORDER BY p.hand_id, p.action_index"
        ))?;
        let rows = stmt.query_map(params![filter.game(), filter.since, filter.tournament(), filter.until], |r| {
            Ok((
                PreflopFact {
                    hand_id: r.get::<_, i64>(0)? as u64,
                    action_index: r.get::<_, i64>(1)? as usize,
                    class: r.get(2)?,
                    position: r.get(3)?,
                    vs_position: r.get(4)?,
                    history: History::Unopened,
                },
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (mut f, history) = row?;
            if let Some(h) = History::parse(&history) {
                f.history = h;
                out.push(f);
            }
        }
        Ok(out)
    }

    fn swing_hands(&self, filter: &Filter) -> Result<Vec<SwingHand>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT h.id, h.tournament_id, h.hero_cards, h.hero_position, h.hero_net, h.hero_ev, h.big_blind, h.hero_showdown
             FROM hands h WHERE {FILTER_SQL} AND h.hero_net IS NOT NULL ORDER BY h.played_at, h.id"
        ))?;
        let rows = stmt.query_map(params![filter.game(), filter.since, filter.tournament(), filter.until], |r| {
            Ok(SwingHand {
                hand_id: r.get::<_, i64>(0)? as u64,
                tournament_id: r.get::<_, Option<i64>>(1)?.map(|t| t as u64),
                hero_cards: r.get(2)?,
                position: r.get(3)?,
                net: r.get(4)?,
                ev: r.get(5)?,
                big_blind: r.get(6)?,
                showdown: r.get::<_, i64>(7)? != 0,
            })
        })?;
        rows.collect()
    }

    /// Cards shown and the hero's share once the money was in, for an all-in example.
    fn add_showdown(&self, s: &mut Swing) -> Result<(), Error> {
        let raw: String = self.conn.query_row("SELECT raw FROM hands WHERE id = ?1", [s.hand_id as i64], |r| r.get(0))?;
        if let Some(a) = parse_hand(&raw).ok().as_ref().and_then(hero_all_in) {
            s.equity_pct = Some(a.share_pct);
            s.opponents = a.opponents.iter().map(|h| h.iter().map(ToString::to_string).collect()).collect();
            s.board = a.board.iter().map(ToString::to_string).collect();
        }
        Ok(())
    }

    /// The review of one session from what is stored.
    pub fn session_review(&self, span: &SessionSpan, engine: &str) -> Result<SessionReview, Error> {
        let filter = Filter { since: Some(span.start.clone()), until: Some(span.end.clone()), ..Default::default() };
        let (hands, unanalyzed) = self.report_hands(&filter, engine)?;
        let decisions = self.report_decisions(&filter)?;
        let preflop = preflop_review(&self.preflop_facts(&filter)?, &decisions);
        let (mut bad_luck, mut good_luck) = swings(&self.swing_hands(&filter)?, &decisions);
        for s in bad_luck.iter_mut().chain(good_luck.iter_mut()).filter(|s| s.ev_bb.is_some()) {
            self.add_showdown(s)?;
        }
        let report = report(&hands, &decisions, unanalyzed, &span.start, &[]);
        Ok(SessionReview { span: span.clone(), report, preflop, bad_luck, good_luck, locked: false })
    }
}
