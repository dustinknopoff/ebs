use chrono::{DateTime, Utc, Days, TimeZone};
use rand::{seq::IteratorRandom, rngs::ThreadRng};
use std::{collections::HashMap, error::Error, fs::File, io::BufReader, iter::zip};

const DEFAULT_COUNT: usize = 1_000;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct Task {
    project: Option<String>,
    #[allow(unused)]
    task: Option<String>,
    #[allow(unused)]
    assignee: Option<String>,
    estimate: Option<f32>,
    actual: Option<f32>,
}

impl Task {
    pub fn has_enough_data(&self) -> bool {
        self.project.is_some() && self.estimate.is_some()
    }
}

#[derive(Debug, Default, Clone)]
pub struct EBS {
    /// Mapping of a project name to it's unique ID.
    /// Fields where this ID is not 
    projects: HashMap<String, usize>,
    /// Buffer time in a project
    buffer: Vec<f32>,
    /// List of estimates for project tasks
    todos: Vec<Vec<f32>>,
    /// Historical ratio of actual to estimate
    velocity: Vec<f32>,
    simulation_runs: Vec<Vec<f32>>,
}

impl EBS {
    pub fn new_from_file(path: String) -> Result<Self, Box<dyn Error>> {
        let mut res = Self::default();
        let file = File::open(path)?;
        let buf_reader = BufReader::new(file);
        let mut rdr = csv::Reader::from_reader(buf_reader);
        let mut estimates: Vec<f32> = Vec::new();
        let mut actuals: Vec<f32> = Vec::new();
        for result in rdr.deserialize() {
            let record: Task = result?;
            if !record.has_enough_data() {
                continue;
            }
            let id = res.projects.keys().len();
            res.projects.entry(record.project.unwrap().clone()).or_insert(id);
            match (&record.estimate, &record.actual) {
                (Some(estimate), Some(actual)) => {
                    estimates.push(*estimate);
                    actuals.push(*actual);
                    res.buffer.push(actual / estimate);
                }
                (Some(estimate), None) => {
                   if let Some(exists) = res.todos.get_mut(id) {
                    exists.push(*estimate)
                   } else {
                    res.todos.push(vec![*estimate]);
                   }
                }
                _ => {}
            }
        }
        res.velocity = zip(estimates, actuals).map(|(e, a)| e / a).collect();
        res.velocity.sort_by(|a, b| a.partial_cmp(b).unwrap());
        res.buffer.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Ok(res)
    }

    pub fn montecarlo(&mut self, count: Option<usize>, mut rng: &mut ThreadRng) -> Vec<Vec<f32>> {
        let count = count.unwrap_or(DEFAULT_COUNT);
        let step = count / 10;
        let start = step - 1;
        (0..count).for_each(|_| {
            self.projects.iter().fold(0.0, |acc, (_, id)| {
                let task_estimates = self.todos[*id].clone();
                let t = task_estimates.iter().fold(0.0, |acc, t| {
                    acc + t / self.velocity.iter().choose(&mut rng).unwrap()
                });
                let time_remaining = acc + t * self.buffer.iter().choose(&mut rng).unwrap();
                if let Some(exists) = self.simulation_runs.get_mut(*id) {
                    exists.push(time_remaining);
                } else {
                    self.simulation_runs.push(vec![time_remaining]);
                }
                time_remaining
            });
        });
        self.simulation_runs.iter_mut().map(|times| {
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            times.iter().skip(start).step_by(step).copied().collect()
        }).collect()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut rng = rand::thread_rng();
    let args = std::env::args();
    if let Some(tasks) = args.into_iter().nth(1) {
        let mut ebs = EBS::new_from_file(tasks)?;
        let _f = ebs.montecarlo(None, &mut rng);
        let results: Vec<DateTime<Utc>> = _f.iter().map(|timeline| {
            let mut total = 0.0;
            let mut day = Utc.with_ymd_and_hms(2015, 9, 4, 0, 0, 0).unwrap();
            timeline.iter().for_each(|hours| {
                while *hours > total {
                    let num_days_to_add = {
                        let intermediate = (hours / 8.0).floor();
                        if intermediate >= 1.0 {
                            intermediate as u64
                        } else {
                            1.0 as u64
                        }
                    };
                    day = day.checked_add_days(Days::new(num_days_to_add)).unwrap();
                    total += hours
                }
            });
            dbg!(total);
            day
        }).collect();
        dbg!(results);
        Ok(())
    } else {
        Ok(())
    }
}
