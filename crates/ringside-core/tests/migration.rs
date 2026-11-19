use ringside_core::db::Db;

/// Ratings stored with five levels would read one level off on the six-level scale:
/// opening an older database drops them once, so they are analysed again.
#[test]
fn five_level_ratings_are_dropped_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE hero_decisions (
                 hand_id INTEGER NOT NULL, action_index INTEGER NOT NULL, street TEXT NOT NULL, spot TEXT NOT NULL,
                 status TEXT NOT NULL, played TEXT NOT NULL, best TEXT, strategy TEXT NOT NULL DEFAULT '[]',
                 played_freq REAL, ev_loss_bb REAL, rating INTEGER, source TEXT, why TEXT NOT NULL DEFAULT '',
                 note TEXT NOT NULL DEFAULT '', reason TEXT, solver TEXT, engine TEXT NOT NULL,
                 PRIMARY KEY (hand_id, action_index));
             INSERT INTO hero_decisions (hand_id, action_index, street, spot, status, played, rating, engine)
                 VALUES (1, 4, 'preflop', 'open', 'rated', 'Fold', 2, 'old');",
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert!(db.hand_decisions(1).unwrap().is_empty());
    drop(db);

    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO hero_decisions (hand_id, action_index, street, spot, status, played, rating, engine, pot_bb, reliability)
         VALUES (1, 4, 'preflop', 'open', 'rated', 'Fold', 3, 'new', 2.5, NULL)",
        [],
    )
    .unwrap();
    drop(conn);
    let db = Db::open(&path).unwrap();
    assert_eq!(db.hand_decisions(1).unwrap().len(), 1, "a second start keeps what was analysed since");
}

/// Hands stored before GGPoker support came from PokerStars and keep counting as such.
#[test]
fn hands_stored_before_ggpoker_support_are_pokerstars_hands() {
    use ringside_core::db::HandQuery;
    use ringside_core::model::Site;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE hands (id INTEGER PRIMARY KEY, game TEXT NOT NULL, tournament_id INTEGER, table_key TEXT NOT NULL,
                 table_name TEXT NOT NULL, played_at TEXT NOT NULL, small_blind INTEGER NOT NULL, big_blind INTEGER NOT NULL,
                 currency TEXT, hero TEXT, hero_cards TEXT NOT NULL DEFAULT '', hero_net INTEGER, raw TEXT NOT NULL);
             INSERT INTO hands (id, game, table_key, table_name, played_at, small_blind, big_blind, hero, hero_net, raw)
                 VALUES (260000000001, 'cash', 'c:Aquila', 'Aquila', '2026/11/01 20:00:00', 5, 10, 'Hero', 5, '');",
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(db.hands_per_site().unwrap(), [(Site::PokerStars, 1)]);
    let listed = db.hands(&HandQuery { site: Some(Site::PokerStars), limit: 10, ..Default::default() }).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].site, Site::PokerStars);
}
