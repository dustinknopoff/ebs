use rand::seq::IteratorRandom;
use std::{collections::HashMap, error::Error, fs::File, io::BufReader, iter::zip, ops::{Deref, DerefMut}};

const DEFAULT_COUNT: usize = 1_000;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct Task {
    project: Option<String>,
    task: Option<String>,
    assignee: Option<String>,
    estimate: Option<f32>,
    actual: Option<f32>,
}

impl Task {
    pub fn has_enough_data(&self) -> bool {
        self.project.is_some() && self.estimate.is_some()
    }
}

pub struct ProjectId(usize);

impl Deref for ProjectId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ProjectId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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
            // Notice that we need to provide a type hint for automatic
            // deserialization.
            let record: Task = result?;
            println!("{:?}", record);
            if !record.has_enough_data() {
                continue;
            }
            let id = dbg!(res.projects.keys().len());
            res.projects.entry(record.project.unwrap().clone()).or_insert(id);
            match (&record.estimate, &record.actual) {
                (Some(estimate), Some(actual)) => {
                    estimates.push(*estimate);
                    actuals.push(*actual);
                    res.buffer.push(actual / estimate);
                }
                (Some(estimate), None) => {
                   if let Some(exists) = res.todos.get_mut(id - 1) {
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
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut rng = rand::thread_rng();
    let args = std::env::args();
    if let Some(tasks) = args.into_iter().nth(1) {
        let mut ebs = EBS::new_from_file(tasks)?;
        // Montecarlo
        let count = DEFAULT_COUNT;
        let step = count / 10;
        let start = step - 1;
        (0..count).for_each(|_| {
            let mut time_remaining = 0.0;
            ebs.projects.iter().for_each(|(_, id)| {
                let task_estimates = ebs.todos[*id].clone();
                let t = task_estimates.iter().fold(0.0, |acc, t| {
                    acc + t / ebs.velocity.iter().choose(&mut rng).unwrap()
                });
                time_remaining += t * ebs.buffer.iter().choose(&mut rng).unwrap();
                if let Some(exists) = ebs.simulation_runs.get_mut(*id) {
                    exists.push(time_remaining);
                } else {
                    ebs.simulation_runs.push(vec![time_remaining]);
                }
            })
        });

        let _f: HashMap<String, Vec<f32>> = ebs.projects
            .iter()
            .map(|(project, id)| {
                let mut times = ebs.simulation_runs[*id].clone();
                times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                (
                    project.clone(),
                    times.iter().skip(start).step_by(step).copied().collect(),
                )
            })
            .collect();
        dbg!(_f);
        Ok(())
    } else {
        Ok(())
    }
}
