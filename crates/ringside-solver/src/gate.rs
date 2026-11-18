//! One solve at a time across the app, in order of what the user waits for.
//!
//! Every background calculation (hand analysis, the practice stock, the library, the
//! community queue) and the solver page take a [`Ticket`] before they solve and drop it
//! afterwards. The gate hands the solve to the waiting ticket of the highest [`Priority`],
//! the oldest first among equals. A running solve of a lower priority sees
//! [`Ticket::should_yield`] turn true as soon as a more urgent ticket waits, stops within
//! an iteration and queues again; equal priorities never interrupt each other.
//!
//! Two trees at once would need the memory of both and halve the speed of each, so the
//! gate also keeps the memory budgets of the callers from adding up.

use std::collections::BTreeSet;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// Who waits for a solve, most urgent first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// On screen now: a hand opened in the replayer, a practice hand that waits, the
    /// solver page.
    Viewed,
    /// The user's new hands (the automatic analysis after PokerStars closed).
    NewHands,
    /// The stock of practice spots, and spots solved ahead of a practice hand.
    Practice,
    /// The rest of the user's history (the bulk analysis).
    History,
    /// The spot library and spots other users need ("Mitrechnen").
    Community,
}

#[derive(Default)]
struct State {
    next: u64,
    waiting: BTreeSet<(Priority, u64)>,
    holder: Option<(Priority, u64)>,
}

#[derive(Default)]
pub struct Gate {
    state: Mutex<State>,
    changed: Condvar,
}

/// A place in the gate's queue; dropping it gives the place or the solve up.
pub struct Ticket<'g> {
    gate: &'g Gate,
    id: (Priority, u64),
}

impl Gate {
    pub fn new() -> Gate {
        Gate::default()
    }

    /// Queues behind every ticket of the same or a higher priority.
    pub fn enter(&self, priority: Priority) -> Ticket<'_> {
        let mut s = self.state.lock().unwrap();
        let id = (priority, s.next);
        s.next += 1;
        s.waiting.insert(id);
        drop(s);
        self.changed.notify_all();
        Ticket { gate: self, id }
    }

    /// The priority of the solve running now.
    pub fn running(&self) -> Option<Priority> {
        self.state.lock().unwrap().holder.map(|(p, _)| p)
    }

    /// How many tickets wait.
    pub fn waiting(&self) -> usize {
        self.state.lock().unwrap().waiting.len()
    }
}

impl Ticket<'_> {
    pub fn priority(&self) -> Priority {
        self.id.0
    }

    /// Takes the solve if it is this ticket's turn; true while it holds it.
    pub fn try_hold(&self) -> bool {
        let mut s = self.gate.state.lock().unwrap();
        Self::take(&mut s, self.id)
    }

    fn take(s: &mut State, id: (Priority, u64)) -> bool {
        if s.holder == Some(id) {
            return true;
        }
        if s.holder.is_none() && s.waiting.first() == Some(&id) {
            s.waiting.remove(&id);
            s.holder = Some(id);
            return true;
        }
        false
    }

    /// Waits up to `timeout` for the turn; true once it holds the solve.
    pub fn hold(&self, timeout: Duration) -> bool {
        let end = Instant::now() + timeout;
        let mut s = self.gate.state.lock().unwrap();
        loop {
            if Self::take(&mut s, self.id) {
                return true;
            }
            let now = Instant::now();
            if now >= end {
                return false;
            }
            s = self.gate.changed.wait_timeout(s, end - now).unwrap().0;
        }
    }

    /// True while this ticket solves and a more urgent one waits.
    pub fn should_yield(&self) -> bool {
        let s = self.gate.state.lock().unwrap();
        s.holder == Some(self.id) && s.waiting.first().is_some_and(|w| w.0 < self.id.0)
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        let mut s = self.gate.state.lock().unwrap();
        if s.holder == Some(self.id) {
            s.holder = None;
        }
        s.waiting.remove(&self.id);
        drop(s);
        self.gate.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    const ALL: [Priority; 5] = [Priority::Viewed, Priority::NewHands, Priority::Practice, Priority::History, Priority::Community];

    #[test]
    fn the_order_is_what_the_user_waits_for() {
        assert!(Priority::Viewed < Priority::NewHands);
        assert!(Priority::NewHands < Priority::Practice);
        assert!(Priority::Practice < Priority::History);
        assert!(Priority::History < Priority::Community);
    }

    /// Property: whatever order tickets arrive in, they hold the solve sorted by priority
    /// and, among equals, in the order they came (a stable sort of the arrivals).
    #[test]
    fn tickets_are_served_by_priority_then_arrival() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..200 {
            let gate = Gate::new();
            let n = 1 + (next() % 12) as usize;
            let arrivals: Vec<Priority> = (0..n).map(|_| ALL[(next() % 5) as usize]).collect();
            let mut tickets: Vec<(usize, Ticket)> = arrivals.iter().enumerate().map(|(i, &p)| (i, gate.enter(p))).collect();
            let mut served = Vec::new();
            while !tickets.is_empty() {
                let holders: Vec<usize> = tickets.iter().enumerate().filter(|(_, (_, t))| t.try_hold()).map(|(k, _)| k).collect();
                assert_eq!(holders.len(), 1, "exactly one ticket solves at a time");
                let (i, _) = tickets.remove(holders[0]);
                served.push(i);
            }
            let mut expected: Vec<usize> = (0..n).collect();
            expected.sort_by_key(|&i| arrivals[i]);
            assert_eq!(served, expected, "arrivals {arrivals:?}");
        }
    }

    #[test]
    fn only_a_more_urgent_ticket_interrupts() {
        let gate = Gate::new();
        let stock = gate.enter(Priority::Practice);
        assert!(stock.try_hold());
        let library = gate.enter(Priority::Community);
        let other_stock = gate.enter(Priority::Practice);
        assert!(!stock.should_yield(), "equal and lower priorities wait");
        let viewed = gate.enter(Priority::Viewed);
        assert!(stock.should_yield());
        assert!(!viewed.try_hold(), "the running solve stops first");
        drop(stock);
        assert!(viewed.try_hold());
        assert!(!viewed.should_yield());
        drop(viewed);
        assert!(!library.try_hold() && other_stock.try_hold());
        assert_eq!(gate.running(), Some(Priority::Practice));
        drop(other_stock);
        assert!(library.try_hold());
    }

    #[test]
    fn a_waiting_thread_gets_its_turn_when_the_solve_ends() {
        let gate = Arc::new(Gate::new());
        let held = gate.enter(Priority::Community);
        assert!(held.try_hold());
        let done = Arc::new(AtomicBool::new(false));
        let (g, d) = (gate.clone(), done.clone());
        let waiter = std::thread::spawn(move || {
            let t = g.enter(Priority::Viewed);
            let got = t.hold(Duration::from_secs(10));
            d.store(true, Ordering::SeqCst);
            got
        });
        while gate.waiting() == 0 {
            std::thread::yield_now();
        }
        assert!(held.should_yield());
        assert!(!done.load(Ordering::SeqCst));
        drop(held);
        assert!(waiter.join().unwrap());
        assert_eq!((gate.running(), gate.waiting()), (None, 0), "a dropped ticket leaves nothing behind");
    }
}
