use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Instant;

#[derive(Parser)]
#[command(about = "Evaluate scheduler performance across multiple configurations")]
struct Args {
    #[arg(help = "JSON file containing problem instances")]
    instances_file: String,

    #[arg(
        short = 't',
        long = "timeout",
        help = "Runtime timeout in seconds for each run",
        default_value = "3"
    )]
    timeout: u64,

    #[arg(
        short = 's',
        long = "nswaps",
        help = "Number of swaps per run (optional)"
    )]
    nswaps: Option<usize>,

    #[arg(
        long = "restarts",
        help = "Comma-separated list of restart counts to test",
        default_value = "1,2,5"
    )]
    restarts: String,

    #[arg(
        long = "noise",
        help = "Test with noise moves enabled",
        action = clap::ArgAction::SetTrue
    )]
    noise: bool,

    #[arg(
        long = "no-proportional",
        help = "Disable proportional resource division (enabled by default)",
        action = clap::ArgAction::SetTrue
    )]
    no_proportional: bool,

    #[arg(
        long = "repeat",
        help = "Number of times to repeat each configuration for statistics",
        default_value = "1"
    )]
    repeat: usize,

    #[arg(long = "json", help = "Output results in JSON format")]
    json: bool,
}

#[derive(Serialize, Deserialize)]
struct RunResult {
    instance_id: String,
    initial_penalty: f32,
    final_penalty: f32,
    improvement: f32,
    config: RunConfig,
}

#[derive(Serialize, Deserialize)]
struct RunConfig {
    noise: bool,
    restarts: Option<usize>,
    proportional: bool,
    timeout: Option<u64>,
    nswaps: Option<usize>,
}

#[derive(Serialize)]
struct EvaluationResult {
    config: ConfigDescription,
    stats: Statistics,
    runs: Vec<RunResult>,
}

#[derive(Serialize)]
struct ConfigDescription {
    noise: bool,
    restarts: usize,
    proportional: bool,
    timeout: u64,
    nswaps: Option<usize>,
}

#[derive(Serialize)]
struct Statistics {
    mean_improvement: f32,
    std_improvement: f32,
    mean_final_penalty: f32,
    std_final_penalty: f32,
    success_rate: f32, // percentage of runs that found improvements
}

fn run_scheduler(
    instances_file: &str,
    config: &ConfigDescription,
) -> Result<Vec<RunResult>, Box<dyn std::error::Error>> {
    let mut cmd = Command::new("cargo");
    cmd.args(&["run", "--release", "--example", "conference-scheduler"])
        .arg(instances_file)
        .arg("--json")
        .arg("--timeout")
        .arg(config.timeout.to_string());

    if config.noise {
        cmd.arg("--noise");
    }

    if config.restarts > 1 {
        cmd.arg("--nrestarts").arg(config.restarts.to_string());
    }

    if config.proportional {
        cmd.arg("--proportional");
    }

    if let Some(nswaps) = config.nswaps {
        cmd.arg("--nswaps").arg(nswaps.to_string());
    }

    let output = cmd.output()?;

    if !output.status.success() {
        return Err(format!(
            "Scheduler failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let results: Vec<RunResult> = serde_json::from_slice(&output.stdout)?;
    Ok(results)
}

fn calculate_statistics(results: &[Vec<RunResult>]) -> Statistics {
    let improvements: Vec<f32> = results
        .iter()
        .flat_map(|run_results| run_results.iter().map(|r| r.improvement))
        .collect();

    let final_penalties: Vec<f32> = results
        .iter()
        .flat_map(|run_results| run_results.iter().map(|r| r.final_penalty))
        .collect();

    let mean_improvement = improvements.iter().sum::<f32>() / improvements.len() as f32;
    let mean_final_penalty = final_penalties.iter().sum::<f32>() / final_penalties.len() as f32;

    let std_improvement = {
        let variance = improvements
            .iter()
            .map(|x| (x - mean_improvement).powi(2))
            .sum::<f32>()
            / improvements.len() as f32;
        variance.sqrt()
    };

    let std_final_penalty = {
        let variance = final_penalties
            .iter()
            .map(|x| (x - mean_final_penalty).powi(2))
            .sum::<f32>()
            / final_penalties.len() as f32;
        variance.sqrt()
    };

    let success_count = improvements.iter().filter(|&&x| x > 0.0).count();
    let success_rate = (success_count as f32 / improvements.len() as f32) * 100.0;

    Statistics {
        mean_improvement,
        std_improvement,
        mean_final_penalty,
        std_final_penalty,
        success_rate,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let restart_counts: Vec<usize> = args
        .restarts
        .split(',')
        .map(|s| s.trim().parse())
        .collect::<Result<Vec<_>, _>>()?;

    let mut all_results = Vec::new();

    for &restarts in &restart_counts {
        let config = ConfigDescription {
            noise: args.noise,
            restarts,
            proportional: !args.no_proportional,
            timeout: args.timeout,
            nswaps: args.nswaps,
        };

        if !args.json {
            println!(
                "Testing config: restarts={}, noise={}, proportional={}, timeout={}s{}",
                restarts,
                config.noise,
                config.proportional,
                config.timeout,
                if let Some(s) = config.nswaps {
                    format!(", nswaps={}", s)
                } else {
                    String::new()
                }
            );
        }

        let mut runs = Vec::new();
        let start_time = Instant::now();

        for run in 1..=args.repeat {
            if !args.json {
                print!("  Run {}/{}...", run, args.repeat);
                std::io::Write::flush(&mut std::io::stdout())?;
            }

            let run_results = run_scheduler(&args.instances_file, &config)?;
            runs.push(run_results);

            if !args.json {
                println!(" done");
            }
        }

        let stats = calculate_statistics(&runs);
        let elapsed = start_time.elapsed();

        if !args.json {
            println!("  Results:");
            println!(
                "    Mean improvement: {:.2} ± {:.2}",
                stats.mean_improvement, stats.std_improvement
            );
            println!(
                "    Mean final penalty: {:.2} ± {:.2}",
                stats.mean_final_penalty, stats.std_final_penalty
            );
            println!("    Success rate: {:.1}%", stats.success_rate);
            println!("    Total time: {:.1}s", elapsed.as_secs_f32());
            println!();
        }

        all_results.push(EvaluationResult {
            config,
            stats,
            runs: runs.into_iter().flatten().collect(),
        });
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&all_results)?);
    } else {
        println!(
            "Evaluation complete! Tested {} configurations.",
            all_results.len()
        );

        // Find best configuration
        if let Some(best) = all_results.iter().max_by(|a, b| {
            a.stats
                .mean_improvement
                .partial_cmp(&b.stats.mean_improvement)
                .unwrap()
        }) {
            println!("Best configuration:");
            println!("  Restarts: {}", best.config.restarts);
            println!("  Noise: {}", best.config.noise);
            println!("  Proportional: {}", best.config.proportional);
            println!("  Mean improvement: {:.2}", best.stats.mean_improvement);
        }
    }

    Ok(())
}
