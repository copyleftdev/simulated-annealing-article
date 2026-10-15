//! A reproducible synthetic optimization experiment, not a service benchmark.

use std::f64::consts::TAU;

pub const SIDE: u32 = 64;
pub const BUDGET: usize = 256;
pub const SEEDS: u64 = 30;
pub const INITIAL: Point = Point { i: 63, j: 63 };

/// xorshift64*: deterministic, non-cryptographic, with a forbidden zero state.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Result<Self, String> {
        if seed == 0 {
            return Err("the random seed must be nonzero".into());
        }
        Ok(Self(seed))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(2_685_821_657_736_338_717)
    }

    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Rejection sampling avoids modulo bias for arbitrary positive bounds.
    pub fn below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0);
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return value % bound;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Schedule {
    pub start: f64,
    pub end: f64,
    pub evaluations: usize,
}

impl Schedule {
    pub fn validate(self) -> Result<(), String> {
        if !self.start.is_finite()
            || !self.end.is_finite()
            || self.end <= 0.0
            || self.start < self.end
            || self.evaluations < 3
        {
            return Err(
                "schedule requires finite start >= end > 0 and at least 3 evaluations".into(),
            );
        }
        Ok(())
    }

    pub fn temperature(self, proposal: usize) -> f64 {
        let fraction = proposal as f64 / (self.evaluations - 2) as f64;
        ((1.0 - fraction) * self.start.ln() + fraction * self.end.ln()).exp()
    }
}

pub const SCHEDULE: Schedule = Schedule {
    start: 20.0,
    end: 0.1,
    evaluations: BUDGET,
};

#[derive(Clone, Debug)]
pub struct Outcome<S> {
    pub best: S,
    pub best_score: f64,
    pub evaluations: usize,
    pub accepted_uphill: usize,
}

fn finite(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err("objective returned a non-finite score".into())
    }
}

// ARTICLE_SNIPPET_START
/// Minimize an objective while retaining the best state ever visited.
pub fn anneal<S: Clone>(
    initial: S,
    mut score: impl FnMut(&S) -> f64,
    mut neighbor: impl FnMut(&S, &mut Rng) -> S,
    seed: u64,
    schedule: Schedule,
) -> Result<Outcome<S>, String> {
    schedule.validate()?;
    let mut rng = Rng::new(seed)?;
    let mut current = initial;
    let mut current_score = finite(score(&current))?;
    let mut result = Outcome {
        best: current.clone(),
        best_score: current_score,
        evaluations: 1,
        accepted_uphill: 0,
    };

    for proposal in 0..schedule.evaluations - 1 {
        let candidate = neighbor(&current, &mut rng);
        let candidate_score = finite(score(&candidate))?;
        result.evaluations += 1;
        let delta = finite(candidate_score - current_score)?;
        let temperature = schedule.temperature(proposal);
        if delta <= 0.0 || rng.unit() < (-delta / temperature).exp() {
            result.accepted_uphill += usize::from(delta > 0.0);
            current = candidate;
            current_score = candidate_score;
            if current_score < result.best_score {
                result.best = current.clone();
                result.best_score = current_score;
            }
        }
    }
    Ok(result)
}
// ARTICLE_SNIPPET_END

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub i: u32,
    pub j: u32,
}

pub fn coordinate(index: u32) -> f64 {
    assert!(index < SIDE);
    -5.12 + 10.24 * f64::from(index) / f64::from(SIDE - 1)
}

pub fn landscape(x: f64, y: f64) -> f64 {
    x * x + y * y + 20.0 - 10.0 * (TAU * x).cos() - 10.0 * (TAU * y).cos()
}

pub fn training_score(point: &Point) -> f64 {
    landscape(coordinate(point.i), coordinate(point.j))
}

/// A synthetic perturbation check, not evidence of production generalization.
/// Called only after a search has selected its best training point.
pub fn held_out_score(point: &Point) -> f64 {
    let x = coordinate(point.i);
    let y = coordinate(point.j);
    let mut total = 0.0;
    for dx in [-0.1, 0.1] {
        for dy in [-0.1, 0.1] {
            total += landscape(x + dx, y + dy);
        }
    }
    total / 4.0
}

/// Each signed axis move has the same probability as its reverse.
/// An out-of-bounds move stays put, consuming one objective evaluation.
pub fn neighbor(point: &Point, rng: &mut Rng) -> Point {
    let axis = rng.below(2);
    let draw = rng.below(16) as i32;
    let step = if draw < 8 { draw - 8 } else { draw - 7 };
    let mut proposed = *point;
    let old = if axis == 0 { point.i } else { point.j };
    let next = old as i32 + step;
    if (0..SIDE as i32).contains(&next) {
        if axis == 0 {
            proposed.i = next as u32;
        } else {
            proposed.j = next as u32;
        }
    }
    proposed
}

pub fn greedy(seed: u64) -> Result<Outcome<Point>, String> {
    let mut rng = Rng::new(seed)?;
    let mut result = Outcome {
        best: INITIAL,
        best_score: training_score(&INITIAL),
        evaluations: 1,
        accepted_uphill: 0,
    };
    for _ in 1..BUDGET {
        let candidate = neighbor(&result.best, &mut rng);
        let score = training_score(&candidate);
        result.evaluations += 1;
        if score <= result.best_score {
            result.best = candidate;
            result.best_score = score;
        }
    }
    Ok(result)
}

pub fn random_search(seed: u64) -> Result<Outcome<Point>, String> {
    let mut rng = Rng::new(seed)?;
    let candidates = std::iter::once(INITIAL).chain((1..BUDGET).map(|_| Point {
        i: rng.below(u64::from(SIDE)) as u32,
        j: rng.below(u64::from(SIDE)) as u32,
    }));
    Ok(best_of(candidates))
}

/// Sixteen evenly spaced, rounded levels per axis. INITIAL appears exactly once.
pub fn grid_search() -> Outcome<Point> {
    let levels: Vec<u32> = (0..16).map(|k| (k * 63 + 7) / 15).collect();
    let candidates = std::iter::once(INITIAL).chain(levels.iter().flat_map(|&i| {
        levels
            .iter()
            .map(move |&j| Point { i, j })
            .filter(|&point| point != INITIAL)
    }));
    best_of(candidates)
}

fn best_of(candidates: impl Iterator<Item = Point>) -> Outcome<Point> {
    let mut result = Outcome {
        best: INITIAL,
        best_score: f64::INFINITY,
        evaluations: 0,
        accepted_uphill: 0,
    };
    for candidate in candidates {
        let score = training_score(&candidate);
        result.evaluations += 1;
        if score < result.best_score {
            result.best = candidate;
            result.best_score = score;
        }
    }
    result
}

/// The 4096-call oracle is outside every search budget and never guides search.
pub fn oracle() -> Outcome<Point> {
    best_of((0..SIDE).flat_map(|i| (0..SIDE).map(move |j| Point { i, j })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn rejects_zero_seed_and_bad_schedules_before_evaluation() {
        assert!(Rng::new(0).is_err());
        for schedule in [
            Schedule {
                start: f64::NAN,
                ..SCHEDULE
            },
            Schedule {
                end: 0.0,
                ..SCHEDULE
            },
            Schedule {
                end: f64::INFINITY,
                ..SCHEDULE
            },
            Schedule {
                end: 21.0,
                ..SCHEDULE
            },
            Schedule {
                evaluations: 2,
                ..SCHEDULE
            },
        ] {
            assert!(anneal(0, |_| panic!("must validate first"), |x, _| *x, 1, schedule).is_err());
        }
        assert!(anneal(0, |_| panic!("must validate first"), |x, _| *x, 0, SCHEDULE).is_err());
    }

    #[test]
    fn rejects_nonfinite_objective_at_initial_or_candidate() {
        assert!(anneal(0, |_| f64::NAN, |x, _| *x, 1, SCHEDULE).is_err());
        assert!(anneal(
            0,
            |x| if *x == 0 { 0.0 } else { f64::INFINITY },
            |_, _| 1,
            1,
            SCHEDULE
        )
        .is_err());
    }

    #[test]
    fn rejects_overflow_in_score_difference() {
        assert!(anneal(
            0,
            |x| if *x == 0 { -f64::MAX } else { f64::MAX },
            |_, _| 1,
            1,
            SCHEDULE,
        )
        .is_err());
    }

    #[test]
    fn retains_best_after_accepting_uphill_and_counts_calls() {
        let calls = Cell::new(0);
        let result = anneal(
            0_u32,
            |x| {
                calls.set(calls.get() + 1);
                f64::from(*x)
            },
            |x, _| x + 1,
            1,
            Schedule {
                start: 1e100,
                end: 1e100,
                evaluations: 8,
            },
        )
        .unwrap();
        assert_eq!(result.best, 0);
        assert_eq!(result.best_score, 0.0);
        assert_eq!(result.accepted_uphill, 7);
        assert_eq!(result.evaluations, 8);
        assert_eq!(calls.get(), 8);
    }

    #[test]
    fn accepting_an_uphill_step_can_cross_a_tiny_barrier() {
        // A mechanism fixture, not a performance benchmark: 0 <-> 1 <-> 2.
        // Each direction has probability 1/2; leaving the domain stays put.
        // Costs are [1, 2, 0], so state 2 requires crossing an uphill edge.
        let score = |state: &u32| [1.0, 2.0, 0.0][*state as usize];
        let neighbor = |state: &u32, rng: &mut Rng| {
            let step = if rng.below(2) == 0 { -1 } else { 1 };
            let candidate = *state as i32 + step;
            if (0..=2).contains(&candidate) {
                candidate as u32
            } else {
                *state
            }
        };
        let result = anneal(
            0_u32,
            score,
            neighbor,
            1,
            Schedule {
                start: 100.0,
                end: 100.0,
                evaluations: 3,
            },
        )
        .unwrap();
        assert_eq!(result.accepted_uphill, 1);
        assert_eq!(result.best, 2);
        assert_eq!(result.best_score, 0.0);
        assert_eq!(result.evaluations, 3);

        let mut greedy_state = 0_u32;
        let mut rng = Rng::new(1).unwrap();
        for _ in 0..2 {
            let candidate = neighbor(&greedy_state, &mut rng);
            if score(&candidate) <= score(&greedy_state) {
                greedy_state = candidate;
            }
        }
        assert_eq!(greedy_state, 0);
        assert_eq!(score(&greedy_state), 1.0);
    }

    #[test]
    fn schedule_reaches_predeclared_endpoints() {
        assert!((SCHEDULE.temperature(0) - 20.0).abs() < 1e-12);
        assert!((SCHEDULE.temperature(BUDGET - 2) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn oracle_matches_separable_one_dimensional_minimum() {
        let minimum_1d = (0..SIDE)
            .map(|i| landscape(coordinate(i), 0.0))
            .fold(f64::INFINITY, f64::min);
        let result = oracle();
        assert_eq!(result.evaluations, 4096);
        assert!((result.best_score - 2.0 * minimum_1d).abs() < 1e-12);
        assert!((landscape(0.0, 0.0)).abs() < 1e-12);
        assert!((landscape(1.0, 1.0) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn all_methods_obey_budget_and_annealing_reproduces() {
        let a = anneal(INITIAL, training_score, neighbor, 7, SCHEDULE).unwrap();
        let b = anneal(INITIAL, training_score, neighbor, 7, SCHEDULE).unwrap();
        assert_eq!(a.best, b.best);
        assert_eq!(a.best_score, b.best_score);
        assert_eq!(a.accepted_uphill, b.accepted_uphill);
        for result in [
            a,
            greedy(7).unwrap(),
            random_search(7).unwrap(),
            grid_search(),
        ] {
            assert_eq!(result.evaluations, BUDGET);
            assert!(result.best_score >= oracle().best_score);
        }
    }
}
