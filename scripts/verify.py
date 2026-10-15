#!/usr/bin/env python3
"""Independently validate experiment evidence using the Python standard library."""

import argparse
import csv
import math
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
SIDE = 64
BUDGET = 256
SEEDS = set(range(1, 31))


def coordinate(index):
    return -5.12 + 10.24 * index / 63.0


def objective_xy(x, y):
    return x * x + y * y + 20.0 - 10.0 * math.cos(2.0 * math.pi * x) - 10.0 * math.cos(2.0 * math.pi * y)


def objective(i, j):
    return objective_xy(coordinate(i), coordinate(j))


def shifted(i, j):
    x, y = coordinate(i), coordinate(j)
    return sum(objective_xy(x + dx, y + dy) for dx in (-0.1, 0.1) for dy in (-0.1, 0.1)) / 4.0


def close(actual, expected, label):
    assert math.isfinite(actual), f"Nonfinite {label}: {actual}"
    assert math.isclose(actual, expected, rel_tol=1e-10, abs_tol=1e-9), f"{label}: {actual} != {expected}"


def read_rows(path):
    with path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def check_rows(rows):
    oracle = min(objective(i, j) for i in range(SIDE) for j in range(SIDE))
    initial = objective(63, 63)
    methods = {row["method"] for row in rows}
    assert methods == {"annealing", "greedy", "random", "grid"}, f"Unexpected methods: {methods}"
    assert len(rows) == 120, f"Expected 120 rows, got {len(rows)}"
    seen = set()
    for row in rows:
        method, seed = row["method"], int(row["seed"])
        assert (method, seed) not in seen, f"Duplicate method/seed: {(method, seed)}"
        seen.add((method, seed))
        assert seed in SEEDS, f"Unexpected seed: {seed}"
        assert int(row["evaluations"]) == BUDGET, f"Budget mismatch: {row}"
        i, j = int(row["best_i"]), int(row["best_j"])
        assert 0 <= i < SIDE and 0 <= j < SIDE, f"Out of bounds: {(i, j)}"
        measured = float(row["best_score"])
        close(measured, objective(i, j), "training score")
        close(float(row["held_out_score"]), shifted(i, j), "shifted score")
        close(float(row["oracle_score"]), oracle, "exhaustive oracle")
        close(float(row["gap"]), measured - oracle, "optimality gap")
        assert measured >= oracle - 1e-9, "Reported result beats the exhaustive oracle"
        assert measured <= initial + 1e-9, "Best retention failed relative to common initial point"
        assert int(row["elapsed_ns"]) >= 0, "Negative elapsed time"
        uphill = int(row["accepted_uphill"])
        assert 0 <= uphill < BUDGET, "Invalid uphill acceptance count"
        if method != "annealing":
            assert uphill == 0, f"Unexpected uphill count for {method}"
    for method in methods:
        assert {seed for name, seed in seen if name == method} == SEEDS
    grid = [row for row in rows if row["method"] == "grid"]
    assert len(grid) == 30, "Expected deterministic grid reference"
    grid_points = [round(k * 63 / 15) for k in range(16)]
    expected_grid = min(objective(i, j) for i in grid_points for j in grid_points)
    for row in grid:
        close(float(row["best_score"]), expected_grid, "coarse-grid best")
    print(f"PASS: 120 result rows, scores, shifted check, budgets, seeds, and oracle ({oracle:.12f})")


def check_neighborhood():
    # Count discrete axis/offset proposals, including rejected boundary moves as
    # self transitions. Equal forward and reverse counts prove symmetry here.
    for origin in range(SIDE):
        for target in range(SIDE):
            def count(start, end):
                return sum((start + step if 0 <= start + step < SIDE else start) == end
                           for step in range(-8, 9) if step)
            assert count(origin, target) == count(target, origin)
    print("PASS: independent discrete boundary proposal symmetry check")


def check_text():
    offenders = []
    for folder in (ROOT, ROOT / "docs", ROOT / "scripts", ROOT / "src", ROOT / "tests", ROOT / "results", ROOT / "assets"):
        if not folder.exists():
            continue
        for path in folder.iterdir():
            if path.is_file() and path.suffix in {".md", ".py", ".rs", ".toml", ".txt", ".csv", ".json", ".svg", ".sh"}:
                if chr(0x2014) in path.read_text():
                    offenders.append(str(path.relative_to(ROOT)))
    assert not offenders, f"Prohibited em dash character in: {offenders}"
    print("PASS: publication and implementation text has no em dash characters")


def check_excerpt():
    source = (ROOT / "src/lib.rs").read_text()
    snippet = source.split("// ARTICLE_SNIPPET_START\n", 1)[1].split("// ARTICLE_SNIPPET_END", 1)[0].strip("\n")
    articles = [path for path in ROOT.glob("*.md") if "<!-- BEGIN RUST EXCERPT -->" in path.read_text()]
    assert len(articles) == 1, "Expected exactly one article with marked Rust excerpt"
    excerpt = articles[0].read_text().split("<!-- BEGIN RUST EXCERPT -->", 1)[1].split("<!-- END RUST EXCERPT -->", 1)[0].strip()
    assert excerpt == "```rust\n" + snippet + "\n```", "Published Rust excerpt differs from tested source"
    print("PASS: article Rust excerpt exactly matches the compiled source")


def reproduce(reference_rows):
    environment = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target"))
    subprocess.run(["cargo", "build", "--release", "--locked", "--offline"], cwd=ROOT, env=environment, check=True)
    binaries = [path for path in (ROOT / "target/release").iterdir()
                if path.is_file() and path.stat().st_mode & 0o111 and "." not in path.name]
    assert len(binaries) == 1, f"Expected one release executable: {binaries}"
    stable = lambda rows: [{key: value for key, value in row.items() if key != "elapsed_ns"} for row in rows]
    with tempfile.TemporaryDirectory(prefix="annealing-verification-") as directory:
        runs = []
        for index in range(2):
            destination = Path(directory) / f"run-{index}.csv"
            subprocess.run([str(binaries[0]), str(destination)], cwd=ROOT, check=True)
            rows = read_rows(destination)
            check_rows(rows)
            runs.append(stable(rows))
        assert runs[0] == runs[1] == stable(reference_rows), "Reproduction differs outside elapsed time"
    print("PASS: two fresh executions reproduce all committed result fields except elapsed time")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", nargs="?", type=Path, default=ROOT / "results/raw.csv")
    parser.add_argument("--reproduce", action="store_true")
    args = parser.parse_args()
    rows = read_rows(args.csv)
    check_rows(rows)
    check_neighborhood()
    check_text()
    check_excerpt()
    if args.reproduce:
        reproduce(rows)


if __name__ == "__main__":
    main()
