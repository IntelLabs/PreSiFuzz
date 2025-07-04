// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::str;
use core::{
    fmt::Debug,
    time::Duration,
};
use serde::{Deserialize, Serialize};
use libafl::{
    corpus::Testcase,
    events::EventFirer,
    executors::ExitKind,
    inputs::{UsesInput},
    observers::{ObserversTuple},
    Error,
    feedbacks::Feedback,
    state::{State},
};

use libafl_bolts::current_time;
use libafl_bolts::Named;
use libafl_bolts::AsSlice;
use libafl::monitors::{UserStats, UserStatsValue, AggregatorOps};
use libafl::events::{Event};
use std::process::Command;
use std::path::Path;
use libafl::inputs::Input;

use libafl::prelude::MapFeedbackMetadata;
use libafl::state::HasMetadata;

use libpresifuzz_observers::verdi_xml_observer::VerdiXMLMapObserver as VerdiObserver;

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerdiFeedback 
{
    history: Vec<u32>,
    name: String,
    id: u32,
    workdir: String,
    score: f32,
    save_on_new_coverage: bool,
    start_time: Duration,
    minimized: bool,
}

impl VerdiFeedback
{
    fn compare_coverage_map(&mut self, o_map: &[u32], capacity: usize) -> bool {
            
        if self.history.capacity() < capacity {
            self.history.resize(capacity, 0);
        }

        // count the bits that are set in the observer map but not in the history map
        return self.history.iter()
            .zip(o_map.iter())
            .map(|(&x, &y)| ((!x) & y).count_ones())
            .sum::<u32>() != 0;
    }

    fn update_history_map(&mut self, o_map: &[u32], capacity: usize) -> (u32, u32) {

        // update history so that it includes all bits that are set in o_map but not currently set in history
        assert_eq!(self.history.len(), o_map.len());
        for (x, &y) in self.history.iter_mut().zip(o_map.iter()) {
            *x |= y;
        }

        let coverable = (self.history.len() * 32) as u32;
        let covered = self.history.iter().map(|&x| x.count_ones()).sum();

        return (covered, coverable);
    }
}

impl<S> Feedback<S> for VerdiFeedback
where
    S: UsesInput + State,
{

    #[allow(clippy::wrong_self_convention)]
    fn is_interesting<EM, OT>(
        &mut self,
        state: &mut S,
        manager: &mut EM,
        _input: &S::Input,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>
    {
        if cfg!(feature = "const_true") {
            return Ok(true);
        } else if cfg!(feature = "const_false") {
            return Ok(false);
        }

        let observer = observers.match_name::<VerdiObserver>(self.name()).unwrap();
        let capacity = observer.cnt();

        let o_map = observer.my_map().as_slice();

        let mut interesting : bool = self.compare_coverage_map(o_map, capacity);

        self.score = (o_map[0] as f32 / o_map[1] as f32) * 100.0;
        println!("Analyzing xml from vdb with coverage {} score at {}% for {}/{}", self.name, self.score, o_map[0], o_map[1]);

        if interesting {
        
            let (covered, coverable) = self.update_history_map(o_map, capacity);

            self.score = (covered as f32 / coverable as f32) * 100.0;

            println!("Merge coverage {} score is {}% {}/{}", self.name, self.score, covered, coverable);

            // Save scrore into state
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: format!("coverage_{}", self.name()).to_string(),
                    value: UserStats::new( UserStatsValue::Ratio(covered as u64, coverable as u64), AggregatorOps::None),
                    phantom: Default::default(),
                },
            )?;

            // Save scrore into state
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: format!("time_{}", self.name()).to_string(),
                    value: UserStats::new( UserStatsValue::Number((current_time() - self.start_time).as_secs()), AggregatorOps::None),
                    phantom: Default::default(),
                },
            )?;

            if self.save_on_new_coverage == true {
                _input.to_file(Path::new(&format!("{}.seed",self.id)))?;         
            }

            self.id += 1;
        }
        
        return Ok(interesting);
    }

    #[inline]
    fn append_metadata<OT>(
        &mut self,
        _state: &mut S,
        _observers: &OT,
        testcase: &mut Testcase<S::Input>,
    ) -> Result<(), Error> 
    where
        OT: ObserversTuple<S>,
    {
        if self.minimized {
            let metadata = MapFeedbackMetadata::<u32>::with_history_map(self.history.clone());
            testcase.metadata_map_mut().insert(metadata);
        }

        Ok(())
    }

    /// Discard the stored metadata in case that the testcase is not added to the corpus
    #[inline]
    fn discard_metadata(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for VerdiFeedback
{
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl VerdiFeedback
{
    /// Creates a new [`VerdiFeedback`], deciding if the given [`VerdiObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(name: &'static str, workdir: &str, minimized: bool) -> Self {
        Self {
            name: name.to_string(),
            history: Vec::<u32>::new(),
            id: 0,
            workdir: workdir.to_string(),
            score: 0.0,
            save_on_new_coverage: false,
            start_time: current_time(),
            minimized: minimized,
        }
    }
}

// TODO: Re-enable this test using vdb from open source design
#[cfg(feature = "std")]
#[cfg(test)]
mod tests {

    extern crate fs_extra;
    use libc::{c_uint, c_char, c_void};
    use std::process;
    use libafl_bolts::prelude::StdRand;
    use libafl::prelude::BytesInput;
    use libafl::executors::{ExitKind};
    use libafl_bolts::current_time;
    use libafl::prelude::InMemoryCorpus;
    use libafl::prelude::Testcase;
    use libafl::prelude::ConstFeedback;
    use libpresifuzz_observers::verdi_xml_observer::VerdiXMLMapObserver;
    use libpresifuzz_observers::verdi_xml_observer::VerdiCoverageMetric;
    use crate::verdi_xml_feedback::VerdiFeedback;

    use libafl::prelude::StdState;
    use libafl::state::HasMaxSize;
    use libafl::observers::Observer;
    use libafl::feedbacks::Feedback;

    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use rand::{Rng, seq::SliceRandom, thread_rng};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::fs;
    use std::path::Path;
    use rand::distributions::Uniform;
    use libafl_bolts::prelude::tuple_list;
    use libafl_bolts::AsSlice;


    use quick_xml::events::Event::{Start, Empty};
    use quick_xml::events::{BytesStart, Event};
    use std::io::{BufReader, BufWriter};
    use quick_xml::Reader;
    use quick_xml::Writer;

    pub fn flip_bits_in_coverage_file(file_path: &str, num_flips: usize) -> bool {
        // Open compressed input file
        let file = File::open(file_path).unwrap_or_else(|e| panic!("Failed to open file: {e}"));
        let decoder = GzDecoder::new(BufReader::new(file));
        let mut reader = Reader::from_reader(BufReader::new(decoder));

        // Prepare compressed output file
        let out_file = File::create(file_path).unwrap_or_else(|e| panic!("Failed to create file: {e}"));
        let encoder = GzEncoder::new(BufWriter::new(out_file), Compression::default());
        let mut writer = Writer::new_with_indent(encoder, b' ', 0);

        let mut buf = Vec::new();
        let mut rng = rand::thread_rng();

        let mut flips_left = num_flips;
        
        // let mut rng = rand::thread_rng();
                                    
        let mut is_interesting_oracle = false;

        loop {
            match reader.read_event_into(&mut buf) {

                Ok(Event::Empty(mut e)) if e.name().as_ref() == b"instance_data" => {
                    // flip value and write back
                    let mut new_elem = BytesStart::new("instance_data");
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"value" {
                            let mut value = String::from_utf8_lossy(&attr.value).into_owned();
           
                            // let do_bit_flip = rng.gen_bool(0.5);
           
                            if !value.is_empty() && flips_left != 0 {
                                let size = value.len();

                                let idx = rng.gen_range(0..value.len());
                                let ch = value.as_bytes()[idx] as char;
                                let flipped = if ch == '0' { '1' } else { '0' };
                                
                                if flipped == '1' && ch == '0' {
                                    is_interesting_oracle = true;
                                }

                                value.replace_range(idx..=idx, &flipped.to_string());
                                
                                flips_left -= 1;
                            }

                            new_elem.push_attribute(("value", value.as_str()));
                        } else {
                            new_elem.push_attribute((attr.key.as_ref(), attr.value.as_ref()));
                        }
                    }
                    writer.write_event(Event::Empty(new_elem))
                        .unwrap_or_else(|e| panic!("Failed to write flipped instance_data"));
                }

                Ok(Event::Eof) => break,

                Ok(other) => {
                    // Pass through everything else unchanged
                    writer.write_event(other)
                        .unwrap_or_else(|e| panic!("Failed to write XML event: {e}"));
                }

                Err(e) => panic!("Error while reading XML: {e}"),
            }

            buf.clear();
        }

        writer.into_inner()
            .finish()
            .unwrap_or_else(|e| panic!("Failed to finish writing compressed file: {e}"));
        
        return is_interesting_oracle;
    }

    fn create_initial_xml_files() {
        
        // Configuration:
        let mut rng = thread_rng();
        let num_instances = rng.gen_range(10..=20); // random number of <instance_data> blocks

        let mut xml_data = String::new();

        // XML header & root
        xml_data.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml_data.push_str("\n<coverage>\n");

        for i in 0..num_instances {
            // Give each instance a unique name
            let name = format!("inst_{}", i);

            // Generate a random bitstring length, e.g., 32–256 bits
            let bit_len = rng.gen_range(32..=64);

            // Random bits as '0' or '1'
            let bit_dist = Uniform::new_inclusive(0, 1);
            let value: String = (0..bit_len)
                .map(|_| {
                    if rng.sample(bit_dist) == 0 { '0' } else { '1' }
                })
                .collect();

            // Write <instance_data>
            xml_data.push_str(&format!(
                r#"  <instance_data name="{}" value="{}"/>"#,
                name, value
            ));
            xml_data.push('\n');
        }

        // Close root
        xml_data.push_str("</coverage>\n");

        // Write compressed XML to tgl.verilog.data.xml.gz
        let full_path = Path::new("./")
            .join("test.vdb")
            .join("snps")
            .join("coverage")
            .join("db")
            .join("testdata")
            .join("test")
            .join("tgl.verilog.data.xml");

        let file = File::create(full_path);
        if !file.is_ok() {
            panic!("Unable to create overwrite randonly generated test xml file");
        }
        let file = file.unwrap();

        let buf_writer = BufWriter::new(file);
        let mut encoder = GzEncoder::new(buf_writer, Compression::default());
        if !encoder.write_all(xml_data.as_bytes()).is_ok() {
            panic!("Faile to write to randomly generated test xml file!");;
        }
        if !encoder.finish().is_ok() {
            panic!("Faile to write to randomly generated test xml file!");;
        }

        println!("Generated tgl.verilog.data.xml with {} instances.", num_instances);
    
    }

    /// Create `{base_dir}/snps/coverage/db/testdata/test/`
    fn create_coverage_dirs(base_dir: &str) -> std::io::Result<()> {
        let full_path = Path::new(base_dir)
            .join("test.vdb")
            .join("snps")
            .join("coverage")
            .join("db")
            .join("testdata")
            .join("test");

        fs::create_dir_all(&full_path)?;

        println!("Created directories: {}", full_path.display());
        Ok(())
    }

    #[test]
    fn test_verdi_xml_observer() {

        let file_path = "./test.vdb/snps/coverage/db/testdata/test/tgl.verilog.data.xml";
        
        for i in 0..1000 {

            println!("########### Test nb {i} ############");

            let mut rng = rand::thread_rng();

            // create the coverage dir hierarchy to mimic vcs built Coverage.vdb
            create_coverage_dirs("./");

            // fill the directory with some randomly generated xml files
            create_initial_xml_files();

            // initial dummy input 
            let input = BytesInput::new(vec![1, 2, 3, 4]);

            let rand = StdRand::with_seed(current_time().as_nanos() as u64);
            let corpus = InMemoryCorpus::<BytesInput>::new();

            // let mut feedback = VerdiXMLFeedback();
            let mut feedback = VerdiFeedback::new_with_observer(
                concat!("verdi_tgl_observer"),
                "./",
                true
            );

            // let mut feedback = ConstFeedback::new(true);
            let mut objective = ConstFeedback::new(false);

            let mut verdi_observer = VerdiXMLMapObserver::new(
                    "verdi_map",
                    &String::from("test.vdb"),
                    ".",
                    VerdiCoverageMetric::Toggle,
                    &"".to_string()
            );

            let mut state = StdState::new(
                rand,
                corpus,
                InMemoryCorpus::<BytesInput>::new(),
                &mut feedback,
                &mut objective,
            )
            .unwrap();
            state.set_max_size(1024);

            // run the observer on intial map
            let _ = verdi_observer.post_exec(&mut state, &input, &ExitKind::Ok);

            // run the feedback on initial map
            let capacity = verdi_observer.cnt();
            println!("Capacity is {} or {}", capacity, verdi_observer.my_map().len());

            let o_map = verdi_observer.my_map().as_slice();
            let is_interesting = feedback.compare_coverage_map(o_map, capacity);

            assert!(is_interesting == true);
                
            let (covered, coverable) = feedback.update_history_map(o_map, capacity);

            // bit flip some coverage items
            let is_interesting_oracle = flip_bits_in_coverage_file(file_path, 250);

            // run the observer on the bits flipped map
            let _ = verdi_observer.post_exec(&mut state, &input, &ExitKind::Ok);

            // run the feedback on the bits flipped map
            let capacity = verdi_observer.cnt();
            println!("Capacity is {} or {}", capacity, verdi_observer.my_map().len());
          
            let o_map = verdi_observer.my_map().as_slice();
            let is_interesting = feedback.compare_coverage_map(o_map, capacity);

            if is_interesting != is_interesting_oracle {

                println!("Test Fail @ new coverage bits!");
                for (_k, item) in o_map.iter().enumerate().filter(|&(_k,_)| _k>=2).take(capacity) {
                    println!("{:#034b}", o_map[_k]);
                }
                assert!(is_interesting == is_interesting_oracle);
            }

            let (covered, coverable) = feedback.update_history_map(o_map, capacity);

            // flip no bits
            let is_interesting_oracle = false;

            // run the observer on the bits flipped map
            let _ = verdi_observer.post_exec(&mut state, &input, &ExitKind::Ok);

            // run the feedback on the bits flipped map
            let capacity = verdi_observer.cnt();
            println!("Capacity is {} or {}", capacity, verdi_observer.my_map().len());

            let o_map = verdi_observer.my_map().as_slice();
            let is_interesting = feedback.compare_coverage_map(o_map, capacity);

            if is_interesting != is_interesting_oracle {
        
                println!("Test Fail @ no new coverage!");
                for (_k, item) in o_map.iter().enumerate().filter(|&(_k,_)| _k>=2).take(capacity) {
                    println!("{:#034b}", o_map[_k]);
                }
                assert!(is_interesting == is_interesting_oracle);
            }
            
            println!("Test Pass!\n\n");
        }
    }
}

