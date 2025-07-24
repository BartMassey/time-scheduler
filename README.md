# Time Scheduler

A flexible Rust library for scheduling activities in time
slots with customizable penalty functions. This library
provides generic types and local search algorithms for
solving scheduling problems where activities need to be
assigned to time slots and locations while minimizing a
penalty function.

The library is designed to be adaptable for various
scheduling scenarios including conference scheduling,
resource allocation, and timetabling problems.

## Repository Organization

This repository is organized as a Cargo workspace with the
following components:

* **`time-scheduler/`** - The core library crate providing
  generic scheduling types and algorithms
* **`ts-gen/`** - A utility binary for generating scheduling
  problem instances with configurable distributions
* **`time-scheduler/examples/`** - Example implementations
  showing how to use the library:
  * `conference-scheduler.rs` - A complete conference
    scheduling example with penalty functions
  * `instance.json` - Sample scheduling instance for testing
  * `README.md` - Detailed usage documentation

## Quick Start

### Using the Library

Add the library to your `Cargo.toml`:

```toml
[dependencies]
time-scheduler = { path = "time-scheduler" }
```

Define your activity type and create a schedule:

```rust
use time_scheduler::{Schedule, SchedulingInstance};

#[derive(Clone, Debug)]
struct Meeting {
    priority: usize,
    topic: usize,
}

// Create a scheduling instance
let instance = SchedulingInstance {
    id: "my-schedule".to_string(),
    nplaces: 3,  // 3 rooms
    ntimes: 5,   // 5 time slots
    activities: vec![
        Meeting { priority: 10, topic: 1 },
        Meeting { priority: 8, topic: 2 },
        // ... more meetings
    ],
};

// Create and optimize the schedule
let mut schedule = Schedule::new(
    instance.nplaces,
    instance.ntimes,
    instance.activities.into_iter(),
);

// Define penalty function returning (unscheduled_count, other_penalty)
let penalty_fn = |schedule: &Schedule<Meeting>| {
    let unscheduled_count = schedule.get_unscheduled_activities().count() 
                          + schedule.empty_slots_count();
    let priority_penalty = schedule.get_unscheduled_activities()
        .map(|m| m.priority as f32)
        .sum::<f32>();
    (unscheduled_count, priority_penalty)
};

// Improve with restarts and noise using builder pattern
schedule.improve(penalty_fn).with_noise().restarts(5).run();

// Or with defaults (no noise, no restarts)
schedule.improve(penalty_fn).run();
```

### Running the Example

Try the conference scheduling example:

```bash
# Run with the provided sample instance
cargo run --example conference-scheduler time-scheduler/examples/instance.json

# Generate your own instances
cargo run --bin ts-gen -- --count 3 --output my-instances.json 4 8 30 --unconference

# Run with custom optimization parameters
cargo run --example conference-scheduler my-instances.json --nrestarts 10 --noise
```

### Generating Problem Instances

Use the `ts-gen` utility to create scheduling problem instances:

```bash
# Generate 5 instances with 3 places, 7 time slots, 25 activities
cargo run --bin ts-gen -- --count 5 --output instances.json 3 7 25

# Use unconference preset (priorities 1-50, 8 topics, specific distributions)
cargo run --bin ts-gen -- --count 1 --output unconference.json 3 7 25 --unconference

# Customize distributions
cargo run --bin ts-gen -- --count 1 --output custom.json 4 6 20 \
    --priority-dist "zipf:1.5" --topic-dist "pareto:2.0:1.0"
```

## Documentation

- **Library API**: Run `cargo doc --open` to view the comprehensive API documentation
- **Examples**: See `time-scheduler/examples/README.md` for detailed usage examples
- **Algorithm Details**: The rustdoc contains detailed explanations of the local search algorithms

## Development

This project was developed with assistance from
[Claude Code](https://claude.ai/code), Anthropic's coding
assistant. The git commit history contains detailed
attribution for AI-assisted contributions.

## License

This work is made available under the "Apache 2.0 or MIT
License". See the file `LICENSE.txt` in this distribution for
license terms.
