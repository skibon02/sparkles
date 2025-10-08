//! Cross-thread ranges example
//! 
//! Demonstrates range events that start in one thread and end in another thread.
//! This example shows:
//! - Main thread starting work items
//! - Worker threads completing the work items  
//! - Cross-thread ranges spanning from main to workers
//!
//! 1. Run `cargo run --example cross-thread-ranges --release`
//! 2. Parse result file: `sparkles-parse-and-save trace.sparkles`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file
//! 4. Look for ranges labeled with `[worker_X] work_item_N` to see cross-thread spans

#[path = "../examples_common.rs"]
pub mod common;

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use clap::Parser;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles::{instant_event, range_event_end, range_event_start};
use crate::common::Args;

struct WorkItem {
    id: u32,
    range_guard: sparkles::RangeStartGuard,
}

fn main() {
    SimpleLogger::default().with_level(LevelFilter::Info).init().unwrap();

    let args = Args::parse();
    info!("Running with args: {args:?}");
    let cfg = args.sparkles_cfg();
    let finalize_guard = sparkles::init(cfg);

    sparkles::wait_client_connected();
    
    println!("Starting cross-thread ranges example...");
    
    let main_range = range_event_start!("main_thread_orchestration");
    
    // Create channels for work distribution
    let (work_sender, work_receiver) = mpsc::channel::<WorkItem>();
    let work_receiver = Arc::new(std::sync::Mutex::new(work_receiver));
    
    // Spawn multiple worker threads
    let mut worker_handles = Vec::new();
    
    for worker_id in 0..3 {
        let work_receiver = Arc::clone(&work_receiver);
        let handle = thread::Builder::new()
            .name(format!("worker_{}", worker_id))
            .spawn(move || {
                let worker_range = range_event_start!("worker_thread_lifetime");
                
                loop {
                    // Try to get work from the queue
                    let work_item = {
                        let receiver = work_receiver.lock().unwrap();
                        receiver.try_recv()
                    };
                    
                    match work_item {
                        Ok(item) => {
                            instant_event!("received_work");
                            
                            // Simulate processing the work item
                            let processing_time = Duration::from_millis(10 + (item.id % 50) as u64);
                            thread::sleep(processing_time);
                            
                            instant_event!("work_completed");
                            
                            // End the cross-thread range that was started in main
                            range_event_end!(item.range_guard, "work_finished");
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            // No work available, sleep briefly
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            // Main thread finished sending work
                            instant_event!("worker_shutting_down");
                            break;
                        }
                    }
                }
                
                range_event_end!(worker_range, "worker_finished");
            })
            .unwrap();
        
        worker_handles.push(handle);
    }
    
    // Main thread creates work items and distributes them
    for work_id in 0..20 {
        instant_event!("creating_work_item");
        
        // Start a range event in the main thread that will be ended in a worker thread
        let range_guard = range_event_start!("cross_thread_work_item");
        
        let work_item = WorkItem {
            id: work_id,
            range_guard,
        };
        
        instant_event!("dispatching_work");
        work_sender.send(work_item).unwrap();
        
        // Brief pause between work creation
        thread::sleep(Duration::from_millis(5));
    }
    
    instant_event!("all_work_dispatched");
    
    // Close the work channel
    drop(work_sender);
    
    // Wait for all workers to complete
    for (_i, handle) in worker_handles.into_iter().enumerate() {
        instant_event!("waiting_for_worker");
        handle.join().unwrap();
        instant_event!("worker_finished");
    }
    
    instant_event!("all_workers_completed");
    
    range_event_end!(main_range, "orchestration_complete");
    
    println!("Cross-thread ranges example completed!");
    println!("Parse the trace with: sparkles-parse-and-save trace.sparkles");
    println!("Then view in Perfetto at: https://ui.perfetto.dev/");
    
    // The finalize_guard will automatically flush all events when dropped
}