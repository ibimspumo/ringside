//! What the Spielwertung reads: every rated decision of the hero with its hand's stack,
//! tournament and time. The rating itself is computed in [`crate::skill`], from scratch
//! each time, so it always matches the stored analysis.

use super::{Db, Error};
use crate::db::{Rating, Source, SpotType, Status};
use crate::skill::Move;

impl Db {
    /// Whether the hero played cash hands on a day from `start` to `end`: a cash day is one
    /// unit of the rating and may reach over a session's edge.
    pub fn cash_on_days(&self, start: &str, end: &str) -> Result<bool, Error> {
        let day = |t: &str| t.get(..10).unwrap_or(t).to_string();
        self.conn.prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM hands WHERE played_at >= ?1 AND played_at < ?2 || '~'
                            AND tournament_id IS NULL AND hero IS NOT NULL)",
        )?
        .query_row(rusqlite::params![day(start), day(end)], |r| r.get(0))
    }

    /// The hero's rated decisions of every hand, in hand order.
    pub fn skill_moves(&self) -> Result<Vec<Move>, Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT d.hand_id, d.action_index, h.tournament_id, h.played_at, d.spot, d.status, d.rating, d.source,
                    d.reliability, d.ev_loss_bb, d.pot_bb, h.hero_chips_bb, h.hero_stack_bb, h.hero_position,
                    CAST(h.hero_net AS REAL) / MAX(h.big_blind, 1), h.hero_cards, d.played, d.best
             FROM hero_decisions d JOIN hands h ON h.id = d.hand_id
             WHERE d.status = 'rated' AND h.hero IS NOT NULL
             ORDER BY d.hand_id, d.action_index",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Move {
                hand_id: r.get::<_, i64>(0)? as u64,
                action_index: r.get::<_, i64>(1)? as usize,
                tournament_id: r.get::<_, Option<i64>>(2)?.map(|t| t as u64),
                played_at: r.get(3)?,
                spot: SpotType::parse(&r.get::<_, String>(4)?).unwrap_or(SpotType::Flop),
                status: Status::parse(&r.get::<_, String>(5)?),
                rating: r.get::<_, Option<i64>>(6)?.and_then(Rating::from_level),
                source: r.get::<_, Option<String>>(7)?.as_deref().and_then(Source::parse),
                reliability: r.get(8)?,
                ev_loss_bb: r.get(9)?,
                pot_bb: r.get(10)?,
                chips_bb: r.get(11)?,
                stack_bb: r.get(12)?,
                position: r.get(13)?,
                hero_net_bb: r.get(14)?,
                hero_cards: r.get(15)?,
                played: r.get(16)?,
                best: r.get(17)?,
            })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::db::{Choice, Decision, HandAnalysis};
    use crate::model::Street;
    use crate::{Db, parse_hand, split_hands};

    use super::*;

    const KNOCKOUT: &str = include_str!("../../tests/fixtures/knockout.txt");

    fn decision(hand_id: u64, action_index: usize, status: Status, loss: Option<f64>) -> Decision {
        Decision {
            hand_id,
            action_index,
            street: Street::Preflop,
            spot: SpotType::CallShove,
            status,
            played: "Call".into(),
            best: Some("Fold".into()),
            strategy: vec![Choice { label: "Fold".into(), freq: 1.0, ev_bb: Some(0.0), played: false, kind: "fold".into() }],
            played_freq: Some(0.0),
            ev_loss_bb: loss,
            rating: loss.map(|_| Rating::Blunder),
            source: Some(Source::Equilibrium),
            why: String::new(),
            note: String::new(),
            reason: None,
            solver: None,
            engine: "test".into(),
            pot_bb: Some(24.0),
            reliability: None,
        }
    }

    #[test]
    fn the_rating_reads_rated_decisions_with_the_own_stack() {
        let mut db = Db::open_in_memory().unwrap();
        let text = split_hands(KNOCKOUT)[0].to_string();
        let hand = parse_hand(&text).unwrap();
        let (id, tid) = (hand.id, hand.tournament_id);
        let hero = hand.hero.clone().unwrap();
        let own = crate::db::own_stack_bb(&hand, &hero);
        db.insert_hands(&[(hand, text)]).unwrap();
        let decisions = vec![decision(id, 8, Status::Rated, Some(3.0)), decision(id, 9, Status::Pending, None)];
        db.save_analysis(&[HandAnalysis { hand_id: id, engine: "test".into(), decisions, job: None }]).unwrap();

        let moves = db.skill_moves().unwrap();
        assert_eq!(moves.len(), 1, "only rated decisions count");
        let m = &moves[0];
        assert_eq!((m.hand_id, m.action_index, m.tournament_id), (id, 8, tid));
        assert!(own.is_some() && m.chips_bb == own);
        assert_eq!(m.rating, Some(Rating::Blunder));
        assert!(crate::skill::profile(&moves).rating.is_some());

        let query = |area| crate::db::HandQuery { area: Some(area), limit: 10, ..Default::default() };
        let hands = db.hands(&query(crate::skill::Area::VsAllIn)).unwrap();
        assert_eq!(hands.iter().map(|h| h.id).collect::<Vec<_>>(), [id]);
        assert_eq!(hands[0].worst.as_ref().map(|w| w.action_index), Some(8));
        assert!(db.hands(&query(crate::skill::Area::River)).unwrap().is_empty());
    }
}
