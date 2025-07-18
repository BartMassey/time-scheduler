use std::iter::from_fn as iter_fn;

use clap::Parser;
use fastrand::usize as random_usize;
use modern_multiset::HashMultiSet;
use ndarray::{Array2, Axis};
use ordered_float::NotNan;

#[derive(Parser)]
struct Args {
    #[arg(short='n', long="nswaps", help="Number of swaps")]
    nswaps: Option<usize>,
    #[arg(name="places", help="Number of places")]
    nplaces: usize,
    #[arg(name="timeslots", help="Number of time slots")]
    ntimes: usize,
    #[arg(name="activities", help="Number of activities")]
    nactivities: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Activity {
    priority: usize,
    topic: usize,
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

#[derive(Debug, Clone)]
struct Schedule {
    slots: Array2<Option<Activity>>,
    unscheduled: Vec<Option<Activity>>,
}

impl Schedule {
    fn new<I>(nplaces: usize, ntimes: usize, activities: I) -> Self
    where
        I: Iterator<Item = Activity>
    {
        let mut acts = activities.fuse();

        let mut slots = Array2::from_elem((nplaces, ntimes), None);
        for x in &mut slots {
            if let Some(a) = acts.next() {
                *x = Some(a)
            } else {
                break;
            }
        }
        
        let unscheduled = acts.map(Some).collect();

        Self { slots, unscheduled }
    }

    fn penalty(&self) -> f32 {
        let mut penalty = 0.0;

        let missed_out = self.unscheduled
            .iter()
            .flatten()
            .map(|a| 1.0 * a.priority as f32)
            .sum::<f32>();
        penalty += missed_out;

        let nempty = self.slots
            .iter()
            .filter(|&a| a.is_none())
            .count();
        penalty += 10_000.0 * nempty as f32;

        let mut topic_conflicts = 0.0;
        let mut priority_conflicts = 0.0;
        for r in self.slots.axis_iter(Axis(1)) {
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
            
            let h: HashMultiSet<_> = r
                .iter()
                .filter_map(|a| a.as_ref())
                .map(|a| a.topic)
                .collect();
            let tc = h
                .distinct_elements()
                .map(|t| {
                    let c = h.count_of(t) as f32;
                    c * c
                })
                .sum::<f32>();
            topic_conflicts += 10.0 * tc;
        }
        penalty += priority_conflicts + topic_conflicts;

        let mut lateness = 0.0;
        for (t, c) in self.slots.axis_iter(Axis(0)).enumerate() {
            for a in c.into_iter().flatten() {
                lateness += 0.1 * a.priority as f32 * t as f32;
            }
        }
        penalty += lateness;

        penalty
    }

    fn improve(&mut self, nswaps: Option<usize>) {
        let self_p = self as *const Schedule;

        let slot_locs = self.slots.iter_mut();
        let unscheduled_locs = self.unscheduled.iter_mut();
        let mut locs: Vec<_> = slot_locs.chain(unscheduled_locs).collect();
        
        let ntotal = locs.len();
        let nswaps = nswaps.unwrap_or(2 * usize::pow(ntotal, 3));

        fn swap(locs: &mut [&mut Option<Activity>], s1: usize, s2: usize) {
            let y1 = locs[s1].take();
            let y2 = locs[s2].take();
            *(locs[s1]) = y2;
            *(locs[s2]) = y1;
        }

        // # Safety
        // There can be no mutation of the underlying objects
        // by penalty(), so the state can be safely used.
        // # Need
        // I know of no way to avoid this hack short of reconstructing
        // the schedule at every search step. This would incur
        // unacceptable performance.
        macro_rules! get_penalty {
            ($self_p:expr) => {
                unsafe {
                    let r = &*$self_p;
                    r.penalty()
                }
            };
        }

        let mut penalty = get_penalty!(self_p);
        for _ in 0..nswaps {
            let mut cur_best = (0, 0);
            let mut cur_penalty = penalty;
            for i in 0..ntotal {
                for j in i + 1..ntotal {
                    swap(&mut locs, i, j);
                    let new_penalty = get_penalty!(self_p);
                    if cur_penalty > new_penalty {
                        cur_best = (i, j);
                        cur_penalty = new_penalty;
                    }
                    swap(&mut locs, j, i);
                }
            }
            if cur_penalty < penalty {
                swap(&mut locs, cur_best.0, cur_best.1);
                penalty = cur_penalty;
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let mut schedule = Schedule::new(
        args.nplaces,
        args.ntimes,
        Activity::randoms(args.nactivities),
    );
    println!("{}", schedule.penalty());
    schedule.improve(args.nswaps);
    println!("{}", schedule.penalty());
}
