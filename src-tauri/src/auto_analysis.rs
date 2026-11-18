//! "Neue Hände automatisch analysieren": when PokerStars closes, or the app starts while it
//! is closed, one run rates the hands that came in since the last finished run. Ringside
//! never calculates while PokerStars runs (account risk), so a run that PokerStars
//! interrupts waits and goes on after the next close; it is still the same run.
//!
//! This module is the state machine only; `analysis.rs` feeds it and does the work.
//!
//! Invariants (tested below over every short sequence of events):
//! - It may calculate only while the setting is on, PokerStars is seen closed and a run is on.
//! - A run starts only on a close of PokerStars or the first look at it (the app start),
//!   never while one is on: open/close in quick succession resumes the same run.
//! - With the setting off nothing starts, and a run that is on ends.
//! - A cancel holds until PokerStars closes the next time; an app restart does not lift it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The next close of PokerStars (or the app start while it is closed) begins a run.
    Idle,
    /// Rates the new hands; holds while PokerStars runs.
    Running,
    /// Stopped by the user; rests until PokerStars closes the next time.
    Cancelled,
}

/// What an observation changed, for the worker to act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// A run began.
    Started,
    /// PokerStars was seen opening while no run was on: the hands from now on are new.
    OpenedIdle,
    /// PokerStars closed after a cancel: the cancel no longer holds.
    CancelLifted,
}

#[derive(Clone, Debug)]
pub struct Auto {
    phase: Phase,
    /// Whether PokerStars ran at the last observation; `None` before the first.
    client: Option<bool>,
}

impl Default for Auto {
    fn default() -> Auto {
        Auto::new(false)
    }
}

impl Auto {
    /// `cancelled`: the user cancelled before the app was last closed.
    pub fn new(cancelled: bool) -> Auto {
        Auto { phase: if cancelled { Phase::Cancelled } else { Phase::Idle }, client: None }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Feeds whether PokerStars runs and whether the setting is on.
    pub fn observe(&mut self, client: bool, enabled: bool) -> Vec<Edge> {
        let before = self.client.replace(client);
        let mut edges = Vec::new();
        if client && before != Some(true) && self.phase == Phase::Idle {
            edges.push(Edge::OpenedIdle);
        }
        if !enabled {
            if self.phase == Phase::Running {
                self.phase = Phase::Idle;
            }
            return edges;
        }
        let closed = !client && before != Some(false);
        // The app start (no observation before) is no close of PokerStars: a cancel survives it.
        let real_close = closed && before == Some(true);
        match self.phase {
            Phase::Idle if closed => {
                self.phase = Phase::Running;
                edges.push(Edge::Started);
            }
            Phase::Cancelled if real_close => {
                self.phase = Phase::Running;
                edges.extend([Edge::CancelLifted, Edge::Started]);
            }
            _ => {}
        }
        edges
    }

    /// Whether a calculation step of the run may go now.
    pub fn may_compute(&self) -> bool {
        self.phase == Phase::Running && self.client == Some(false)
    }

    /// The user stopped the analysis: no run until PokerStars closes the next time.
    pub fn cancel(&mut self) {
        self.phase = Phase::Cancelled;
    }

    /// Nothing of the run is left; true when a run was on.
    pub fn finish(&mut self) -> bool {
        let was = self.phase == Phase::Running;
        if was {
            self.phase = Phase::Idle;
        }
        was
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum Ev {
        Client(bool),
        Setting(bool),
        Cancel,
        Finish,
    }

    const ALL: [Ev; 6] = [Ev::Client(true), Ev::Client(false), Ev::Setting(true), Ev::Setting(false), Ev::Cancel, Ev::Finish];

    /// Plays `events` from a fresh start (setting on) and checks the invariants after each.
    fn play(cancelled: bool, events: &[Ev]) {
        let mut auto = Auto::new(cancelled);
        let (mut client, mut enabled) = (None::<bool>, true);
        // A cancel holds until a real close; true from the start when the app was closed cancelled.
        let mut held = cancelled;
        for (i, &ev) in events.iter().enumerate() {
            let was = auto.phase();
            let edges = match ev {
                Ev::Client(c) => {
                    let before = client.replace(c);
                    let edges = auto.observe(c, enabled);
                    if !c && before == Some(true) {
                        held = false;
                    }
                    edges
                }
                Ev::Setting(on) => {
                    enabled = on;
                    client.map(|c| auto.observe(c, on)).unwrap_or_default()
                }
                Ev::Cancel => {
                    auto.cancel();
                    held = true;
                    Vec::new()
                }
                Ev::Finish => {
                    auto.finish();
                    Vec::new()
                }
            };
            let at = || format!("after {:?}", &events[..=i]);
            if auto.may_compute() {
                assert_eq!(client, Some(false), "calculates while PokerStars runs or before it was seen, {}", at());
                assert!(enabled, "calculates with the setting off, {}", at());
            }
            let started = edges.contains(&Edge::Started);
            if started {
                assert!(enabled, "started with the setting off, {}", at());
                assert_ne!(was, Phase::Running, "a second run started while one was on, {}", at());
                assert!(matches!(ev, Ev::Client(false)), "started without a close, {}", at());
                assert!(!held, "started although cancelled, {}", at());
            }
            if held {
                assert_ne!(auto.phase(), Phase::Running, "a cancel did not hold, {}", at());
            }
            if !enabled {
                assert_ne!(auto.phase(), Phase::Running, "a run goes on with the setting off, {}", at());
            }
        }
    }

    #[test]
    fn invariants_hold_for_every_short_sequence() {
        fn all(depth: usize, prefix: &mut Vec<Ev>) {
            play(false, prefix);
            play(true, prefix);
            if depth == 0 {
                return;
            }
            for ev in ALL {
                prefix.push(ev);
                all(depth - 1, prefix);
                prefix.pop();
            }
        }
        all(7, &mut Vec::new());
    }

    #[test]
    fn closing_pokerstars_starts_exactly_one_run() {
        let mut auto = Auto::new(false);
        assert_eq!(auto.observe(true, true), [Edge::OpenedIdle]);
        assert!(!auto.may_compute());
        assert_eq!(auto.observe(true, true), []);
        assert_eq!(auto.observe(false, true), [Edge::Started]);
        assert!(auto.may_compute());
        // Ticks while it stays closed start nothing.
        assert_eq!(auto.observe(false, true), []);
    }

    #[test]
    fn the_app_start_with_pokerstars_closed_starts_a_run() {
        let mut auto = Auto::new(false);
        assert_eq!(auto.observe(false, true), [Edge::Started]);
        assert!(auto.may_compute());
    }

    #[test]
    fn flapping_pokerstars_pauses_and_resumes_the_same_run() {
        let mut auto = Auto::new(false);
        let mut starts = 0;
        for client in [false, true, false, true, true, false, true, false] {
            let edges = auto.observe(client, true);
            starts += edges.iter().filter(|e| **e == Edge::Started).count();
            assert!(!edges.contains(&Edge::OpenedIdle), "a paused run keeps its hands");
            assert_eq!(auto.may_compute(), !client);
        }
        assert_eq!(starts, 1);
        assert!(auto.finish());
        assert!(!auto.finish(), "finished once");
    }

    #[test]
    fn the_setting_off_starts_nothing_and_ends_a_run() {
        let mut auto = Auto::new(false);
        assert_eq!(auto.observe(false, false), []);
        assert_eq!(auto.observe(true, false), [Edge::OpenedIdle]);
        assert_eq!(auto.observe(false, false), []);
        assert!(!auto.may_compute());

        let mut auto = Auto::new(false);
        auto.observe(false, true);
        assert!(auto.may_compute());
        auto.observe(false, false);
        assert_eq!(auto.phase(), Phase::Idle);
        // Switched back on, it waits for the next close.
        assert_eq!(auto.observe(false, true), []);
        assert!(!auto.may_compute());
    }

    #[test]
    fn a_cancel_holds_until_the_next_close() {
        let mut auto = Auto::new(false);
        auto.observe(false, true);
        auto.cancel();
        assert!(!auto.may_compute());
        assert_eq!(auto.observe(false, true), []);
        assert_eq!(auto.observe(true, true), [], "opening keeps the cancelled hands for the next run");
        assert_eq!(auto.observe(false, true), [Edge::CancelLifted, Edge::Started]);
        assert!(auto.may_compute());
    }

    #[test]
    fn a_cancel_survives_an_app_restart() {
        let mut auto = Auto::new(true);
        assert_eq!(auto.observe(false, true), []);
        assert!(!auto.may_compute());
        auto.observe(true, true);
        assert_eq!(auto.observe(false, true), [Edge::CancelLifted, Edge::Started]);
    }
}
