#!/usr/bin/env python3
"""Run a complete src/lib.rs mutation campaign in an isolated temporary copy."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def summarize(output, suite):
    raw = json.loads((output / "outcomes.json").read_text())
    records = []
    baseline = []
    for entry in raw["outcomes"]:
        if entry["scenario"] == "Baseline":
            baseline.append(entry["summary"])
            continue
        mutant = entry["scenario"]["Mutant"]
        log = (output / entry["log_path"]).read_text()
        records.append({
            "name": mutant["name"],
            "outcome": entry["summary"],
            "failed_tests": sorted(set(re.findall(r"test ([\w:]+) \.\.\. FAILED", log))),
        })
    assert baseline == ["Success"], f"Unmutated baseline failed: {baseline}"
    assert len(records) == raw["total_mutants"]
    assert len(records) == len(json.loads((output / "mutants.json").read_text())), "Incomplete campaign"
    assert sum(raw[key] for key in ("caught", "missed", "unviable", "timeout")) == len(records)
    result = {
        "suite": suite,
        "tool": "cargo-mutants",
        "version": raw["cargo_mutants_version"],
        "scope": "Every mutation generated for src/lib.rs; no exclusions. " + (
            "CLI is exercised but not mutated." if suite == "expanded" else "Only the eight inline library tests run; CLI is not exercised."
        ),
        "test_timeout_seconds": 10,
        "build_timeout_seconds": 60,
        "jobs": 4,
        "source_sha256": hashlib.sha256((ROOT / "src/lib.rs").read_bytes()).hexdigest(),
        "test_source_sha256": {
            str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in ([ROOT / "src/lib.rs"] + (sorted((ROOT / "tests").glob("*.rs")) if suite == "expanded" else []))
        },
        "counts": {key: raw[key] for key in ("total_mutants", "caught", "missed", "unviable", "timeout")},
        "baseline": baseline[0],
        "mutants": records,
    }
    destination = ROOT / "results" / f"mutation-{suite}.json"
    destination.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result["counts"]))
    print(f"Saved {destination.relative_to(ROOT)}; inspect survivors separately from build failures and timeouts.")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=("original", "expanded"), default="expanded")
    args = parser.parse_args()
    # Original uses the unchanged eight inline tests. Expanded adds integration tests.
    with tempfile.TemporaryDirectory(prefix="annealing-mutations-") as temporary:
        directory = Path(temporary)
        project = directory / "project"
        project.mkdir()
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(ROOT / name, project / name)
        shutil.copytree(ROOT / "src", project / "src")
        if args.suite == "expanded":
            shutil.copytree(ROOT / "tests", project / "tests")
            (project / "results").mkdir()
            shutil.copy2(ROOT / "results/raw.csv", project / "results/raw.csv")
        command = [
            "cargo", "mutants", "--dir", str(project), "--file", "src/lib.rs",
            "--timeout", "10", "--build-timeout", "60", "--jobs", "4",
            "--cargo-arg=--offline", "--cargo-arg=--locked",
            "--output", str(directory / "output"), "--colors", "never",
        ]
        completed = subprocess.run(command, env=dict(os.environ, CARGO_TARGET_DIR="target"), check=False)
        # A completed campaign can exit nonzero because of surviving mutants or timeouts.
        # The report, rather than a zero wrapper exit, describes the testing outcome.
        output = directory / "output/mutants.out"
        if not (output / "outcomes.json").exists():
            raise SystemExit(completed.returncode or 1)
        summarize(output, args.suite)


if __name__ == "__main__":
    main()
