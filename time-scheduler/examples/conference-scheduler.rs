use std::collections::HashMap;
use std::fs;

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
        help = "Number of restarts (0 = no restarts)"
    )]
    restarts: Option<usize>,
    #[arg(help = "JSON file containing problem instances")]
    instances_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Activity {
    pub priority: usize,
    pub topic: usize,
}

fn activity_penalty(schedule: &Schedule<Activity>) -> f32 {
    let mut penalty = 0.0;

    let missed_out = schedule
        .get_unscheduled_activities()
        .map(|a| 1.0 * a.priority as f32)
        .sum::<f32>();
    penalty += missed_out;

    let nempty = schedule.empty_slots_count();
    penalty += 10_000.0 * nempty as f32;

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
    penalty += priority_conflicts + topic_conflicts;

    let mut lateness = 0.0;
    for (t, c) in schedule.slots().axis_iter(Axis(0)).enumerate() {
        for a in c.into_iter().flatten() {
            lateness += 0.1 * a.priority as f32 * t as f32;
        }
    }
    penalty += lateness;

    penalty
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let file_contents = fs::read_to_string(&args.instances_file)?;
    let instances: Vec<SchedulingInstance<Activity>> = serde_json::from_str(&file_contents)?;

    for instance in instances {
        println!("Processing instance: {}", instance.id);
        let mut schedule = Schedule::new(
            instance.nplaces,
            instance.ntimes,
            instance.activities.into_iter(),
        );

        let initial_penalty = activity_penalty(&schedule);
        schedule.improve(activity_penalty, args.nswaps, args.noise, args.restarts);
        let final_penalty = activity_penalty(&schedule);

        println!("  Initial penalty: {initial_penalty:.2}");
        println!("  Final penalty:   {final_penalty:.2}");
        println!("  Improvement:     {:.2}", initial_penalty - final_penalty);
        println!();
    }

    Ok(())
}
