use annealing_proof::{
    anneal, greedy, grid_search, held_out_score, neighbor, oracle, random_search, training_score,
    INITIAL, SCHEDULE, SEEDS,
};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() > 1 {
        return Err("usage: annealing-proof [output.csv]; omit path for stdout".into());
    }
    let writer: Box<dyn Write> = if let Some(path) = arguments.first() {
        let path = Path::new(path);
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        Box::new(File::create(path)?)
    } else {
        Box::new(io::stdout().lock())
    };
    let mut writer = BufWriter::new(writer);
    let oracle_score = oracle().best_score;
    writeln!(writer, "method,seed,evaluations,best_i,best_j,best_score,held_out_score,accepted_uphill,elapsed_ns,oracle_score,gap")?;
    for seed in 1..=SEEDS {
        for method in ["annealing", "greedy", "random", "grid"] {
            let started = Instant::now();
            let result = match method {
                "annealing" => anneal(INITIAL, training_score, neighbor, seed, SCHEDULE)?,
                "greedy" => greedy(seed)?,
                "random" => random_search(seed)?,
                "grid" => grid_search(),
                _ => unreachable!(),
            };
            let elapsed_ns = started.elapsed().as_nanos();
            // Neither the oracle nor this perturbation score feeds the search.
            let held_out = held_out_score(&result.best);
            writeln!(
                writer,
                "{method},{seed},{},{},{},{:.12},{:.12},{},{elapsed_ns},{oracle_score:.12},{:.12}",
                result.evaluations,
                result.best.i,
                result.best.j,
                result.best_score,
                held_out,
                result.accepted_uphill,
                result.best_score - oracle_score
            )?;
        }
    }
    writer.flush()?;
    Ok(())
}
