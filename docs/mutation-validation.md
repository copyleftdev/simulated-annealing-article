# Mutation testing the annealing implementation

We ran cargo-mutants 27.0.0 against every mutation it generated for `src/lib.rs`, without exclusions. The original eight Rust unit tests were compared with an expanded suite of 23 tests: the original eight, six acceptance integration tests, and nine algorithm contract tests. The production library source is identical in both campaigns. This isolates the effect of stronger tests.

| Outcome | Original tests | Expanded tests |
| --- | ---: | ---: |
| Caught by failing tests | 102 | 153 |
| Survived all tests | 53 | 2 |
| Failed to compile | 25 | 25 |
| Timed out | 1 | 1 |
| Total generated mutations | 181 | 181 |

The full per-mutation outcomes and failing test names are in [original results](../results/mutation-original.json) and [expanded results](../results/mutation-expanded.json). Neither compile failures nor timeouts are counted as assertion-based detections.

## Gaps that the mutation run exposed

The original Rust suite allowed removing the minus sign from `exp(-delta / temperature)` and replacing the uniform random output with a constant to go undetected. Its oracle tests also shared enough objective code that a broken coordinate transform could pass. The independent Python verifier covered some numerical errors, but cargo-mutants initially ran only Cargo tests, not that verifier.

The expanded suite checks actual acceptance frequencies, observes accepted current states, checks the documented random stream against a separately calculated reference vector, exercises non-power-of-two rejection sampling, and compares shifted scores with a closed-form average. It also checks actual neighbor support at asymmetric interior and boundary points and runs the executable against its recorded non-timing output. These tests caught 51 previously surviving mutations.

One especially useful boundary mutation changed `uniform < probability` to `uniform <= probability`. Ordinary samples are unlikely to hit exact equality. A test now uses an algebraically derived seed whose first generator output is 1, making the upper-53-bit uniform value exactly zero. At a temperature where the exponential underflows to zero, the correct strict comparison rejects the uphill move. The mutated inclusive comparison accepts it and fails the test. This seed is a deliberate numerical edge fixture, separate from the predeclared empirical-rate seeds.

The executable fixture detects changes to the published seeded results, including reversing signed neighbor steps or changing tie selection. This protects the reproducibility contract; it does not mean every alternative tie rule or statistically equivalent neighborhood is an inferior optimizer.

## The two survivors

The remaining mutations replace `x + dx` with `x - dx`, or `y + dy` with `y - dy`, inside `held_out_score`.

Both offsets range over `{-0.1, +0.1}` and all four combinations are averaged. Negating one offset only permutes the same mathematical terms, so these mutations are equivalent for this intended average in exact arithmetic. Floating-point addition order can change the last bits; the tests intentionally use tolerances rather than requiring a particular summation order. We retained both survivors in the published counts rather than excluding them to claim a perfect score.

## The timeout and compile failures

The timeout reverses the bounded-generator rejection condition from `value >= threshold` to `value < threshold`. With a power-of-two bound, the threshold is zero and no unsigned value can satisfy the mutant condition. The loop never terminates. The runner detects it through a 10-second timeout, not an assertion failure.

The 25 unviable mutants fail compilation. They provide no evidence that the tests detect incorrect runtime behavior.

## Reproduce

Install the recorded tool version if needed:

```sh
cargo install cargo-mutants --version 27.0.0 --locked
python3 scripts/mutation_check.py --suite original
python3 scripts/mutation_check.py --suite expanded
```

Each command creates an isolated temporary project, runs an unmutated baseline, and tests all generated library mutations with four jobs, a 60-second build timeout, and a 10-second test timeout. Cargo compilation runs offline with the lockfile. The wrapper writes a compact report after verifying the campaign completed. A completed wrapper is not a declaration that all mutants were caught: inspect the outcome counts and survivors. Tool installation can require network access.

`--suite original` includes the current library's eight inline tests and omits integration tests. `--suite expanded` adds the integration tests and their recorded CSV fixture. Source and test hashes are recorded so later changes can be distinguished from this campaign.

The CLI is exercised by the expanded tests but `src/main.rs`, the Python scripts, documentation, and tests themselves are outside the mutation target. Generated mutations cannot represent every possible bug. This campaign strengthens confidence in the tested behavior; it is not a proof of correctness, statistical randomness certification, or production-workload validation. [cargo-mutants documentation](https://mutants.rs/) explains the tool's purpose and workflow.
