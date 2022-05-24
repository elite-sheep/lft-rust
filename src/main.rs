// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use crossbeam_channel::unbounded;

mod naive_threadpool;
use naive_threadpool::{ NaiveThreadPool };

use std::{thread, time};

mod threadpool;
use threadpool::{ ThreadPool };


extern crate npy;

use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use npy::NpyData;
use ndarray::{arr1, Array, Array1};
use ndarray_rand::RandomExt;
use ndarray_rand::rand_distr::Uniform;

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

// #[derive(Debug)]
// struct Array {
//     a: f64,
//     b: f64,
// }

fn main() {
    pretty_env_logger::init();
    let now = SystemTime::now();

    let n_workers = 8;
    let n_tasks = 128;
    let thread_pool = threadpool::builder()
        .num_workers(n_workers)
        .max_thread_count(16)
        .thread_stack_size(8 * 1024 * 1024)
        .build();
    // thread_pool.start();

    let (tx, rx) = unbounded();
    for i in 0..n_tasks {
        // trace!("hello, world: {} times.", i);
        let tx = tx.clone();
        thread_pool.execute(move || {
            // trace!("hello, world: {}.", i);
            // let ten_millis = time::Duration::from_millis(128);
            // let now = time::Instant::now();

            // thread::sleep(ten_millis);

            // trace!("hello, world");
            // multiply_random_matrix();
            multiply_matrices();
            // let time_to_sleep = time::Duration::from_millis(1000);
            // thread::sleep(time_to_sleep);
            tx.send(1).expect("channel will be there waiting for the pool");
        });
        
        // if i < 3 {
        //     thread_pool.spawn_extra_one_worker();
        // }
    }
    thread_pool.join();

    rx.iter().take(n_tasks).fold(0, |i, j| {
        // trace!("Hello, world: {}, {}.", i, j);
        i+j
    });
    
    let since_the_epoch = now.elapsed();
    trace!("Time duration: {:?}ms", since_the_epoch.unwrap().as_millis());
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
    let mut rng = StdRng::seed_from_u64(2);
    let matrix_size = rng.gen_range(1024..2048);

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