use std::fs::File;
use std::io::Write;
use std::iter::from_fn as iter_fn;

use clap::Parser;
use fastrand::usize as random_usize;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
struct Args {
    #[arg(short, long, help = "Random seed for reproducibility")]
    seed: Option<u64>,
    #[arg(short, long, help = "Number of instances to generate")]
    count: usize,
    #[arg(short, long, help = "Output file for JSON instances")]
    output: String,
    #[arg(name = "places", help = "Number of places")]
    nplaces: usize,
    #[arg(name = "timeslots", help = "Number of time slots")]
    ntimes: usize,
    #[arg(name = "activities", help = "Number of activities")]
    nactivities: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Activity {
    pub priority: usize,
    pub topic: usize,
}

impl Activity {
    fn random() -> Self {
        let priority = random_usize(0..=100);
        let topic = random_usize(1..=3);
        Self { priority, topic }
    }

    fn randoms(mut n: usize) -> impl Iterator<Item = Self> {
        iter_fn(move || {
            if n > 0 {
                n -= 1;
                Some(Self::random())
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulingInstance {
    pub id: String,
    pub nplaces: usize,
    pub ntimes: usize,
    pub activities: Vec<Activity>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if let Some(seed) = args.seed {
        fastrand::seed(seed);
    }

    let mut instances = Vec::new();
    
    for i in 0..args.count {
        let instance = SchedulingInstance {
            id: format!("instance_{:03}", i),
            nplaces: args.nplaces,
            ntimes: args.ntimes,
            activities: Activity::randoms(args.nactivities).collect(),
        };
        instances.push(instance);
    }

    let json = serde_json::to_string_pretty(&instances)?;
    let mut file = File::create(&args.output)?;
    file.write_all(json.as_bytes())?;

    println!("Generated {} instances and saved to {}", args.count, args.output);
    Ok(())
}