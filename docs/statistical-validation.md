# Acceptance probability validation

The integration tests in `tests/acceptance.rs` exercise the public `anneal`
function from the Rust library. They do not reproduce the acceptance decision in
a separate test implementation. Six integration tests passed on October 15, 2026;
the measured test runtime was approximately 0.06 seconds on the development host.
Clippy also passed with warnings treated as errors.

Run the tests and print the measured cases with:

```bash
CARGO_TARGET_DIR=target cargo test --locked --offline --test acceptance -- --nocapture
```

## Predeclared empirical check

Each of five cases starts at integer state zero. The neighbor proposes
`current + 1`, and the objective is `2 * state`. Every proposal therefore has
exactly the same positive score difference, 2, whether previous proposals were
accepted or rejected. These integer-valued scores and differences are exactly
representable over the tested range.

Each case uses 200,000 proposals, plus one initial objective evaluation. The
temperature is constant within each case. Temperatures, seeds, trial count, and
tolerance were declared before executing the tests and were not adjusted after
observing the results.

| Temperature | Seed | Accepted | Trials | Expected probability | Observed fraction | Absolute error |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 | 11 | 180,971 | 200,000 | 0.904837418036 | 0.904855 | 0.000017581964 |
| 4 | 29 | 120,752 | 200,000 | 0.606530659713 | 0.603760 | 0.002770659713 |
| 2 | 47 | 73,976 | 200,000 | 0.367879441171 | 0.369880 | 0.002000558829 |
| 1 | 71 | 26,980 | 200,000 | 0.135335283237 | 0.134900 | 0.000435283237 |
| 0.5 | 101 | 3,765 | 200,000 | 0.018315638889 | 0.018825 | 0.000509361111 |

Expected probability is `exp(-2 / temperature)`. Each case must have absolute
error at most **0.006347853112**, about 0.635 percentage points. The tolerance is
calculated from:

```text
n = 200,000 trials per case
m = 5 cases
alpha = 0.000001
tolerance = sqrt(ln(2*m/alpha) / (2*n))
```

Under an ideal model of independent uniform random draws within each case,
acceptances with fixed probability are independent Bernoulli observations.
[Hoeffding's inequality](https://doi.org/10.1080/01621459.1963.10500830)
bounds a single case's probability of exceeding this
absolute tolerance by `2 * exp(-2*n*tolerance^2)`. A union bound over the five
cases gives an overall false-rejection probability at most `alpha`. Independence
between cases is not required for that union bound.

The actual run uses fixed, deterministic xorshift64* streams. These tests neither
prove that those streams are independent uniform samples nor certify the random
generator. There is no random repeated execution behind a probabilistic guarantee
for these fixed seeds. The bound motivates a conservative acceptance threshold
under the stated model; the executed evidence is a deterministic regression check.
Floating-point evaluation of temperature and exponential also introduces tiny
differences from the exact real-valued mathematical probabilities.

The test records current states observed by the neighbor. Those states provide
an additional check that the reported acceptance count corresponds to actual
state changes. Every decision except the last is observed by the following
proposal, so the independently observed count must agree within one. The tests
also check the exact number of objective calls and preservation of the initial
best state in this deliberately all-uphill problem.

Output lines beginning with `ACCEPTANCE_RESULT` contain these CSV fields:

```text
marker,temperature,seed,trials,accepted,expected,observed,absolute_error,tolerance
```

## Exact behavioral checks

Separate fixtures inspect the current state supplied to the next proposal. This
distinguishes accepting a candidate from merely calculating its score.

- Downhill proposals always advance the current state and improve the retained best.
- Equal scores advance the current state and preserve the first recorded best.
- At temperature 0.001 and positive difference 2, `exp(-2000)` underflows to zero
  in `f64`; every uphill proposal is rejected in this floating-point fixture.
- Scaling objective values and the full cooling schedule by 16 preserves the
  entire observed state sequence for the fixed seed in a mixed uphill/downhill
  fixture. This checks the dependence on the ratio of difference to temperature.
- A mixed sequence of improvements and accepted regressions retains the best
  state even after a later worse state is accepted.

## Limits of this evidence

The one-way proposal used to isolate acceptance is a test fixture. It does not
form a symmetric proposal distribution for a stationary Boltzmann sampler, and
the test makes no sampling-distribution claim.

Five temperatures and five fixed seeds do not exhaust the parameter space. The
absolute tolerance can miss small biases, especially relative biases at low
acceptance probabilities. These checks do not measure convergence rates, compare
optimizer quality, validate real-world objective functions, or establish
production generalization. The extreme-cold result is an exact floating-point
behavior, not a claim that the mathematical exponential is exactly zero.

The scaling fixture demonstrates agreement on one finite seeded trajectory.
Floating-point rounding can change decisions near thresholds on other inputs.
Passing mutation tests, reported separately, only demonstrates detection of the
specific mutations actually executed.

## Independent review of the expanded contracts and mutation evidence

The nine tests in `tests/algorithm_contracts.rs` were reviewed and executed
independently; all passed. Both mutation reports contain 181 distinct mutation
names, their category totals agree with their individual records, and their
source hashes match the current `src/lib.rs`. The original suite reports 102
caught, 53 missed, 25 unviable, and one timeout. The expanded suite reports 153
caught, two missed, 25 unviable, and one timeout.

The boundary fixture's seed was independently derived by taking the multiplier's
modular inverse modulo `2^64`, then undoing the three xor-shift transforms. Seed
`11013388057781577488` produces first raw output 1. Discarding its low 11 bits
therefore gives an exactly zero uniform value. This fixture detects the incorrect
use of `<=` instead of `<` when both the draw and acceptance probability are zero.

The two surviving mutants change the sign of one symmetric coordinate offset in
the shifted-score average. Each replaces the same four real-valued terms with a
permutation, so the mathematical result is unchanged. That is a semantic review,
not an assertion that arbitrary floating-point summation orders always agree
bit for bit. An independent Python enumeration of all 4,096 project grid points
found no difference for either sign change on the review host.

The remaining timeout changes the bounded generator's acceptance comparison from
`>=` to `<`. For power-of-two bounds, its rejection threshold is zero, so the
mutant cannot terminate. The timeout is reported separately from failed tests.
Unviable mutants are build failures, not detected runtime defects.

Some caught mutations change a promised seeded stream or the selected candidate
among ties while preserving a distribution or optimal score. The CLI fixture and
generator reference intentionally detect such reproducibility changes. Therefore
the caught count is not a count of distinct algorithmic correctness defects.
Neighbor support coverage checks which moves appear, not their exact frequencies.
No claim of exhaustive behavioral coverage or universal correctness follows from
the finite mutation campaign.
