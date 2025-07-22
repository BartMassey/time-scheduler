use std::fs::File;
use std::io::Write;
use std::iter::from_fn as iter_fn;
use std::str::FromStr;

use clap::Parser;
use fastrand::{f64 as random_f64, usize as random_usize};
use serde::{Deserialize, Serialize};
use time_scheduler::SchedulingInstance;

#[derive(Debug, Clone)]
enum Distribution {
    Uniform,
    Zipf { exponent: f64 },
    Pareto { shape: f64, scale: f64 },
    Geometric { p: f64 },
}

impl FromStr for Distribution {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts[0].to_lowercase().as_str() {
            "uniform" => Ok(Distribution::Uniform),
            "zipf" => {
                if parts.len() != 2 {
                    return Err("Zipf distribution requires exponent: zipf:1.5".to_string());
                }
                let exponent = parts[1].parse().map_err(|_| "Invalid exponent")?;
                Ok(Distribution::Zipf { exponent })
            }
            "pareto" => {
                if parts.len() != 3 {
                    return Err("Pareto distribution requires shape and scale: pareto:2.0:1.0".to_string());
                }
                let shape = parts[1].parse().map_err(|_| "Invalid shape")?;
                let scale = parts[2].parse().map_err(|_| "Invalid scale")?;
                Ok(Distribution::Pareto { shape, scale })
            }
            "geometric" => {
                if parts.len() != 2 {
                    return Err("Geometric distribution requires probability: geometric:0.3".to_string());
                }
                let p = parts[1].parse().map_err(|_| "Invalid probability")?;
                if p <= 0.0 || p >= 1.0 {
                    return Err("Geometric probability must be between 0 and 1".to_string());
                }
                Ok(Distribution::Geometric { p })
            }
            _ => Err(format!("Unknown distribution: {}. Options: uniform, zipf, pareto, geometric", parts[0]))
        }
    }
}

#[derive(Parser)]
struct Args {
    #[arg(short, long, help = "Random seed for reproducibility")]
    seed: Option<u64>,
    #[arg(short, long, help = "Number of instances to generate")]
    count: usize,
    #[arg(short, long, help = "Output file for JSON instances")]
    output: String,
    #[arg(long, help = "Use unconference preset: priority 1-50 (pareto:1.8:1.0), 8 topics (zipf:1.2)")]
    unconference: bool,
    #[arg(long, default_value = "1", help = "Minimum priority value")]
    min_priority: usize,
    #[arg(long, default_value = "100", help = "Maximum priority value")]
    max_priority: usize,
    #[arg(long, default_value = "5", help = "Number of topic categories")]
    ntopics: usize,
    #[arg(long, default_value = "zipf:1.5", help = "Priority distribution: uniform, zipf:exp, pareto:shape:scale, geometric:p")]
    priority_dist: Distribution,
    #[arg(long, default_value = "pareto:2.0:1.0", help = "Topic distribution: uniform, zipf:exp, pareto:shape:scale, geometric:p")]
    topic_dist: Distribution,
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

impl Distribution {
    fn sample(&self, min: usize, max: usize) -> usize {
        match self {
            Distribution::Uniform => random_usize(min..=max),
            Distribution::Zipf { exponent } => {
                Self::sample_zipf(min, max, *exponent)
            }
            Distribution::Pareto { shape, scale } => {
                Self::sample_pareto(min, max, *shape, *scale)
            }
            Distribution::Geometric { p } => {
                Self::sample_geometric(min, max, *p)
            }
        }
    }
    
    fn sample_zipf(min: usize, max: usize, exponent: f64) -> usize {
        let n = max - min + 1;
        let mut sum = 0.0;
        for i in 1..=n {
            sum += 1.0 / (i as f64).powf(exponent);
        }
        
        let u = random_f64();
        let target = u * sum;
        let mut cumulative = 0.0;
        
        for i in 1..=n {
            cumulative += 1.0 / (i as f64).powf(exponent);
            if cumulative >= target {
                return max - (i - 1); // high rank = high value
            }
        }
        max
    }
    
    fn sample_pareto(min: usize, max: usize, shape: f64, scale: f64) -> usize {
        let u = random_f64();
        let value = scale * ((1.0 - u).powf(-1.0 / shape));
        let normalized = ((value - scale) / (10.0 * scale)).max(0.0).min(1.0);
        min + ((max - min) as f64 * (1.0 - normalized)) as usize
    }
    
    fn sample_geometric(min: usize, max: usize, p: f64) -> usize {
        let u = random_f64();
        let value = ((1.0 - u).ln() / p.ln()).floor() as usize;
        let range = max - min + 1;
        min + (value % range)
    }
}

impl Activity {
    fn random_with_distributions(
        min_priority: usize,
        max_priority: usize,
        ntopics: usize,
        priority_dist: &Distribution,
        topic_dist: &Distribution,
    ) -> Self {
        let priority = priority_dist.sample(min_priority, max_priority);
        let topic = topic_dist.sample(1, ntopics);
        Self { priority, topic }
    }

    fn randoms_with_distributions(
        mut n: usize,
        min_priority: usize,
        max_priority: usize,
        ntopics: usize,
        priority_dist: Distribution,
        topic_dist: Distribution,
    ) -> impl Iterator<Item = Self> {
        iter_fn(move || {
            if n > 0 {
                n -= 1;
                Some(Self::random_with_distributions(
                    min_priority, max_priority, ntopics, &priority_dist, &topic_dist
                ))
            } else {
                None
            }
        })
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();

    if let Some(seed) = args.seed {
        fastrand::seed(seed);
    }

    // Apply unconference preset if requested
    if args.unconference {
        args.min_priority = 1;
        args.max_priority = 50;
        args.ntopics = 8;
        args.priority_dist = Distribution::Pareto { shape: 1.8, scale: 1.0 };
        args.topic_dist = Distribution::Zipf { exponent: 1.2 };
    }

    let mut instances = Vec::new();
    
    for i in 0..args.count {
        let instance = SchedulingInstance::<Activity> {
            id: format!("instance_{:03}", i),
            nplaces: args.nplaces,
            ntimes: args.ntimes,
            activities: Activity::randoms_with_distributions(
                args.nactivities,
                args.min_priority,
                args.max_priority,
                args.ntopics,
                args.priority_dist.clone(),
                args.topic_dist.clone(),
            ).collect(),
        };
        instances.push(instance);
    }

    let json = serde_json::to_string_pretty(&instances)?;
    let mut file = File::create(&args.output)?;
    file.write_all(json.as_bytes())?;

    println!("Generated {} instances and saved to {}", args.count, args.output);
    Ok(())
}