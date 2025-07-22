use std::fs;

use clap::Parser;
use fastrand::usize as random_usize;
use modern_multiset::HashMultiSet;
use ndarray::{Array2, Axis};
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoundsError {
    #[error("Place index {0} is out of bounds")]
    Place(usize),
    #[error("Time index {0} is out of bounds")]
    Time(usize),
}

#[derive(Parser)]
struct Args {
    #[arg(short='s', long="nswaps", help="Number of swaps per restart")]
    nswaps: Option<usize>,
    #[arg(short='n', long="noise", help="Use noise moves")]
    noise: bool,
    #[arg(short='r', long="nrestarts", help="Number of restarts (0 = no restarts)")]
    restarts: Option<usize>,
    #[arg(help="JSON file containing problem instances")]
    instances_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Activity {
    pub priority: usize,
    pub topic: usize,
}

pub trait Penalty {
    fn penalty(&self) -> f32;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulingInstance<A> {
    pub id: String,
    pub nplaces: usize,
    pub ntimes: usize,
    pub activities: Vec<A>,
}

#[derive(Debug, Clone, Copy)]
enum Loc {
    S(usize, usize),      // (place, time) in slots array
    U(usize),             // index in unscheduled vec
}

#[derive(Debug, Clone)]
pub struct Schedule<A> {
    slots: Array2<Option<A>>,
    unscheduled: Vec<Option<A>>,
}

impl<A: Clone> Schedule<A> {
    fn new<I>(nplaces: usize, ntimes: usize, activities: I) -> Self
    where
        I: Iterator<Item = A>
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

    pub fn get_activity_at(&self, place: usize, time: usize) -> Result<Option<&A>, BoundsError> {
        let (nplaces, ntimes) = self.slots.dim();
        if place >= nplaces {
            return Err(BoundsError::Place(place));
        }
        if time >= ntimes {
            return Err(BoundsError::Time(time));
        }
        Ok(self.slots[(place, time)].as_ref())
    }

    pub fn get_unscheduled_activities(&self) -> impl Iterator<Item = &A> {
        self.unscheduled.iter().filter_map(|opt| opt.as_ref())
    }

    pub fn dimensions(&self) -> (usize, usize) {
        self.slots.dim()
    }

    pub fn empty_slots_count(&self) -> usize {
        self.slots.iter().filter(|opt| opt.is_none()).count()
    }

    fn reshuffle(&mut self) {
        // Collect all activities from both slots and unscheduled
        let mut all_activities = Vec::new();
        
        // Collect from slots
        for slot in self.slots.iter_mut() {
            if let Some(activity) = slot.take() {
                all_activities.push(activity);
            }
        }
        
        // Collect from unscheduled
        for unscheduled_slot in self.unscheduled.iter_mut() {
            if let Some(activity) = unscheduled_slot.take() {
                all_activities.push(activity);
            }
        }
        
        // Shuffle the activities
        for i in (1..all_activities.len()).rev() {
            let j = random_usize(0..=i);
            all_activities.swap(i, j);
        }
        
        // Redistribute: fill slots first, then unscheduled
        let mut activity_iter = all_activities.into_iter();
        
        for slot in self.slots.iter_mut() {
            if let Some(activity) = activity_iter.next() {
                *slot = Some(activity);
            }
        }
        
        for unscheduled_slot in self.unscheduled.iter_mut() {
            if let Some(activity) = activity_iter.next() {
                *unscheduled_slot = Some(activity);
            }
        }
    }

    fn swap_locations(&mut self, loc1: Loc, loc2: Loc) {
        use Loc::*;
        
        let activity1 = match loc1 {
            S(p, t) => self.slots[(p, t)].take(),
            U(i) => self.unscheduled[i].take(),
        };
        let activity2 = match loc2 {
            S(p, t) => self.slots[(p, t)].take(),
            U(i) => self.unscheduled[i].take(),
        };
        
        match loc1 {
            S(p, t) => self.slots[(p, t)] = activity2,
            U(i) => self.unscheduled[i] = activity2,
        }
        match loc2 {
            S(p, t) => self.slots[(p, t)] = activity1,
            U(i) => self.unscheduled[i] = activity1,
        }
    }

    fn improve_single(&mut self, nswaps: Option<usize>, noise: bool) 
    where 
        Self: Penalty,
    {
        let (nplaces, ntimes) = self.slots.dim();
        let nunscheduled = self.unscheduled.len();
        let ntotal = nplaces * ntimes + nunscheduled;
        let nswaps = nswaps.unwrap_or(2 * usize::pow(ntotal, 3));
        
        use Loc::*;
        
        
        let all_locations: Vec<Loc> = (0..nplaces)
            .flat_map(|p| (0..ntimes).map(move |t| S(p, t)))
            .chain((0..nunscheduled).map(U))
            .collect();
        
        let mut penalty = self.penalty();
        
        for _ in 0..nswaps {
            if noise && random_usize(0..2) == 0 {
                let i = random_usize(0..ntotal);
                let j = random_usize(0..ntotal);
                self.swap_locations(all_locations[i], all_locations[j]);
                let new_penalty = self.penalty();
                if new_penalty < penalty {
                    penalty = new_penalty;
                } else {
                    self.swap_locations(all_locations[j], all_locations[i]);
                }
                continue;
            }

            let mut cur_best = (0, 0);
            let mut cur_penalty = penalty;
            for i in 0..ntotal {
                for j in i + 1..ntotal {
                    self.swap_locations(all_locations[i], all_locations[j]);
                    let new_penalty = self.penalty();
                    if cur_penalty > new_penalty {
                        cur_best = (i, j);
                        cur_penalty = new_penalty;
                    }
                    self.swap_locations(all_locations[j], all_locations[i]);
                }
            }
            if cur_penalty < penalty {
                self.swap_locations(all_locations[cur_best.0], all_locations[cur_best.1]);
                penalty = cur_penalty;
            }
        }
    }

    pub fn improve(&mut self, nswaps: Option<usize>, noise: bool, restarts: Option<usize>) 
    where 
        Self: Penalty,
    {
        let num_restarts = restarts.unwrap_or(0);
        
        if num_restarts == 0 {
            // No restarts - run original improve method
            self.improve_single(nswaps, noise);
            return;
        }
        
        let mut best_penalty = self.penalty();
        let mut best_schedule = self.clone();
        
        // Run the specified number of restarts
        for restart_num in 0..=num_restarts {
            if restart_num > 0 {
                self.reshuffle();
            }
            
            self.improve_single(nswaps, noise);
            let current_penalty = self.penalty();
            
            if current_penalty < best_penalty {
                best_penalty = current_penalty;
                best_schedule = self.clone();
            }
        }
        
        // Restore the best schedule found across all restarts
        *self = best_schedule;
    }
}

impl Penalty for Schedule<Activity> {
    fn penalty(&self) -> f32 {
        let mut penalty = 0.0;

        let missed_out = self.get_unscheduled_activities()
            .map(|a| 1.0 * a.priority as f32)
            .sum::<f32>();
        penalty += missed_out;

        let nempty = self.empty_slots_count();
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
        
        let initial_penalty = schedule.penalty();
        schedule.improve(args.nswaps, args.noise, args.restarts);
        let final_penalty = schedule.penalty();
        
        println!("  Initial penalty: {:.2}", initial_penalty);
        println!("  Final penalty:   {:.2}", final_penalty);
        println!("  Improvement:     {:.2}", initial_penalty - final_penalty);
        println!();
    }
    
    Ok(())
}
