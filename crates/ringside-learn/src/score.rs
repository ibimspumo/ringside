//! Tolerance rules shared by the graders.

/// Score of an estimate that is off by `err`: 1 within `full`, falling linearly to 0
/// at `zero`.
pub fn band(err: f64, full: f64, zero: f64) -> f64 {
    let err = err.abs();
    if err <= full {
        1.0
    } else if err >= zero {
        0.0
    } else {
        (zero - err) / (zero - full)
    }
}

/// Partial credit for a wrong exact answer: at most 0.5, falling to 0 once the answer
/// is off by `scale`.
pub fn partial(err: f64, scale: f64) -> f64 {
    0.5 * (1.0 - err.abs() / scale.max(1e-9)).max(0.0)
}

/// Score of a wrong decision that costs `ev_loss_bb`: 0.5 for a negligible loss,
/// 0 from half a big blind on.
pub fn decision(ev_loss_bb: f64) -> f64 {
    0.5 * (1.0 - ev_loss_bb / 0.5).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands() {
        assert_eq!(band(1.5, 2.0, 10.0), 1.0);
        assert_eq!(band(-6.0, 2.0, 10.0), 0.5);
        assert_eq!(band(12.0, 2.0, 10.0), 0.0);
        assert_eq!(partial(0.0, 4.0), 0.5);
        assert_eq!(partial(8.0, 4.0), 0.0);
        assert_eq!(decision(0.0), 0.5);
        assert_eq!(decision(0.25), 0.25);
        assert_eq!(decision(3.0), 0.0);
    }
}
