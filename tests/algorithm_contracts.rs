use annealing_proof::{
    coordinate, greedy, grid_search, held_out_score, neighbor, Point, Rng, Schedule,
};
use std::collections::BTreeSet;
use std::f64::consts::TAU;

#[test]
fn seeded_generator_matches_independently_calculated_reference_vector() {
    // Calculated with a separate Python integer implementation of xorshift64*.
    // This locks the promised stream for reproduction, not randomness quality.
    let expected = [
        5_180_492_295_206_395_165,
        12_380_297_144_915_551_517,
        13_389_498_078_930_870_103,
        5_599_127_315_341_312_413,
        1_036_278_371_763_004_928,
        14_440_594_066_559_445_721,
    ];
    let mut rng = Rng::new(1).unwrap();
    for value in expected {
        assert_eq!(rng.next_u64(), value);
    }
    let mut uniform = Rng::new(1).unwrap();
    assert_eq!(
        uniform.unit(),
        (expected[0] >> 11) as f64 / 9_007_199_254_740_992.0
    );
    for _ in 0..10_000 {
        assert!((0.0..1.0).contains(&uniform.unit()));
    }
}

#[test]
fn bounded_generator_handles_non_power_of_two_rejection_and_unit_bound() {
    let bound = (1_u64 << 63) + 1;
    let mut rng = Rng::new(1).unwrap();
    // First reference draw is below 2^64 mod bound and must be rejected.
    // Second draw is accepted, leaving the third raw draw next in the stream.
    assert_eq!(rng.below(bound), 12_380_297_144_915_551_517 - bound);
    assert_eq!(rng.next_u64(), 13_389_498_078_930_870_103);
    for _ in 0..100 {
        assert_eq!(rng.below(1), 0);
    }
}

#[test]
fn coordinates_have_exact_domain_and_strictly_increasing_spacing() {
    assert!((coordinate(0) + 5.12).abs() < 1e-12);
    assert!((coordinate(63) - 5.12).abs() < 1e-12);
    for i in 0..63 {
        assert!((coordinate(i + 1) - coordinate(i) - 10.24 / 63.0).abs() < 1e-12);
        assert!(coordinate(i + 1) > coordinate(i));
    }
}

#[test]
fn shifted_score_matches_analytic_average_at_asymmetric_points() {
    for point in [
        Point { i: 7, j: 19 },
        Point { i: 31, j: 32 },
        Point { i: 0, j: 47 },
    ] {
        let x = -5.12 + 10.24 * f64::from(point.i) / 63.0;
        let y = -5.12 + 10.24 * f64::from(point.j) / 63.0;
        // E[(x +/- h)^2] = x^2+h^2 and the averaged cosine is cos(x)*cos(h).
        let expected = x * x + y * y + 0.02 + 20.0
            - 10.0 * (TAU * 0.1).cos() * ((TAU * x).cos() + (TAU * y).cos());
        assert!((held_out_score(&point) - expected).abs() < 1e-11);
    }
}

#[test]
fn actual_neighbor_reaches_only_the_specified_axis_moves_at_edges_and_interior() {
    for point in [
        Point { i: 0, j: 0 },
        Point { i: 63, j: 63 },
        Point { i: 7, j: 42 },
        Point { i: 32, j: 17 },
    ] {
        let mut expected = BTreeSet::new();
        for step in -8..=8 {
            if step == 0 {
                continue;
            }
            for axis in 0..2 {
                let mut candidate = (point.i, point.j);
                let old = if axis == 0 { point.i } else { point.j };
                let moved = i64::from(old) + step;
                if (0..64).contains(&moved) {
                    if axis == 0 {
                        candidate.0 = moved as u32;
                    } else {
                        candidate.1 = moved as u32;
                    }
                }
                expected.insert(candidate);
            }
        }
        let mut actual = BTreeSet::new();
        let mut rng = Rng::new(419).unwrap();
        for _ in 0..4_096 {
            let candidate = neighbor(&point, &mut rng);
            assert!(expected.contains(&(candidate.i, candidate.j)));
            actual.insert((candidate.i, candidate.j));
        }
        assert_eq!(actual, expected);
    }
}

#[test]
fn baseline_scores_match_independently_enumerated_reference_values() {
    assert!((greedy(7).unwrap().best_score - 2.564_507_861_122).abs() < 1e-10);
    let grid = grid_search();
    assert!((grid.best_score - 3.479_834_739_799).abs() < 1e-10);
    let levels: Vec<u32> = (0..16)
        .map(|i| (f64::from(i) * 63.0 / 15.0).round() as u32)
        .collect();
    assert!(levels.contains(&grid.best.i) && levels.contains(&grid.best.j));
}

#[test]
fn geometric_cooling_has_correct_interior_and_positive_finite_endpoints() {
    let schedule = Schedule {
        start: 100.0,
        end: 1.0,
        evaluations: 12,
    };
    schedule.validate().unwrap();
    assert!((schedule.temperature(5) - 10.0).abs() < 1e-12);
    let mut previous = f64::INFINITY;
    for proposal in 0..11 {
        let temperature = schedule.temperature(proposal);
        assert!(temperature.is_finite() && temperature > 0.0 && temperature < previous);
        previous = temperature;
    }
}

#[test]
fn a_zero_uniform_draw_still_rejects_zero_acceptance_probability() {
    // Inverting the integer xorshift64* transform gives a first output of 1.
    // Dropping its low 11 bits makes unit() exactly zero. No seed search needed.
    let seed = 11_013_388_057_781_577_488;
    assert_eq!(Rng::new(seed).unwrap().unit(), 0.0);
    let result = annealing_proof::anneal(
        0_u32,
        |state| f64::from(*state),
        |state, _| state + 1,
        seed,
        Schedule {
            start: 0.001,
            end: 0.001,
            evaluations: 3,
        },
    )
    .unwrap();
    assert_eq!(result.accepted_uphill, 0);
    assert_eq!(result.best, 0);
}

#[test]
fn executable_reproduces_recorded_experiment_except_timing() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_annealing-proof"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let actual = String::from_utf8(output.stdout).unwrap();
    fn stable(text: &str) -> Vec<Vec<&str>> {
        text.lines()
            .map(|line| {
                line.split(',')
                    .enumerate()
                    .filter_map(|(index, field)| (index != 8).then_some(field))
                    .collect()
            })
            .collect()
    }
    assert_eq!(stable(&actual), stable(include_str!("../results/raw.csv")));
}
