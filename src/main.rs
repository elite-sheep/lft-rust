// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use std::sync::mpsc::channel;

mod naive_threadpool;
use naive_threadpool::{ NaiveThreadPool };

#[macro_use] extern crate queues;

fn main() {
    pretty_env_logger::init();

    let n_workers = 4;
    let n_jobs = 8;
    let thread_pool = NaiveThreadPool::new(n_workers);
    thread_pool.start();

    let (tx, rx) = channel();
    for i in 0..n_jobs+10 {
        trace!("hello, world: {} times.", i);
        let tx = tx.clone();
        thread_pool.execute(move || {
            trace!("hello, world");
            tx.send(1).expect("channel will be there waiting for the pool");
        });
    }

    rx.iter().take(n_jobs).fold(0, |i, j| {
        trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
    
}
