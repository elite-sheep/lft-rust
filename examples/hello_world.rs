// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

extern crate lft_rust;
use lft_rust::{ 
    single_queue_threadpool_builder, 
    lft_builder 
};

use crossbeam_channel::unbounded;

fn single_queue_threadpool_hello_world() {
    trace!("Hello world: Single queue threadpool.");
    let n_workers = 4;
    let n_tasks = 16;
    let thread_pool = single_queue_threadpool_builder()
        .num_workers(n_workers)
        .thread_stack_size(8 * 1024 * 1024)
        .build();

    let (tx, rx) = unbounded();
    for _ in 0..n_tasks {
        let tx = tx.clone();
        thread_pool.execute(move || {
            tx.send(1).expect("channel will be there waiting for the pool");
        });
    }
    thread_pool.join();

    rx.iter().take(n_tasks).fold(0, |i, j| {
        trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
}

fn lft_hello_world() {
    trace!("Hello world: Lock free threadpool.");
    let n_workers = 4;
    let n_tasks = 16;
    let max_threads = 4;
    let thread_pool = lft_builder()
        .num_workers(n_workers)
        .max_thread_count(max_threads)
        .thread_stack_size(8 * 1024 * 1024)
        .build();

    let (tx, rx) = unbounded();
    for _ in 0..n_tasks {
        let tx = tx.clone();
        thread_pool.execute(move || {
            tx.send(1).expect("channel will be there waiting for the pool");
        });
    }
    thread_pool.join();

    rx.iter().take(n_tasks).fold(0, |i, j| {
        trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
}

fn main() {
    pretty_env_logger::init();
    single_queue_threadpool_hello_world();
    lft_hello_world();
}
