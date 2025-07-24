use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use clap::Parser;
use ndarray::Axis;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};
use time_scheduler::{Schedule, SchedulingInstance};

#[derive(Parser)]
struct Args {
    #[arg(short = 's', long = "nswaps", help = "Number of swaps per restart")]
    nswaps: Option<usize>,
    #[arg(short = 'n', long = "noise", help = "Use noise moves")]
    noise: bool,
    #[arg(
        short = 'r',
        long = "nrestarts",
        help = "Total number of runs (0-1 = single run, 2+ = with restarts)"
    )]
    restarts: Option<usize>,
    #[arg(
        short = 'p',
        long = "proportional",
        help = "Divide total swap budget across restarts for fair comparison"
    )]
    proportional: bool,
    #[arg(
        short = 't',
        long = "timeout",
        help = "Runtime timeout in seconds"
    )]
    timeout: Option<u64>,
    #[arg(
        long = "json",
        help = "Output results in JSON format for script parsing"
    )]
    json: bool,
    #[arg(help = "JSON file containing problem instances")]
    instances_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Activity {
    pub priority: usize,
    pub topic: usize,
}

#[derive(Serialize)]
struct RunResult {
    instance_id: String,
    initial_unscheduled: usize,
    initial_other_penalty: f32,
    final_unscheduled: usize,
    final_other_penalty: f32,
    unscheduled_improvement: i32,
    other_improvement: f32,
    config: RunConfig,
}

#[derive(Serialize)]
struct RunConfig {
    noise: bool,
    restarts: Option<usize>,
    proportional: bool,
    timeout: Option<u64>,
    nswaps: Option<usize>,
}

fn activity_penalty(schedule: &Schedule<Activity>) -> (usize, f32) {
    let nunscheduled = schedule.get_unscheduled_activities().count();
    let nempty = schedule.empty_slots_count();
    
    let mut other_penalty = 0.0;

    // Penalty for unscheduled activities based on their priority
    let missed_out = schedule
        .get_unscheduled_activities()
        .map(|a| 1.0 * a.priority as f32)
        .sum::<f32>();
    other_penalty += missed_out;

    // Priority and topic conflicts within time slots
    let mut topic_conflicts = 0.0;
    let mut priority_conflicts = 0.0;
    for r in schedule.slots().axis_iter(Axis(1)) {
        let mut vars: Vec<_> = r
            .iter()
            .filter_map(|a| a.as_ref())
            .map(|a| {
                let p = a.priority as f32;
                p * p
            })
            .map(|p| NotNan::new(p).unwrap())
            .collect();
        vars.sort();
        let big3 = vars
            .into_iter()
            .rev()
            .take(3)
            .map(NotNan::into_inner)
            .sum::<f32>();
        priority_conflicts += 1.0 * f32::sqrt(big3);

        let mut topic_counts: HashMap<usize, f32> = HashMap::new();
        for a in r.iter().filter_map(|a| a.as_ref()) {
            *topic_counts.entry(a.topic).or_insert(0.0) += 1.0;
        }
        let tc = topic_counts.values().map(|&c| c * c).sum::<f32>();
        topic_conflicts += 10.0 * tc;
    }
    other_penalty += priority_conflicts + topic_conflicts;

    // Lateness penalty (earlier time slots are preferred)
    let mut lateness = 0.0;
    for (t, c) in schedule.slots().axis_iter(Axis(0)).enumerate() {
        for a in c.into_iter().flatten() {
            lateness += 0.1 * a.priority as f32 * t as f32;
        }
    }
    other_penalty += lateness;

    (nunscheduled + nempty, other_penalty)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let file_contents = fs::read_to_string(&args.instances_file)?;
    let instances: Vec<SchedulingInstance<Activity>> = serde_json::from_str(&file_contents)?;

    let mut results = Vec::new();


    for instance in instances {
        
        let mut schedule = Schedule::new(
            instance.nplaces,
            instance.ntimes,
            instance.activities.into_iter(),
        );

        let (initial_unscheduled, initial_other_penalty) = activity_penalty(&schedule);

        // Use the new builder API
        let mut improver = schedule.improve(activity_penalty);
        if let Some(nswaps) = args.nswaps {
            improver = improver.max_swaps(nswaps);
        }
        if args.noise {
            improver = improver.with_noise();
        }
        if let Some(restarts) = args.restarts {
            if args.proportional {
                improver = improver.restarts_proportional(restarts);
            } else {
                improver = improver.restarts(restarts);
            }
        }
        if let Some(timeout_secs) = args.timeout {
            improver = improver.timeout(Duration::from_secs(timeout_secs));
        }
        improver.run();

        let (final_unscheduled, final_other_penalty) = activity_penalty(&schedule);
        let unscheduled_improvement = initial_unscheduled as i32 - final_unscheduled as i32;
        let other_improvement = initial_other_penalty - final_other_penalty;

        if args.json {
            results.push(RunResult {
                instance_id: instance.id,
                initial_unscheduled,
                initial_other_penalty,
                final_unscheduled,
                final_other_penalty,
                unscheduled_improvement,
                other_improvement,
                config: RunConfig {
                    noise: args.noise,
                    restarts: args.restarts,
                    proportional: args.proportional,
                    timeout: args.timeout,
                    nswaps: args.nswaps,
                },
            });
        } else {
            println!("{} unscheduled:{}->{} other:{:.2}->{:.2} improvements:{},{:.2}", 
                     instance.id, 
                     initial_unscheduled, final_unscheduled,
                     initial_other_penalty, final_other_penalty,
                     unscheduled_improvement, other_improvement);
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    }

    Ok(())
}
