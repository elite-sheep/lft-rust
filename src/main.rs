// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use crossbeam_channel::unbounded;

mod naive_threadpool;
use naive_threadpool::{ NaiveThreadPool };

fn main() {
    pretty_env_logger::init();

    let n_workers = 4;
    let n_jobs = 8;
    let thread_pool = naive_threadpool::builder()
        .num_workers(8)
        .thread_stack_size(8 * 1024 * 1024)
        .build();
    // thread_pool.start();

    let (tx, rx) = unbounded();
    for i in 0..n_jobs+10 {
        trace!("hello, world: {} times.", i);
        let tx = tx.clone();
        thread_pool.execute(move || {
            trace!("hello, world");
            tx.send(1).expect("channel will be there waiting for the pool");
        });
    }
    thread_pool.join();

    rx.iter().take(n_jobs+10).fold(0, |i, j| {
        trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
    
}
