// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use std::{sync::mpsc::channel, vec};

mod naive_threadpool;
use naive_threadpool::{ NaiveThreadPool };

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
    let thread_pool = naive_threadpool::builder()
        .num_workers(8)
        .thread_stack_size(8 * 1024 * 1024)
        .build();
    // thread_pool.start();

    let (tx, rx) = channel();
    for i in 0..n_jobs+10 {
        trace!("hello, world: {} times.", i);
        let tx = tx.clone();
        thread_pool.execute(move || {
            // trace!("hello, world");
            // tx.send(1).expect("channel will be there waiting for the pool");
            multiply_random_matrix();
        });
    }
    thread_pool.join();

    rx.iter().take(n_jobs+10).fold(0, |i, j| {
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