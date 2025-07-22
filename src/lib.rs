use ndarray::Array2;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoundsError {
    #[error("Place index {0} is out of bounds")]
    Place(usize),
    #[error("Time index {0} is out of bounds")]
    Time(usize),
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
    pub fn new<I>(nplaces: usize, ntimes: usize, activities: I) -> Self
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

    // Provide access to internal structure for penalty calculation
    pub fn slots(&self) -> &Array2<Option<A>> {
        &self.slots
    }

    fn reshuffle(&mut self) {
        use fastrand::usize as random_usize;
        
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
        
        // Redistribute activities: fill slots first, then unscheduled
        let mut activity_iter = all_activities.into_iter();
        
        // Fill slots
        for slot in self.slots.iter_mut() {
            if let Some(activity) = activity_iter.next() {
                *slot = Some(activity);
            }
        }
        
        // Fill unscheduled
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
        use fastrand::usize as random_usize;
        use Loc::*;
        
        let (nplaces, ntimes) = self.slots.dim();
        let nunscheduled = self.unscheduled.len();
        let ntotal = nplaces * ntimes + nunscheduled;
        let nswaps = nswaps.unwrap_or(2 * usize::pow(ntotal, 3));
        
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

            let mut cur_best = (0, 1);
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

impl<A: Clone> Schedule<A> {
    pub fn improve_with_penalty<F>(&mut self, penalty_fn: F, nswaps: Option<usize>, noise: bool, restarts: Option<usize>)
    where 
        F: Fn(&Schedule<A>) -> f32,
    {
        let num_restarts = restarts.unwrap_or(0);
        
        if num_restarts == 0 {
            self.improve_single_with_penalty(&penalty_fn, nswaps, noise);
            return;
        }
        
        let mut best_penalty = penalty_fn(self);
        let mut best_schedule = self.clone();
        
        for restart_num in 0..=num_restarts {
            if restart_num > 0 {
                self.reshuffle();
            }
            
            self.improve_single_with_penalty(&penalty_fn, nswaps, noise);
            let current_penalty = penalty_fn(self);
            
            if current_penalty < best_penalty {
                best_penalty = current_penalty;
                best_schedule = self.clone();
            }
        }
        
        *self = best_schedule;
    }

    fn improve_single_with_penalty<F>(&mut self, penalty_fn: &F, nswaps: Option<usize>, noise: bool) 
    where
        F: Fn(&Schedule<A>) -> f32,
    {
        use fastrand::usize as random_usize;
        use Loc::*;
        
        let (nplaces, ntimes) = self.slots.dim();
        let nunscheduled = self.unscheduled.len();
        let ntotal = nplaces * ntimes + nunscheduled;
        let nswaps = nswaps.unwrap_or(2 * usize::pow(ntotal, 3));
        
        let all_locations: Vec<Loc> = (0..nplaces)
            .flat_map(|p| (0..ntimes).map(move |t| S(p, t)))
            .chain((0..nunscheduled).map(U))
            .collect();

        let mut penalty = penalty_fn(self);
        for _ in 0..nswaps {
            if noise && random_usize(0..2) == 0 {
                let i = random_usize(0..ntotal);
                let j = random_usize(0..ntotal);
                self.swap_locations(all_locations[i], all_locations[j]);
                let new_penalty = penalty_fn(self);
                if new_penalty < penalty {
                    penalty = new_penalty;
                } else {
                    self.swap_locations(all_locations[j], all_locations[i]);
                }
                continue;
            }

            let mut cur_best = (0, 1);
            let mut cur_penalty = penalty;
            for i in 0..ntotal {
                for j in i + 1..ntotal {
                    self.swap_locations(all_locations[i], all_locations[j]);
                    let new_penalty = penalty_fn(self);
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
}