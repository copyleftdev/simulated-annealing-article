#!/usr/bin/env python3
"""Regenerate descriptive statistics and optional figures from actual Rust output."""

import argparse
import csv
import json
import math
import os
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parents[1]
METHODS = ("annealing", "greedy", "random", "grid")
LABELS = {"annealing": "Annealing", "greedy": "Greedy", "random": "Random", "grid": "Coarse grid"}


def load():
    with (ROOT / "results/raw.csv").open(newline="") as stream:
        rows = list(csv.DictReader(stream))
    assert len(rows) == 120, "expected 30 rows for each of four methods"
    summary = {}
    for method in METHODS:
        selected = [row for row in rows if row["method"] == method]
        assert len(selected) == 30
        assert {int(row["seed"]) for row in selected} == set(range(1, 31))
        assert all(int(row["evaluations"]) == 256 for row in selected)
        scores = [float(row["best_score"]) for row in selected]
        summary[method] = {
            "runs": 30,
            "independent_random_runs": 0 if method == "grid" else 30,
            "evaluations_per_run": 256,
            "median_score": statistics.median(scores),
            "min_score": min(scores),
            "max_score": max(scores),
            "median_gap": statistics.median(float(row["gap"]) for row in selected),
            "oracle_hits": sum(abs(float(row["gap"])) <= 1e-9 for row in selected),
            "median_shifted_score": statistics.median(float(row["held_out_score"]) for row in selected),
            "median_accepted_uphill": statistics.median(int(row["accepted_uphill"]) for row in selected),
        }
    return rows, summary


def table(summary):
    lines = [
        "| Method | Median best score | Best to worst score | Exact grid optimum hits |",
        "| --- | ---: | ---: | ---: |",
    ]
    for method, data in summary.items():
        hits = f"{data['oracle_hits']}/30" if method != "grid" else ("Yes" if data["oracle_hits"] else "No") + " (deterministic)"
        lines.append(
            f"| {LABELS[method]} | {data['median_score']:.3f} | "
            f"{data['min_score']:.3f} to {data['max_score']:.3f} | {hits} |"
        )
    return "\n".join(lines)


def plots(rows):
    os.environ.setdefault("MPLCONFIGDIR", "/tmp/annealing-matplotlib")
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import numpy as np

    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 12, "axes.spines.top": False, "axes.spines.right": False})
    out = ROOT / "assets"
    out.mkdir(exist_ok=True)
    fig, ax = plt.subplots(figsize=(10, 5.8), layout="constrained")
    colors = ["#166b87", "#a34d20", "#4c7441", "#71558f"]
    for index, method in enumerate(METHODS):
        values = [float(row["best_score"]) for row in rows if row["method"] == method]
        if method == "grid":
            ax.scatter([index], [values[0]], marker="D", s=95, color=colors[index], zorder=3)
        else:
            jitter = [((seed * 17) % 31 - 15) / 110 for seed in range(30)]
            ax.scatter([index + x for x in jitter], values, color=colors[index], alpha=0.65, s=34)
        median = statistics.median(values)
        ax.plot([index - 0.23, index + 0.23], [median, median], color="#172c39", linewidth=3)
    optimum = float(rows[0]["oracle_score"])
    ax.axhline(optimum, linestyle="--", color="#555555", linewidth=1, label=f"Discrete optimum: {optimum:.3f}")
    ax.set_xticks(range(4), [LABELS[m] for m in METHODS])
    ax.set_ylabel("Best objective score found (lower is better)")
    ax.set_title("One landscape. Equal evaluation budgets.", loc="left", weight="bold", pad=20)
    ax.grid(axis="y", alpha=0.18)
    ax.legend(loc="upper right", frameon=False)
    fig.supxlabel("256 evaluations per run; seeds 1 to 30. Bars show medians.\nGrid diamond is one deterministic result. Data: results/raw.csv", fontsize=10)
    fig.savefig(out / "search-comparison.png", dpi=180)
    fig.savefig(out / "search-comparison.svg")
    plt.close(fig)

    x = np.array([-5.12 + 10.24 * i / 63 for i in range(64)])
    xx, yy = np.meshgrid(x, x)
    energy = xx * xx + yy * yy + 20 - 10 * np.cos(2 * math.pi * xx) - 10 * np.cos(2 * math.pi * yy)
    fig, axes = plt.subplots(1, 2, figsize=(11, 4.6), layout="constrained")
    heat = axes[0].imshow(energy, origin="lower", extent=(-5.12, 5.12, -5.12, 5.12), cmap="viridis_r")
    axes[0].set(xlabel="Parameter x", ylabel="Parameter y", title="The 64 by 64 search landscape")
    fig.colorbar(heat, ax=axes[0], label="Objective score")
    slice_score = x*x + x[31]*x[31] + 20 - 10*np.cos(2*math.pi*x) - 10*math.cos(2*math.pi*x[31])
    axes[1].plot(x, slice_score, marker=".", markersize=3, color="#166b87", linewidth=1.5)
    axes[1].set(xlabel="Parameter x", ylabel="Objective score", title="One slice contains many local valleys")
    axes[1].grid(alpha=0.18)
    fig.supxlabel("Synthetic Rastrigin objective. Slice holds parameter y at index 31. Lower scores are better.", fontsize=10)
    fig.savefig(out / "landscape.png", dpi=180)
    fig.savefig(out / "landscape.svg")
    plt.close(fig)


def replace_block(text, name, body):
    start = f"<!-- BEGIN {name} -->"
    end = f"<!-- END {name} -->"
    before, tail = text.split(start, 1)
    _, after = tail.split(end, 1)
    return before + start + "\n" + body.strip() + "\n" + end + after


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plots", action="store_true", help="requires Matplotlib and NumPy")
    parser.add_argument("--update-article", action="store_true", help="refresh exact source excerpt and generated results table")
    args = parser.parse_args()
    rows, summary = load()
    (ROOT / "results/summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    report = "# Measured synthetic search results\n\n" + table(summary) + "\n\n"
    report += "Each method receives 256 objective calls. Stochastic rows summarize seeds 1 through 30. "
    report += "The grid result is deterministic and repeated for CSV uniformity. Ranges are observed minima and maxima, not confidence intervals.\n\n"
    report += "| Method | Median gap to oracle | Median shifted score | Median accepted uphill moves |\n| --- | ---: | ---: | ---: |\n"
    for method, data in summary.items():
        report += f"| {LABELS[method]} | {data['median_gap']:.3f} | {data['median_shifted_score']:.3f} | {data['median_accepted_uphill']:.1f} |\n"
    report += "\nThe shifted score averages four combinations of coordinate offsets of plus or minus 0.1. It is calculated only after candidate selection. It is a deterministic sensitivity check.\n"
    (ROOT / "results/summary.md").write_text(report)
    if args.update_article:
        article_path = ROOT / "article.md"
        article = article_path.read_text()
        source = (ROOT / "src/lib.rs").read_text()
        excerpt = source.split("// ARTICLE_SNIPPET_START\n", 1)[1].split("// ARTICLE_SNIPPET_END", 1)[0].strip()
        article = replace_block(article, "RUST EXCERPT", "```rust\n" + excerpt + "\n```")
        article = replace_block(article, "RESULTS", table(summary))
        article_path.write_text(article)
    if args.plots:
        plots(rows)
    print(report)


if __name__ == "__main__":
    main()
