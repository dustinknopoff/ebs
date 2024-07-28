use indicatif::ProgressBar;
use jiff::{civil::Weekday, ToSpan};
use rand::{rngs::ThreadRng, seq::IteratorRandom};
use std::{collections::HashMap, error::Error, fs::File, io::BufReader, iter::zip};

const DEFAULT_COUNT: usize = 1_000_000;

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
        let mut only_with_estimates_actuals: Vec<f32> = vec![];
        let mut all_actuals: Vec<f32> = vec![];
        for result in rdr.deserialize() {
            let record: Task = result?;
            if record.project.is_none() {
                continue;
            }
            // We guarantee above that the project key is defined in the row.
            // We then use it in objects in Self as an id to the index
            // (Struct of Arrays instead of Array of Structs)
            let id = {
                let id = res.projects.keys().len();
                let project = record.project.unwrap().clone();
                *res.projects.entry(project.clone()).or_insert(id)
            };
            match (&record.estimate, &record.actual) {
                (Some(estimate), Some(actual)) => {
                    estimates.push(*estimate);
                    actuals.push(*actual);
                    if let Some(exists) = all_actuals.get_mut(id) {
                        *exists += actual;
                    } else {
                        all_actuals.push(*actual);
                    }
                    if let Some(exists) = only_with_estimates_actuals.get_mut(id) {
                        *exists += actual;
                    } else {
                        only_with_estimates_actuals.push(*actual);
                    }
                }
                (Some(estimate), None) => {
                    if let Some(exists) = res.todos.get_mut(id) {
                        exists.push(*estimate)
                    } else {
                        res.todos.push(vec![*estimate]);
                    }
                }
                (None, Some(actual)) => {
                    if let Some(exists) = all_actuals.get_mut(id) {
                        *exists += actual;
                    } else {
                        all_actuals.push(*actual);
                    }
                }
                _ => {}
            }
        }
        res.buffer = res
            .projects.values().map(|id|{
                // The buffer is the sum of all actual values divided by the sum of those which
                // Also have an estimate
                 all_actuals[*id] / only_with_estimates_actuals[*id]
            })
            .collect();
        // Velocities are the estimate : actual ratio
        res.velocity = zip(estimates, actuals).map(|(e, a)| e / a).collect();
        res.velocity.sort_by(|a, b| a.partial_cmp(b).unwrap());
        res.buffer.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Ok(res)
    }

    pub fn montecarlo(&mut self, count: Option<usize>, mut rng: &mut ThreadRng) -> Vec<Vec<f32>> {
        let count = count.unwrap_or(DEFAULT_COUNT);
        let pb = ProgressBar::new((count * self.projects.len()) as u64);
        let step = count / 100;
        let start = step - 1;
        pb.tick();
        // We run {count} simulations
        (0..count).for_each(|_| {
            self.projects.iter().fold(0.0, |remaining, (_, id)| {
                // The "montecarlo" here is randomly specifying that the 
                // Task will take a previous velocity length
                let task_estimates = self.todos[*id].clone();
                let t = task_estimates.iter().fold(0.0, |estimate, t| {
                    t / self.velocity.iter().choose(&mut rng).unwrap() + estimate
                });
                // And then that we will have a random buffer left after finishing the task
                let time_remaining = t * self.buffer.iter().choose(&mut rng).unwrap() + remaining;
                if let Some(exists) = self.simulation_runs.get_mut(*id) {
                    exists.push(time_remaining);
                } else {
                    self.simulation_runs.push(vec![time_remaining]);
                }
                time_remaining
            });
            pb.inc(1);
        });
        pb.finish_and_clear();
        println!(
            "Simulations ran for {} projects in {:?}.",
            self.projects.len(),
            pb.elapsed()
        );
        // We then trim down the simulation runs to 1/10th sampling
        self.simulation_runs
            .iter_mut()
            .map(|times| {
                times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                times.iter().skip(start).step_by(step).copied().collect()
            })
            .collect()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut rng = rand::thread_rng();
    let date = jiff::Zoned::now();
    let args = std::env::args();
    if let Some(tasks) = args.into_iter().nth(1) {
        let mut ebs = EBS::new_from_file(tasks)?;
        let _f = ebs.montecarlo(None, &mut rng);
        ebs.projects.iter().for_each(|(project, id)| {
            let chance50 = (_f[*id][49] / 8.0).ceil();
            let chance95 = (_f[*id][94] / 8.0).ceil();
            println!("{project}:");
            println!("\t50% chance: {}, {} dev days", &dev_days_as_days(chance50 as usize, date.clone()), chance50);
            println!("\t95% chance: {}, {} dev days", &dev_days_as_days(chance95 as usize, date.clone()), chance95);
        })
    }
    Ok(())
}

fn dev_days_as_days(number: usize, startdate: jiff::Zoned) -> jiff::Zoned {
        (0..=number).fold(startdate, |acc, _| {
            let mut day = acc.start_of_day().unwrap();
            day = day.checked_add(1.day()).unwrap();
            match day.weekday() {
                Weekday::Monday
                | Weekday::Tuesday
                | Weekday::Wednesday
                | Weekday::Thursday
                | Weekday::Friday => {
                    day
                }
                Weekday::Saturday => {
                    day.checked_add(2.days()).unwrap()
                },
                Weekday::Sunday => {
                    day.checked_add(1.days()).unwrap()
                }
            }
        })

}