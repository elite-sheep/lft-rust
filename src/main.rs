// Copyright @yucwang 2022

use async_std::task;

extern crate pretty_env_logger;
#[macro_use] extern crate log;

mod sequentialtaskqueue;
use sequentialtaskqueue::{ SequentialTaskQueue };

fn main() {
    pretty_env_logger::init();

    let mut task_queue = SequentialTaskQueue::new(1024);
    let test_task = task::spawn(async {
        1 + 4
    });
    task_queue.enqueue(&test_task.task());

    trace!("Hello, World");
}
