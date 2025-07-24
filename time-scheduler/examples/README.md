# Examples

This directory contains examples demonstrating how to use
the time-scheduler library.

## Conference Scheduler Example

The `conference-scheduler.rs` example shows how to implement
a scheduling system for conferences or unconferences with
activity priorities and topic conflicts.

### Usage

```bash
# Run with the provided example instance
cargo run --example conference-scheduler instance.json

# Run with custom parameters
cargo run --example conference-scheduler instance.json --nswaps 1000 --nrestarts 5 --noise

# Generate your own instances using ts-gen
cargo run --bin ts-gen -- --count 5 --output my-instances.json 3 7 25 --unconference
cargo run --example conference-scheduler my-instances.json
```

### Arguments

- `--nswaps <N>` - Number of swaps per restart (default: 5 * total_slots^2)
- `--nrestarts <N>` - Total number of runs including restarts (default: 1)
- `--proportional` - Divide total swap budget across restarts for fair comparison
- `--noise` - Use noise moves to explore more solutions

### Example Instance Format

The `instance.json` file contains a sample 3×7 grid
unconference instance with 25 activities. Each activity has:

- `priority` - Importance/popularity (1-50 for unconference preset)
- `topic` - Category/track (1-8 topics for unconference preset)

The penalty function returns a tuple `(unscheduled_count, other_penalty)` balancing:
- **Unscheduled items** - Count of unscheduled activities plus empty slots (first tuple element)
- **Other penalties** - Combined penalties for conflicts and preferences (second tuple element):
  - **Missed priorities** - Unscheduled high-priority activities
  - **Topic conflicts** - Activities in same topic scheduled simultaneously  
  - **Priority conflicts** - High-priority activities competing for same time
  - **Lateness penalty** - Earlier time slots preferred

### Customizing the Penalty Function

To create your own scheduling application:

1. Define your activity type

2. Implement a penalty function returning `(usize, f32)` tuple using
   `schedule.get_activity_at()`,
   `schedule.get_unscheduled_activities()`, etc.

3. Use the builder pattern: `schedule.improve(your_penalty_fn).run()`

Example penalty function structure:
```rust
let penalty_fn = |schedule: &Schedule<YourActivity>| {
    let unscheduled_count = schedule.get_unscheduled_activities().count() 
                          + schedule.empty_slots_count();
    let other_penalties = /* your custom penalty calculation */;
    (unscheduled_count, other_penalties)
};
```

See the source code for a complete example of penalty function implementation.
