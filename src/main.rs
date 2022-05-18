// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use crossbeam_channel::unbounded;

mod naive_threadpool;
use naive_threadpool::{ NaiveThreadPool };

use std::{thread, time};

mod threadpool;
use threadpool::{ ThreadPool };
#[macro_use] extern crate queues;


extern crate npy;

use std::io::Read;
use npy::NpyData;
use ndarray::{Array, Array1};

// #[derive(Debug)]
// struct Array {
//     a: f64,
//     b: f64,
// }

fn main() {
    pretty_env_logger::init();

    let n_workers = 4;
    let n_jobs = 8;
    let thread_pool = threadpool::builder()
        .num_workers(4)
        .max_thread_count(16)
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

            //multiply_random_matrix();

            tx.send(1).expect("channel will be there waiting for the pool");
        });
        
        // if i < 3 {
        //     thread_pool.spawn_extra_one_worker();
        // }
    }
    thread_pool.join();

    rx.iter().take(n_jobs+1).fold(0, |i, j| {
        trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
    
}


fn multiply_random_matrix() {
    let mut buf1 = vec![];
    let mut buf2=vec![];
    let data1: Vec<f64> = NpyData::from_bytes(&buf1).unwrap().to_vec();
    let data2: Vec<f64> = NpyData::from_bytes(&buf2).unwrap().to_vec();

    //transpose data2
    let data1_len = data1.len();
    let mut output_array = vec![0.0; data1_len];
    let new_vector: Array1<_> = 1 * output_array;
    transpose::transpose(&data2, &mut output_array, 1, data1_len);

    let a = Array::from(data1);
    let b = Array::from(output_array);

    let multiplied_matrix = a.dot(&new_vector);
}