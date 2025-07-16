use std::collections::HashSet;
use std::iter::from_fn as iter_fn;

use fastrand::usize as random_usize;
use ndarray::{Array2, Axis};

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

#[derive(Debug)]
struct Schedule {
    slots: Array2<Option<Activity>>,
    unscheduled: HashSet<Activity>,
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
        
        let unscheduled = acts.into_iter().collect();

        Self { slots, unscheduled }
    }
}

fn main() {
    let _schedule = Schedule::new(3, 2, Activity::randoms(8));
    println!("{_schedule:?}");
}
