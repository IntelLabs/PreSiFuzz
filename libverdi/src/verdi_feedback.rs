// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::str;
use core::{
    fmt::Debug,
};
use std::process::Command as pcmd;
use serde::{Deserialize, Serialize};
use libafl::{
    bolts::tuples::Named,
    corpus::Testcase,
    events::EventFirer,
    executors::ExitKind,
    inputs::{Input, UsesInput},
    observers::{ObserversTuple},
    state::HasClientPerfMonitor,
    Error,
    feedbacks::Feedback
};
use libafl::monitors::UserStats;
use libafl::events::{Event};
use crate::verdi_observer::VerdiMapObserver as VerdiObserver;
extern crate fs_extra;
use fs_extra::dir::copy;
use std::fs;
use std::process::Stdio;

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerdiFeedback {
    history: Vec<u32>,
    name: String,
    id: u32,
    outdir: String,
    score : f32
}

impl<S> Feedback<S> for VerdiFeedback
where
    S: UsesInput + HasClientPerfMonitor,
{

    #[allow(clippy::wrong_self_convention)]
    fn is_interesting<EM, OT>(
        &mut self,
        state: &mut S,
        manager: &mut EM,
        input: &S::Input,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>
    {
        let observer = observers.match_name::<VerdiObserver>(self.name()).unwrap();

        let capacity = observer.cnt() as usize;
        let mut interesting : bool = false;

        let o_map = observer.map();

        for (i, item) in o_map.iter().enumerate().take(capacity) {
            if self.history[i] < *item {
                interesting = true; 
                break;
            }
        }

        if self.score < observer.score() {
            self.score = observer.score();

            // Save scrore into state
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: format!("coverage"),
                    value: UserStats::Float(self.score as f64),
                    phantom: Default::default(),
                },
            )?;

        }

        if interesting {

            // self.history = observer.map().clone().to_vec();
//
            // let args = "-dir ./Coverage.vdb -full64 -format text -metric tgl -report ./urg_report";
            // let mut command = pcmd::new("urg");
            // let command = command.args(args.split(' '))
                // .stdin(Stdio::piped())
                // .stdout(Stdio::piped())
                // .stderr(Stdio::piped());
//
            // let child = command.spawn().expect("failed to start process");
//
            // let output = command.output().expect("failed to start process");
            // println!("status: {}", String::from_utf8_lossy(&output.stdout));
            // println!("status: {}", String::from_utf8_lossy(&output.stderr));
//
            // let file = File::open("./urg_report/tests.txt").unwrap();
//
            // let reader = std::io::BufReader::new(file);
//
            // let lines: Vec<String> = reader
                // .lines()
                // .map(|line| line.expect("Something went wrong while parsing urg report"))
                // .collect();
//
            // let lines = lines[4].split_whitespace();
//
            // let str_items: Vec<&str> = lines
            // .map(|s| s)
            // .collect();
//
            // println!("Coverage: {} \n", str_items[0]);
//
            // let score = str_items[0].parse::<f64>().unwrap();
//
            // manager.fire(
                // state,
                // Event::UpdateUserStats {
                    // name: format!("coverage"),
                    // value: UserStats::Float(score),
                    // phantom: Default::default(),
                // },
            // )?;

            self.id += 1;
        }
        Ok(interesting)
    }

    #[inline]
    fn append_metadata(
        &mut self,
        _state: &mut S,
        _testcase: &mut Testcase<S::Input>,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Discard the stored metadata in case that the testcase is not added to the corpus
    #[inline]
    fn discard_metadata(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for VerdiFeedback {
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl VerdiFeedback {
    /// Creates a new [`VerdiFeedback`], deciding if the given [`VerdiObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(name: &'static str, capacity: usize, outdir: &String) -> Self {
        let mut map = Vec::<u32>::with_capacity(capacity);
        for _i in 0..capacity {
            map.push(0);
        }
        Self {
            name: name.to_string(),
            history: map,
            id: 0,
            outdir: outdir.to_string(),
            score: 0.0
        }
    }
}

