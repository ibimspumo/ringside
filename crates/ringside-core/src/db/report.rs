//! What the tournament report reads: the hands of one tournament, their stored decisions
//! and the scores of all tournaments for the comparison. The report itself is computed in
//! [`crate::report`].

use rusqlite::params;

use super::{Db, Error, FILTER_SQL, Filter, hand_range};
use crate::db::{Choice, Rating, Source, SpotType, Status};
use crate::parser::parse_hand;
use crate::profile::{ProfileRow, facing_bet};
use crate::report::{PastScore, Report, ReportDecision, ReportHand, report};

use super::analysis::parse_street;

impl Db {
    /// The hands of `filter` (a tournament or a session) in play order, and how many of them
    /// are not analysed with `engine` yet.
    pub fn report_hands(&self, filter: &Filter, engine: &str) -> Result<(Vec<ReportHand>, usize), Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT h.id, h.played_at, h.hero_cards, h.hero_class, h.hero_position, h.hero_net, h.big_blind, h.hero_ev,
                    h.raw, h.hero, COALESCE(a.engine = ?5, 0)
             FROM hands h LEFT JOIN hand_analysis a ON a.hand_id = h.id
             WHERE {FILTER_SQL} AND {range} AND h.hero_net IS NOT NULL ORDER BY h.played_at, h.id",
            range = hand_range("h.id")
        ))?;
        let mut unanalyzed = 0;
        let mut hands = Vec::new();
        let mut rows = stmt.query(params![filter.game(), filter.since, filter.tournament(), filter.until, engine])?;
        while let Some(r) = rows.next()? {
            let raw: String = r.get(8)?;
            let hero: Option<String> = r.get(9)?;
            // The stack is not a column; one tournament is a few hundred hands to parse.
            let stack = hero.and_then(|name| {
                let hand = parse_hand(&raw).ok()?;
                hand.seats.iter().find(|s| s.name == name).map(|s| s.stack)
            });
            unanalyzed += (r.get::<_, i64>(10)? == 0) as usize;
            hands.push(ReportHand {
                hand_id: r.get::<_, i64>(0)? as u64,
                played_at: r.get(1)?,
                hero_cards: r.get(2)?,
                class: r.get(3)?,
                position: r.get(4)?,
                net: r.get(5)?,
                big_blind: r.get(6)?,
                ev: r.get(7)?,
                stack,
            });
        }
        Ok((hands, unanalyzed))
    }

    /// Every unforced decision of the hero in the hands of `filter`, in play order.
    pub fn report_decisions(&self, filter: &Filter) -> Result<Vec<ReportDecision>, Error> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT d.hand_id, d.action_index, h.played_at, d.spot, d.status, d.rating, d.ev_loss_bb, d.source, d.reliability,
                    d.strategy, h.hero_position, h.hero_stack_bb,
                    (SELECT COUNT(*) FROM player_hands p WHERE p.hand_id = h.id),
                    d.street, d.played, d.best, d.why, d.pot_bb
             FROM hero_decisions d JOIN hands h ON h.id = d.hand_id
             WHERE {FILTER_SQL} AND {range} AND d.status <> 'forced'
             ORDER BY h.played_at, d.hand_id, d.action_index",
            range = hand_range("d.hand_id")
        ))?;
        let rows = stmt.query_map(params![filter.game(), filter.since, filter.tournament(), filter.until], |r| {
            let strategy: Vec<Choice> = serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default();
            Ok(ReportDecision {
                row: ProfileRow {
                    hand_id: r.get::<_, i64>(0)? as u64,
                    action_index: r.get::<_, i64>(1)? as usize,
                    played_at: r.get(2)?,
                    spot: SpotType::parse(&r.get::<_, String>(3)?).unwrap_or(SpotType::Flop),
                    status: Status::parse(&r.get::<_, String>(4)?),
                    rating: r.get::<_, Option<i64>>(5)?.and_then(Rating::from_level),
                    ev_loss_bb: r.get(6)?,
                    source: r.get::<_, Option<String>>(7)?.as_deref().and_then(Source::parse),
                    reliability: r.get(8)?,
                    facing_bet: facing_bet(&strategy),
                    position: r.get(10)?,
                    stack_bb: r.get(11)?,
                    players: r.get::<_, i64>(12)? as usize,
                },
                street: parse_street(&r.get::<_, String>(13)?),
                played: r.get(14)?,
                best: r.get(15)?,
                why: r.get(16)?,
                strategy,
                pot_bb: r.get(17)?,
            })
        })?;
        rows.collect()
    }

    /// Rated decisions summed per tournament, for the comparison with earlier ones.
    pub fn past_scores(&self) -> Result<Vec<PastScore>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT h.tournament_id, COALESCE(MAX(t.started_at), MIN(h.played_at)), MAX(t.name), COUNT(*),
                    COALESCE(SUM(d.rating <= ?1), 0), COALESCE(SUM(d.ev_loss_bb), 0)
             FROM hero_decisions d JOIN hands h ON h.id = d.hand_id LEFT JOIN tournaments t ON t.id = h.tournament_id
             WHERE h.tournament_id IS NOT NULL AND d.status = 'rated'
             GROUP BY h.tournament_id",
        )?;
        let rows = stmt.query_map([Rating::Good.level()], |r| {
            Ok(PastScore {
                tournament_id: r.get::<_, i64>(0)? as u64,
                started_at: r.get(1)?,
                name: r.get(2)?,
                decisions: r.get::<_, i64>(3)? as usize,
                good: r.get::<_, i64>(4)? as usize,
                loss_bb: r.get(5)?,
            })
        })?;
        rows.collect()
    }

    /// The report of a tournament from what is stored; `started_at` orders it among the
    /// others for the comparison.
    pub fn tournament_report(&self, tournament_id: u64, started_at: &str, engine: &str) -> Result<Report, Error> {
        let filter = Filter { tournament_id: Some(tournament_id), ..Default::default() };
        let (hands, unanalyzed) = self.report_hands(&filter, engine)?;
        let decisions = self.report_decisions(&filter)?;
        let past: Vec<PastScore> = self.past_scores()?.into_iter().filter(|p| p.tournament_id != tournament_id).collect();
        Ok(report(&hands, &decisions, unanalyzed, started_at, &past))
    }
}

#[cfg(test)]
mod tests {
    use crate::Db;

    #[test]
    fn an_unknown_tournament_has_an_empty_report() {
        let db = Db::open_in_memory().unwrap();
        let r = db.tournament_report(42, "2026/11/18 20:00:00", "e1").unwrap();
        assert_eq!((r.hands, r.score.decisions, r.worst.len()), (0, 0, 0));
    }
}
