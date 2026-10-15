#!/usr/bin/env python3
"""Execute the real Rust acceptance tests and independently check their CSV output."""

import csv
import io
import math
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FIELDS = "temperature,seed,trials,accepted,expected_probability,observed_rate,absolute_error,tolerance"


def validate(text):
    rows = list(csv.DictReader(io.StringIO(text)))
    assert len(rows) == 5
    assert {(float(row["temperature"]), int(row["seed"])) for row in rows} == {(20., 11), (4., 29), (2., 47), (1., 71), (.5, 101)}
    for row in rows:
        trials = int(row["trials"])
        assert trials == 200_000
        accepted = int(row["accepted"])
        assert 0 <= accepted <= trials
        expected = math.exp(-2 / float(row["temperature"]))
        observed = accepted / trials
        tolerance = math.sqrt(math.log(10 / 1e-6) / (2 * trials))
        assert abs(expected - observed) <= tolerance
        for field, value in [("expected_probability", expected), ("observed_rate", observed),
                             ("absolute_error", abs(expected-observed)), ("tolerance", tolerance)]:
            assert abs(float(row[field]) - value) < 1e-11, field


def main():
    command = ["cargo", "test", "--locked", "--offline", "--test", "acceptance", "--", "--nocapture"]
    run = subprocess.run(command, cwd=ROOT, env=dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target")),
                         text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if run.returncode:
        print(run.stdout)
        raise SystemExit(run.returncode)
    prefix = "ACCEPTANCE_RESULT,"
    lines = [line.removeprefix(prefix) for line in run.stdout.splitlines() if line.startswith(prefix)]
    text = FIELDS + "\n" + "\n".join(lines) + "\n"
    validate(text)
    (ROOT / "results/acceptance.csv").write_text(text)
    print(text, end="")
    print("PASS: five actual-kernel acceptance cases, one million proposals, independently checked formulas")


if __name__ == "__main__":
    main()
