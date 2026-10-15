# Independent validation review

Status: implementation and numerical evidence checked on October 15, 2026.
The article's Rust excerpt matches the tested source exactly.

## Scope of the evidence

The experiment compares optimizers on a deterministic synthetic objective. Its
results can demonstrate search behavior on that objective. They do not measure
service throughput, production latency, infrastructure spending, or agent quality.

All algorithms must receive the same objective evaluation budget, including any
initial candidate evaluation. An exhaustive oracle may establish the attainable
minimum, but its evaluations are outside every optimizer's budget and must never
inform candidate selection.

Held-out scores must be computed only after each optimizer has selected its
configuration using training scores. A changed synthetic objective is a
sensitivity check, not evidence of generalization to real production workloads.

## Executed checks

- `CARGO_TARGET_DIR=target cargo test --locked --offline`: eight tests passed.
  Tests exercise invalid seed and schedule rejection, nonfinite score rejection,
  score-difference overflow rejection, best retention after accepted regressions,
  objective call counting, cooling endpoints, oracle calculation, repeated
  seeded searches, and escape across an explicit three-state barrier.
- `CARGO_TARGET_DIR=target cargo clippy --locked --offline --all-targets -- -D warnings`:
  passed without warnings.
- `scripts/verify.py` independently recomputed all 120 training and shifted scores
  with Python's math library, exhaustively enumerated the reference minimum,
  checked seed coverage, candidate bounds, evaluation budgets, optimality gaps,
  and the deterministic grid's result. These checks passed.
- Independently enumerated the one-dimensional proposal transition counts,
  including out-of-range self transitions. Every forward count equaled its
  reverse count. The random axis choice preserves this symmetry in two dimensions.
- Executed the release binary twice in fresh temporary directories. Both complete
  results matched the recorded CSV in every field except elapsed time. The test
  intentionally excludes timing because clock measurements are not deterministic.
- `python3 scripts/verify.py --reproduce` passed as one complete command after
  the editorial build inserted the final excerpt. The JSON and Markdown summary
  statistics were also compared with an independent calculation from the raw CSV.
- The publication and implementation text sweep found no em dash characters at
  the time of review. The verification script includes this check for future edits.

Source inspection confirmed that all methods include the common initial point in
their budget; random search makes 255 further draws with replacement; grid search
visits each of its 256 chosen points once; and invalid local moves still incur an
evaluation. The oracle and shifted scores never feed candidate selection.

The generator rejects seed zero, converts its upper 53 output bits to a unit
float, and uses rejection sampling for bounded integers. It is a deterministic,
non-cryptographic generator. No claim of cryptographic or statistical certification
is made. Same seed labels do not imply identical candidate sequences across
algorithms because they consume random draws differently.

## Numerical findings

| Method | Median training score | Runs reaching the discrete optimum |
| --- | ---: | ---: |
| Annealing | 2.564507861122 | 30 / 30 |
| Greedy | 2.564507861122 | 30 / 30 |
| Random | 4.219781425802 | 5 / 30 |
| Fixed grid | 3.479834739799 | 0 / 30 repeated references |

Annealing accepted between 14 and 33 uphill moves per run. Thus the acceptance
mechanism executed, but it did not improve the final score over greedy search in
this experiment. A neighborhood permitting jumps across several coordinate values
can make a seemingly difficult landscape easy for greedy search.

A separate three-state test with scores `[1, 2, 0]` demonstrates the barrier
mechanism directly: annealing accepts the uphill intermediate state and reaches
the best state, whereas greedy search cannot leave its initial state. This is a
deliberately constructed correctness fixture with a selected seed, not an
additional comparative performance result.

The grid is a single deterministic reference repeated in the CSV. Those repeats
are not independent samples. Its alignment matters: this coarse grid does not
include central indices 31 and 32, where the discrete optimum lies. Its performance
does not establish that grids are generally inferior.

Thirty seeds quantify variation on one fixed synthetic objective. They do not
establish performance on other workloads, statistical significance across problem
families, or gains in cost or throughput. The recorded durations are single
measurements of a cheap objective and include optimizer overhead. They are not a
production performance benchmark.

## Defect found and repaired

The first kernel version accepted any finite objective values but did not reject
overflow when subtracting two finite scores. An extreme score difference could
become infinity and distort acceptance. The implementation now rejects a
nonfinite difference, and its regression test passed. This repair does not change
the bounded Rastrigin results.

## Original draft corrections

With inclusive integer batch sizes, the motivating configuration count is
64 * 4,065 * 196 = 50,991,360. One evaluation per second would take approximately
590.18 days. This illustrative space is distinct from the executable experiment.

The acceptance formula assumes a positive temperature and finite objective
differences. Temperature is expressed on the objective's scale. Multiplying all
scores by a constant changes acceptance unless temperature changes with it.

Clamping continuous proposals to fixed bounds does not preserve a symmetric
proposal distribution. A tested discrete implementation should document how it
handles boundaries and should not silently inherit that teaching shortcut.

Claims of global convergence require restrictive conditions and cannot be inferred
from a finite search with geometric cooling. Best observed is the appropriate
description for practical returned configurations.
