// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use crossbeam_channel::unbounded;

mod naive_threadpool;
use naive_threadpool::{ NaiveThreadPool };

use std::{thread, time};

mod threadpool;
use threadpool::{ ThreadPool };

fn main() {
    pretty_env_logger::init();

    let n_workers = 4;
    let n_jobs = 8;
    let thread_pool = threadpool::builder()
        .num_workers(1)
        .max_thread_count(8)
        .thread_stack_size(8 * 1024 * 1024)
        .build();
    // thread_pool.start();

    let (tx, rx) = unbounded();
    for i in 0..n_jobs+8 {
        trace!("hello, world: {} times.", i);
        let tx = tx.clone();
        thread_pool.execute(move || {
            trace!("hello, world: {}.", i);
            let ten_millis = time::Duration::from_millis(128);
            let now = time::Instant::now();

            thread::sleep(ten_millis);
            tx.send(1).expect("channel will be there waiting for the pool");
        });
        
        if i < 4 {
            thread_pool.spawn_extra_one_worker();
        }

        if i == 1 || i == 2 || i == 5 || i == 6 {
            thread_pool.shutdown_one_worker();
        }
    }
    thread_pool.join();

    rx.iter().take(n_jobs+8).fold(0, |i, j| {
        trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
    
}
