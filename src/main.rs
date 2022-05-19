// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use std::{sync::mpsc::channel, vec};

mod naive_threadpool;
use naive_threadpool::{ NaiveThreadPool };

#[macro_use] extern crate queues;


extern crate npy;

use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use npy::NpyData;
use ndarray::{arr1, Array, Array1};
use ndarray_rand::RandomExt;
use ndarray_rand::rand_distr::Uniform;

use std::{thread, time};
use rand::Rng;

// #[derive(Debug)]
// struct Array {
//     a: f64,
//     b: f64,
// }

fn main() {
    pretty_env_logger::init();
    let now = SystemTime::now();

    let n_workers = 4;
    let n_jobs = n_workers;
    let thread_pool = naive_threadpool::builder()
        .num_workers(n_workers)
        .thread_stack_size(8 * 1024 * 1024)
        .build();
    // thread_pool.start();

    let (tx, rx) = channel();
    for i in 0..n_jobs {
        // trace!("hello, world: {} times.", i);
        let tx = tx.clone();
        thread_pool.execute(move || {
            // trace!("hello, world");
            // multiply_random_matrix();
            multiply_matrices();
            // let time_to_sleep = time::Duration::from_millis(1000);
            // thread::sleep(time_to_sleep);
            tx.send(1).expect("channel will be there waiting for the pool");
        });
    }
    thread_pool.join();

    rx.iter().take(n_jobs).fold(0, |i, j| {
        // trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
    
    let since_the_epoch = now.elapsed();
    println!("Time duration: {:?}ms", since_the_epoch.unwrap().as_millis());
    // generate_random_matrices(3, 4);
}


fn multiply_random_matrix() {
    let mut buf1 = vec![];
    let mut buf2=vec![];
    std::fs::File::open("examples/test_1.npy").unwrap()
        .read_to_end(&mut buf1).unwrap();
        std::fs::File::open("examples/test_2.npy").unwrap()
        .read_to_end(&mut buf2).unwrap();

    let data1: Vec<f64> = NpyData::from_bytes(&buf1).unwrap().to_vec();
    let data2: Vec<f64> = NpyData::from_bytes(&buf2).unwrap().to_vec();

    //transpose data2
    let data1_len = data1.len();
    let mut output_array = vec![0.0; data1_len];
    let new_vector: Array1<_> = 1.0 * arr1(&output_array);
    transpose::transpose(&data2, &mut output_array, 1, data1_len);

    let a = Array::from(data1);
    let b = Array::from(output_array);

    let multiplied_matrix = a.dot(&new_vector);
}


fn multiply_matrices() {
    let matrix_size = rand::thread_rng().gen_range(1800..2200);

    //generate random matrices of size 1000*1000
    let a = Array::random((matrix_size, matrix_size), Uniform::new(0.0_f32, 10.0_f32));
    let b = Array::random((matrix_size, matrix_size), Uniform::new(0.0_f32, 10.0_f32));

    //transpose data2
    // let data1_len = data1.len();
    // let mut output_array = vec![0.0; data1_len];
    // let new_vector: Array1<_> = 1.0 * arr1(&output_array);
    // transpose::transpose(&data2, &mut output_array, 1, data1_len);

    // let a = Array::from(data1);
    // let b = Array::from(output_array);

    let multiplied_matrix = a.dot(&b);
}