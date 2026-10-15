//! Acceptance behavior checked through the actual public optimization kernel.
use annealing_proof::{anneal, Rng, Schedule};
use std::cell::Cell;

const TRIALS: usize = 200_000;
const FAMILY_ALPHA: f64 = 1e-6;
const CASES: [(f64, u64); 5] = [(20.0, 11), (4.0, 29), (2.0, 47), (1.0, 71), (0.5, 101)];

fn constant_schedule(temperature: f64, proposals: usize) -> Schedule {
    Schedule {
        start: temperature,
        end: temperature,
        evaluations: proposals + 1,
    }
}

#[test]
fn empirical_uphill_acceptance_matches_predeclared_probabilities() {
    let tolerance =
        (f64::ln(2.0 * CASES.len() as f64 / FAMILY_ALPHA) / (2.0 * TRIALS as f64)).sqrt();
    for (temperature, seed) in CASES {
        let calls = Cell::new(0_usize);
        let last_observed = Cell::new(0_u32);
        let outcome = anneal(
            0_u32,
            |state| {
                calls.set(calls.get() + 1);
                // Integer states keep every candidate-current difference exactly 2.
                2.0 * f64::from(*state)
            },
            |current, _| {
                assert!(*current == last_observed.get() || *current == last_observed.get() + 1);
                last_observed.set(*current);
                current + 1
            },
            seed,
            constant_schedule(temperature, TRIALS),
        )
        .unwrap();
        let probability = (-2.0 / temperature).exp();
        let observed = outcome.accepted_uphill as f64 / TRIALS as f64;
        let error = (observed - probability).abs();
        println!(
            "ACCEPTANCE_RESULT,{temperature},{seed},{TRIALS},{},{probability:.12},{observed:.12},{error:.12},{tolerance:.12}",
            outcome.accepted_uphill
        );
        assert!(
            error <= tolerance,
            "T={temperature}: error {error} exceeds {tolerance}"
        );
        assert_eq!(outcome.evaluations, TRIALS + 1);
        assert_eq!(calls.get(), TRIALS + 1);
        assert_eq!(outcome.best, 0);
        assert_eq!(outcome.best_score, 0.0);
        // The neighbor sees all decisions except the final one. Its observed
        // current state therefore independently checks the reported count to 1.
        assert!(outcome.accepted_uphill >= last_observed.get() as usize);
        assert!(outcome.accepted_uphill <= last_observed.get() as usize + 1);
    }
}

#[test]
fn downhill_candidates_always_advance_the_current_state() {
    let mut visited = Vec::new();
    let result = anneal(
        0_u32,
        |state| -f64::from(*state),
        |state, _| {
            visited.push(*state);
            state + 1
        },
        13,
        constant_schedule(0.001, 64),
    )
    .unwrap();
    assert_eq!(visited, (0..64).collect::<Vec<_>>());
    assert_eq!(result.best, 64);
    assert_eq!(result.best_score, -64.0);
    assert_eq!(result.accepted_uphill, 0);
    assert_eq!(result.evaluations, 65);
}

#[test]
fn equal_scores_advance_the_current_state_without_erasing_the_first_best() {
    let mut visited = Vec::new();
    let result = anneal(
        0_u32,
        |_| 7.0,
        |state, _| {
            visited.push(*state);
            state + 1
        },
        17,
        constant_schedule(0.001, 64),
    )
    .unwrap();
    assert_eq!(visited, (0..64).collect::<Vec<_>>());
    assert_eq!(result.best, 0);
    assert_eq!(result.best_score, 7.0);
    assert_eq!(result.accepted_uphill, 0);
}

#[test]
fn extremely_cold_search_rejects_every_uphill_candidate() {
    let mut visited = Vec::new();
    let result = anneal(
        0_u32,
        |state| 2.0 * f64::from(*state),
        |state, _| {
            visited.push(*state);
            state + 1
        },
        19,
        constant_schedule(0.001, 128),
    )
    .unwrap();
    // exp(-2000) underflows to zero in f64, so rejection is exact here.
    assert_eq!(visited, vec![0; 128]);
    assert_eq!(result.accepted_uphill, 0);
    assert_eq!(result.best, 0);
    assert_eq!(result.best_score, 0.0);
}

#[test]
fn scaling_objective_and_temperature_preserves_the_seeded_path() {
    fn run(scale: f64) -> (Vec<usize>, annealing_proof::Outcome<usize>) {
        let mut visited = Vec::new();
        let result = anneal(
            0_usize,
            |state| [7.0, 2.0, 5.0, -1.0, 3.0, -2.0, 0.0][*state] * scale,
            |state, rng: &mut Rng| {
                visited.push(*state);
                if rng.below(2) == 0 {
                    (state + 6) % 7
                } else {
                    (state + 1) % 7
                }
            },
            23,
            Schedule {
                start: 8.0 * scale,
                end: 0.125 * scale,
                evaluations: 8_193,
            },
        )
        .unwrap();
        (visited, result)
    }
    let (original_path, original) = run(1.0);
    let (scaled_path, scaled) = run(16.0);
    assert_eq!(original_path, scaled_path);
    assert_eq!(original.best, scaled.best);
    assert_eq!(original.best_score * 16.0, scaled.best_score);
    assert_eq!(original.accepted_uphill, scaled.accepted_uphill);
    assert!(original.accepted_uphill > 0);
    assert_eq!(original.evaluations, scaled.evaluations);
}

#[test]
fn best_so_far_survives_later_accepted_regressions() {
    let mut visited = Vec::new();
    let result = anneal(
        0_usize,
        |state| [5.0, 1.0, 3.0, 0.0, 2.0][*state],
        |state, _| {
            visited.push(*state);
            state + 1
        },
        31,
        constant_schedule(1e100, 4),
    )
    .unwrap();
    assert_eq!(visited, vec![0, 1, 2, 3]);
    assert_eq!(result.best, 3);
    assert_eq!(result.best_score, 0.0);
    assert_eq!(result.accepted_uphill, 2);
    assert_eq!(result.evaluations, 5);
}
