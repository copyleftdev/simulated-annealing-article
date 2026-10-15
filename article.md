---
title: "Stop Guessing. Let the Algorithm Find the Sweet Spot."
published: false
cover_image: https://raw.githubusercontent.com/copyleftdev/simulated-annealing-article/main/assets/cover.png
description: "A reproducible Rust experiment in simulated annealing, search budgets, and the engineering discipline of measuring what matters."
tags: programming, algorithms, rust, performance
---

*Simulated annealing gives a search permission to get worse before it gets better. The useful part is learning when that permission pays off.*

There is a familiar ritual in software engineering: increase the thread count, change the batch size, run the benchmark, and squint at the result. Then change another number.

Eventually, something looks better, and we call it optimized.

But better than the previous attempt is a low bar. How much of the search space did we explore? What did each experiment cost? Does the improvement survive a different workload?

Those questions are the difference between tuning by habit and running an optimization experiment.

Simulated annealing is one way to run that experiment. Its most interesting feature is also its least intuitive: sometimes it accepts a worse candidate on purpose.

Let's examine why, implement the rule in Rust, and compare it with greedy search, random search, and a grid. Every method gets the same evaluation budget. The experiment is small enough to run locally and check exhaustively.

## Why the first valley can trap you

Imagine walking through a mountain range at night, trying to find its lowest point. You can see nearby terrain, so you keep taking downhill steps.

Eventually, every nearby step leads uphill. You have found a local minimum. Another valley might be deeper, but your rule prevents you from crossing the ridge.

That is the problem with accepting only immediate improvements. Whether a solution is a local minimum also depends on the moves your search permits. A larger jump might cross a barrier that a small step cannot.

Simulated annealing sometimes accepts uphill moves. It borrows its name from the connection between cooling physical materials and searching for low-energy configurations, developed in [Kirkpatrick, Gelatt, and Vecchi's 1983 paper](https://doi.org/10.1126/science.220.4598.671).

In the algorithm, energy becomes an objective score. Temperature becomes a parameter controlling the willingness to accept regressions. Cooling gradually reduces that willingness.

This creates a route out of some local minima. A finite run can still miss the best solution.

## The acceptance rule

Suppose lower scores are better. Calculate the change from the current candidate:

```text
delta = score(candidate) - score(current)
```

Accept improvements and ties. For a worse candidate, draw a uniform random number between zero inclusive and one exclusive, then accept when it is smaller than:

```text
p = exp(-delta / temperature)
```

This is the usual Metropolis acceptance rule for symmetric proposals, used in the [original simulated annealing paper](https://cgi.cse.unsw.edu.au/~cs4601/refs/papers/KirkpatrickGelattVecchi83.pdf). Symmetric means the chance of proposing a move equals the chance of proposing its reverse.

For a regression of two score units, the probability is about 82% at temperature 10, 14% at temperature 1, and effectively zero at temperature 0.1.

Temperature has the same units as the score. Multiply every score by 1,000 and keep the old temperature, and you have changed the search's behavior. A cooling schedule is a design choice, not a magic constant.

Also keep two records: where the search is now and the best candidate it has ever found. Exploring a worse configuration should not erase a better answer.

## Define what good means before searching

Consider these hypothetical service settings:

| Setting | Allowed values | Count |
| --- | --- | ---: |
| Worker threads | Every integer from 1 to 64 | 64 |
| Batch size | Every integer from 32 to 4,096 | 4,065 |
| Timeout | 50 to 2,000 milliseconds, stepping by 10 | 196 |

That is **50,991,360 configurations**. At one evaluation per second, sequential exhaustive search would take about **590 days**, before repeats or setup costs.

The hard part starts before choosing a search algorithm. What makes a configuration good?

One possible objective is:

```text
score = 0.5 * (p99_latency / reference_latency)
      + 0.3 * (memory / reference_memory)
      - 0.2 * (throughput / reference_throughput)
```

Fix those positive reference values before the experiment. The ratios are dimensionless; the weights express your priorities. Changing the scales during the search would make scores from different iterations incomparable.

Hard requirements need separate treatment. If a configuration violates a memory cap, correctness invariant, or latency limit, reject it as infeasible. A throughput gain should not quietly compensate for breaking a requirement.

Even among feasible candidates, there may be several reasonable trade-offs. A candidate lies on the Pareto frontier if no feasible alternative improves an objective without worsening another. A weighted score expresses a preference among trade-offs; it does not discover your priorities for you.

**The most important function is the one that decides what good looks like.**

## Give the algorithm something we can verify

For an executable demonstration, use a smaller problem where we can check the answer exhaustively.

The companion Rust project searches a 64 by 64 grid of abstract parameter indices. Each index maps onto a coordinate between -5.12 and 5.12. Its objective is the two-dimensional [Rastrigin function](https://www.sfu.ca/~ssurjano/rastr.html):

```text
E(x, y) = x*x + y*y + 20
        - 10*cos(2*pi*x) - 10*cos(2*pi*y)
```

The quadratic terms pull the search toward the center. The cosine terms add many local valleys. These scores have no latency, throughput, or dollar units. This is a deterministic mathematical experiment.

The continuous function reaches zero at the origin. Our even-sized grid does not contain the origin, so we enumerate all 4,096 grid points to find the discrete optimum. That enumeration is a separate reference calculation, unavailable to the search algorithms.

![Heatmap of the synthetic Rastrigin objective and a one-dimensional slice showing many local valleys.](https://raw.githubusercontent.com/copyleftdev/simulated-annealing-article/main/assets/landscape.png)

The comparison fixes these choices before running:

- **256 objective evaluations per run**, including the initial candidate and repeated proposals.
- **The same initial point**, index `(63, 63)`, for each method.
- **30 seeds**, numbered 1 through 30, for the stochastic methods.
- **Annealing and greedy search share a neighborhood**: move one index by a signed step of 1 through 8. An out-of-range proposal stays in place and still costs an evaluation.
- **Annealing cools geometrically from 20 to 0.1** over its 255 proposals. Greedy search accepts only improvements or ties.
- **Random search samples the whole grid with replacement.** Grid search uses a fixed 16 by 16 subset, rounding evenly spaced coordinates to integer indices.

Equal evaluation counts make one cost comparable. They do not give all methods identical information or movement rules. The starting point, neighborhood, grid spacing, and cooling schedule remain consequential choices.

## The Rust loop

The project has no external Rust dependencies. Its seeded generator makes the search repeatable on the tested toolchain. This is the loop from the implementation; validation, random generation, and candidate construction live in the companion source.

<!-- BEGIN RUST EXCERPT -->
```rust
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
```
<!-- END RUST EXCERPT -->

The boundary handling matters. We choose signed moves with equal probability and leave invalid moves at the current point. Simply clamping an arbitrary move to the nearest boundary would accumulate probability there and lose the symmetry assumed by this acceptance rule.

The implementation also rejects invalid settings and nonfinite objective values, counts evaluations explicitly, and tests that a temporarily worse current state cannot overwrite the best result.

A separate unit test makes the barrier explicit: three adjacent states have costs `1, 2, 0`. Starting at the first state, greedy search cannot cross the middle state. With the test's fixed seed and temperature, annealing accepts the uphill move and then reaches zero. That tiny fixture verifies the mechanism; it is not evidence that annealing usually wins.

## Run it and compare the evidence

The [companion project](https://github.com/copyleftdev/simulated-annealing-article) includes the Rust source, raw results, an independent results checker, and a plotting script. Clone it and run:

```bash
git clone https://github.com/copyleftdev/simulated-annealing-article.git
cd simulated-annealing-article
export CARGO_TARGET_DIR=target
cargo test --locked
cargo run --release --locked -- results/raw.csv
python3 scripts/verify.py --reproduce
python3 scripts/analyze.py
```

Python analysis uses the standard library. Add Matplotlib to regenerate the figures with `python3 scripts/analyze.py --plots`.

<!-- BEGIN RESULTS -->
| Method | Median best score | Best to worst score | Exact grid optimum hits |
| --- | ---: | ---: | ---: |
| Annealing | 2.565 | 2.565 to 2.565 | 30/30 |
| Greedy | 2.565 | 2.565 to 2.565 | 30/30 |
| Random | 4.220 | 2.565 to 8.425 | 5/30 |
| Coarse grid | 3.480 | 3.480 to 3.480 | No (deterministic) |
<!-- END RESULTS -->

Lower is better. The exhaustive grid optimum is **2.564508**, and both annealing and greedy search reached it in all 30 runs. Annealing accepted between 14 and 33 uphill moves per run, but those moves did not improve its final score over greedy search here.

That result is a useful correction to the valley metaphor. Our neighborhood allows jumps of up to eight index positions. The cosine ripples repeat roughly every six positions, so greedy search can jump across hills without accepting a worse landing point. This experiment gives us no reason to prefer annealing over this greedy baseline on final score.

Random search reached the optimum in five runs. The coarse grid returned the same result every time; repeating it creates no new statistical evidence. Its chosen spacing misses the central indices 31 and 32, so its result is sensitive to grid alignment. These are observed score ranges, not confidence intervals or a universal ranking.

![Scores from 30 seeds: annealing and greedy reach the discrete optimum in every run; random search varies; the coarse grid has one fixed result.](https://raw.githubusercontent.com/copyleftdev/simulated-annealing-article/main/assets/search-comparison.png)

The raw CSV also records search duration. With such a cheap objective, optimizer overhead and machine conditions can substantially affect timing. An evaluation count is not a measurement of infrastructure savings.

There is a second check: after each run selects its best candidate, the harness shifts each coordinate by either -0.1 or +0.1, evaluates all four combinations, and records their mean score. These four evaluations are outside the 256-call search budget and its timer, and the search never sees their scores. This tests sensitivity to a small, specified change in the formula. It does not simulate production traffic or establish robustness to measurement noise.

That distinction matters. Thirty seeds describe variation in search outcomes on one fixed problem. They are not thirty independent workloads.

## Try breaking the optimizer

A passing test suite deserves a challenge too. We used [cargo-mutants](https://mutants.rs/) to alter the library and rerun its tests. The original eight Rust tests missed 53 of 181 generated mutations, including deleting the minus sign in the acceptance formula.

After adding acceptance and contract tests, **153 mutations triggered test failures**. Two survived: they permute symmetric terms in the shifted-score average, preserving its mathematical value. Another 25 did not compile, and one caused a nonterminating loop detected by a timeout. We report those categories separately in the [mutation review](https://github.com/copyleftdev/simulated-annealing-article/blob/main/docs/mutation-validation.md).

The statistical check drives the real kernel through **one million uphill proposals** across five predeclared temperatures. At a score difference of 2 and temperature 2, expected acceptance is about 36.79%; observed acceptance was 36.988%. Every case fell within the predeclared tolerance of about 0.635 percentage points.

That tolerance uses a Hoeffding bound under ideal independent random draws. Our fixed pseudorandom streams make this a reproducible regression check, not a certification of randomness. The [statistical review](https://github.com/copyleftdev/simulated-annealing-article/blob/main/docs/statistical-validation.md) records all cases and assumptions. The expanded suite has 23 tests; production optimizer code and benchmark results are unchanged.

## The optimizer proposes. The testing harness disposes.

A real service adds a complication: measurements fluctuate.

A noisy score can make a regression look like an improvement. Searching thousands of configurations also creates thousands of opportunities to get lucky. The most impressive single measurement may be the least trustworthy result.

For service tuning, make the evaluator earn its authority:

1. Fix the workload, objective, constraints, and search budget before comparing methods.
2. Repeat measurements, control warmup and machine conditions, and interleave candidates when drift could bias the comparison. Charge those repetitions to the evaluation budget.
3. Evaluate finalists on reserved workloads, including skewed inputs, contention, and failure cases. Report variability alongside the central estimate.
4. Check correctness and operational limits before promoting a candidate. Keep a rollback path and monitor the deployed result.

Once reserved workloads influence another round of tuning, they become development data. A fresh evaluation set is needed for a fresh generalization claim.

Accepting a worse candidate is a search decision inside an evaluation environment. It does not require deploying that candidate to customers.

## Where this becomes an engineering habit

The same framing gives several everyday tuning problems a concrete shape:

| Problem | Candidate decisions | Objective and constraints |
| --- | --- | --- |
| Service performance | Workers, batching, queue depth | Minimize cost per completed request while meeting latency and correctness limits |
| AI agent orchestration | Model routing, retry budget, verification threshold | Minimize cost on a fixed task set while meeting quality and latency requirements |
| Test scheduling | Test order and assignment to workers | Minimize completion time while preserving the required suite and dependencies |
| Infrastructure sizing | Instance mix, capacity, scaling thresholds | Minimize cost under representative demand while meeting capacity and recovery requirements |

Applying search to these problems requires a candidate representation, legal moves, and a trustworthy evaluator. In an agent system, for example, a cheap answer is useful only if the quality check deserves trust. Evaluators can be fooled or overfit too.

The separation is useful: a search proposes configurations, an evaluator measures outcomes, and a deployment process decides what can ship.

You do not need to adopt annealing to adopt that architecture.

## Make annealing earn its place

Start with a baseline you can understand. If the space is small, enumerate it. If a coarse grid or random search gives good results within the budget, that is valuable evidence.

Annealing becomes interesting when useful local moves exist but a search that accepts only improvements gets trapped. Its neighborhood and temperature schedule still need attention. If objective evaluations are expensive, compare methods designed to spend fewer evaluations. If useful gradients exist, consider methods that exploit them.

The cost of tuning the optimizer also belongs in the accounting. Trying twenty cooling schedules and reporting only the best is another search, with another budget.

The engineering habit is simple: define success, constrain the experiment, compare alternatives, and keep the evidence. Repeat when the environment changes.

Sometimes a better answer is on the other side of a hill. Sometimes a simpler search already finds it. Measure which situation you are in.

*What configuration have you spent hours tuning manually, and what would a trustworthy objective function for it look like?*

*Disclosure: AI agents drafted and revised this article, implemented the experiment, and performed technical review. The numerical results come from executing the accompanying code.*
