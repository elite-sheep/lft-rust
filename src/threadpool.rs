// Copyright 2022 @yucwang
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A lock-free thread pool used to execute functions in parallel.
//!
//! Spawns a specified number of worker threads and replenishes the pool if any worker threads
//! panic.
//!
//! # Examples
//!
//! ## Synchronized with a channel
//!
//! Every thread sends one message over the channel, which then is collected with the `take()`.
//!
//! ```
//! use crossbeam_channel::unbounded;
//!
//! let n_workers = 4;
//! let n_jobs = 8;
//! let pool = lft_rust::lft_builder()
//!                 .num_workers(n_workers)
//!                 .build();
//!
//! let (tx, rx) = unbounded();
//! for _ in 0..n_jobs {
//!     let tx = tx.clone();
//!     pool.execute(move|| {
//!         tx.send(1).expect("channel will be there waiting for the pool");
//!     });
//! }
//! drop(tx);
//!
//! assert_eq!(rx.iter().take(n_jobs).fold(0, |a, b| a + b), 8);
//! ```
//!
//! ## Synchronized with a barrier
//!
//! Keep in mind, if a barrier synchronizes more jobs than you have workers in the pool,
//! you will end up with a [deadlock](https://en.wikipedia.org/wiki/Deadlock)
//! at the barrier which is [not considered unsafe](
//! https://doc.rust-lang.org/reference/behavior-not-considered-unsafe.html).
//!
//! ```
//! use lft_rust::{ ThreadPool };
//! use std::sync::{Arc, Barrier};
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! // create at least as many workers as jobs or you will deadlock yourself
//! let n_workers = 42;
//! let n_jobs = 23;
//! let pool = lft_rust::lft_builder()
//!                 .num_workers(n_workers)
//!                 .build();
//! let an_atomic = Arc::new(AtomicUsize::new(0));
//!
//! assert!(n_jobs <= n_workers, "too many jobs, will deadlock");
//!
//! // create a barrier that waits for all jobs plus the starter thread
//! let barrier = Arc::new(Barrier::new(n_jobs + 1));
//! for _ in 0..n_jobs {
//!     let barrier = barrier.clone();
//!     let an_atomic = an_atomic.clone();
//!
//!     pool.execute(move|| {
//!         // do the heavy work
//!         an_atomic.fetch_add(1, Ordering::Relaxed);
//!
//!         // then wait for the other threads
//!         barrier.wait();
//!     });
//! }
//!
//! // wait for the threads to finish the work
//! barrier.wait();
//! assert_eq!(an_atomic.load(Ordering::SeqCst), n_jobs);
//! ```

use num_cpus;

use crossbeam_channel::{ unbounded, Receiver, Sender };
use log::{ trace, warn };

use std::fmt;
use std::sync::atomic::{ AtomicI8, AtomicUsize, Ordering };
use std::sync::{ Arc, Condvar, Mutex };
use std::{ thread, time };

#[cfg(test)]
mod test;

/// Creates a new thread pool with the same number of workers as CPUs are detected.
///
/// # Examples
///
/// Create a new thread pool capable of executing at least one jobs concurrently:
///
/// ```
/// let pool = threadpool::auto_config();
/// ```
pub fn lft_auto_config() -> ThreadPool {
    lft_builder().build()
}

/// Initiate a new [`Builder`].
///
/// [`Builder`]: struct.Builder.html
///
/// # Examples
///
/// ```
/// let builder = lft_rust::lft_builder();
/// ```
pub const fn lft_builder() -> Builder {
    Builder {
        num_workers: None,
        max_thread_count: None,
        worker_name: None,
        thread_stack_size: None,
    }
}

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Thunk<'a> = Box<dyn FnBox + Send + 'a>;

struct Sentinel<'a> {
    shared_data: &'a Arc<ThreadPoolSharedData>,
    receiver: &'a Arc<Receiver<Thunk<'static>>>,
    num_jobs: &'a Arc<AtomicUsize>,
    thread_closing: &'a Arc<AtomicI8>,
    active: bool,
}

impl<'a> Sentinel<'a> {
    fn new(shared_data: &'a Arc<ThreadPoolSharedData>, 
           receiver: &'a Arc<Receiver<Thunk<'static>>>,
           num_jobs: &'a Arc<AtomicUsize>,
           thread_closing: &'a Arc<AtomicI8>) -> Sentinel<'a> {
        Sentinel {
            shared_data: shared_data,
            receiver: receiver,
            num_jobs: num_jobs,
            thread_closing: thread_closing,
            active: true,
        }
    }

    /// Cancel and destroy this sentinel.
    fn cancel(mut self) {
        self.active = false;
    }
}

impl<'a> Drop for Sentinel<'a> {
    fn drop(&mut self) {
        if self.active {
            self.shared_data.active_count.fetch_sub(1, Ordering::SeqCst);
            self.thread_closing.store(3, Ordering::SeqCst);
            if thread::panicking() {
                self.shared_data.panic_count.fetch_add(1, Ordering::SeqCst);
            }
            if self.num_jobs.load(Ordering::Acquire) == 0 {
                self.shared_data.no_work_notify_all();
            }

            self.thread_closing.store(1, Ordering::SeqCst);
            spawn_in_pool(self.shared_data.clone(), 
                          self.receiver.clone(), 
                          self.num_jobs.clone(), 
                          self.thread_closing.clone())
        }
    }
}

/// [`ThreadPool`] factory, which can be used in order to configure the properties of the
/// [`ThreadPool`].
///
/// The three configuration options available:
///
/// * `num_workers`: the number of worker threads that will be spawned upon building.
/// * `max_thread_count`: maximum number of threads that will be alive at any given moment by the built
///   [`ThreadPool`]
/// * `worker_name`: thread name for each of the threads spawned by the built [`ThreadPool`]
/// * `thread_stack_size`: stack size (in bytes) for each of the threads spawned by the built
///   [`ThreadPool`]
///
/// [`ThreadPool`]: struct.ThreadPool.html
///
/// # Examples
///
/// Build a [`ThreadPool`] that uses a maximum of eight threads simultaneously and each thread has
/// a 8 MB stack size:
///
/// ```
/// let pool = lft_rust::lft_builder()
///     .num_workers(8)
///     .thread_stack_size(8 * 1024 * 1024)
///     .build();
/// ```
#[derive(Clone, Default)]
pub struct Builder {
    num_workers: Option<usize>,
    max_thread_count: Option<usize>,
    worker_name: Option<String>,
    thread_stack_size: Option<usize>,
}

impl Builder {
    /// Set the number of threads that will be spawned upon building. If it is not specified, it
    /// will be the maximum number of threads that can be spawned.
    ///
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Panics
    ///
    /// This method will panic if `num_workers` is 0.
    ///
    /// # Examples
    ///
    /// No more than eight threads will be alive simultaneously for this pool:
    ///
    /// ```
    /// use std::thread;
    ///
    /// let pool = lft_rust::lft_builder()
    ///     .num_workers(8)
    ///     .build();
    ///
    /// for _ in 0..42 {
    ///     pool.execute(|| {
    ///         println!("Hello from a worker thread!")
    ///     })
    /// }
    /// ```
    pub fn num_workers(mut self, num_workers: usize) -> Builder {
        assert!(num_workers > 0);
        self.num_workers = Some(num_workers);
        self
    }

    /// Set the maximum number of worker-threads that will be alive at any given moment by the built
    /// [`ThreadPool`]. If not specified, defaults the number of threads to the number of CPUs.
    ///
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Panics
    ///
    /// This method will panic if `max_thread_count` is 0.
    ///
    /// # Examples
    ///
    /// No more than eight threads will be alive simultaneously for this pool:
    ///
    /// ```
    /// use std::thread;
    ///
    /// let pool = threadpool::builder()
    ///     .max_thread_count(8)
    ///     .build();
    ///
    /// for _ in 0..42 {
    ///     pool.execute(|| {
    ///         println!("Hello from a worker thread!")
    ///     })
    /// }
    /// ```
    pub fn max_thread_count(mut self, max_thread_count: usize) -> Builder {
        assert!(max_thread_count > 0);
        self.max_thread_count = Some(max_thread_count);
        self
    }

    /// Set the thread name for each of the threads spawned by the built [`ThreadPool`]. If not
    /// specified, threads spawned by the thread pool will be unnamed.
    ///
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Examples
    ///
    /// Each thread spawned by this pool will have the name "foo":
    ///
    /// ```
    /// use std::thread;
    ///
    /// let pool = lft_rust::lft_builder()
    ///     .worker_name("foo")
    ///     .build();
    ///
    /// for _ in 0..100 {
    ///     pool.execute(|| {
    ///         assert_eq!(thread::current().name(), Some("foo"));
    ///     })
    /// }
    /// ```
    pub fn worker_name<S: AsRef<str>>(mut self, name: S) -> Builder {
        // TODO save the copy with Into<String>
        self.worker_name = Some(name.as_ref().to_owned());
        self
    }

    /// Set the stack size (in bytes) for each of the threads spawned by the built [`ThreadPool`].
    /// If not specified, threads spawned by the threadpool will have a stack size [as specified in
    /// the `std::thread` documentation][thread].
    ///
    /// [thread]: https://doc.rust-lang.org/nightly/std/thread/index.html#stack-size
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Examples
    ///
    /// Each thread spawned by this pool will have a 4 MB stack:
    ///
    /// ```
    /// let pool = lft_rust::lft_builder()
    ///     .thread_stack_size(4096 * 1024)
    ///     .build();
    ///
    /// for _ in 0..100 {
    ///     pool.execute(|| {
    ///         println!("This thread has a 4 MB stack size!");
    ///     })
    /// }
    /// ```
    pub fn thread_stack_size(mut self, size: usize) -> Builder {
        self.thread_stack_size = Some(size);
        self
    }

    /// Finalize the [`Builder`] and build the [`ThreadPool`].
    ///
    /// [`Builder`]: struct.Builder.html
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Examples
    ///
    /// ```
    /// let pool = lft_rust::lft_builder()
    ///     .num_workers(8)
    ///     .thread_stack_size(16*1024*1024)
    ///     .build();
    /// ```
    pub fn build(self) -> ThreadPool {
        let mut num_workers = self.num_workers.unwrap_or_else(num_cpus::get);
        let max_thread_count = self.max_thread_count.unwrap_or_else(|| {num_workers});
        if max_thread_count < num_workers {
            warn!("Number of works is larger than max thread number, shrinking 
                     the thread pool to max thread number {}.", max_thread_count);
            num_workers = max_thread_count;
        }

        let mut num_jobs_list: Vec<Arc<AtomicUsize>> = Vec::with_capacity(max_thread_count);
        let mut thread_closing_list: Vec<Arc<AtomicI8>> = Vec::with_capacity(max_thread_count);
        let mut sender_list: Vec<Sender<Thunk<'static>>> = Vec::with_capacity(max_thread_count);
        let mut receiver_list: Vec<Arc<Receiver<Thunk<'static>>>> = Vec::with_capacity(max_thread_count);
        for i in 0..max_thread_count {
            let (tx, rx) = unbounded::<Thunk<'static>>();
            num_jobs_list.push(Arc::new(AtomicUsize::new(0)));
            sender_list.push(tx);
            receiver_list.push(Arc::new(rx));
            if i < num_workers {
                thread_closing_list.push(Arc::new(AtomicI8::new(1)));
            } else {
                thread_closing_list.push(Arc::new(AtomicI8::new(3)));
            }
        }

        let context = Arc::new(ThreadPoolContext {
            queued_count: num_jobs_list.clone(),
            thread_closing: thread_closing_list.clone(),
            senders: sender_list,
            receivers: receiver_list.clone(),
        });

        let shared_data = Arc::new(ThreadPoolSharedData {
            name: self.worker_name,
            // job_receiver: Mutex::new(rx),
            empty_condvar: Condvar::new(),
            empty_trigger: Mutex::new(()),
            join_generation: AtomicUsize::new(0),
            queued_count: AtomicUsize::new(0),
            active_count: AtomicUsize::new(0),
            num_workers: AtomicUsize::new(num_workers),
            max_thread_count: AtomicUsize::new(max_thread_count),
            panic_count: AtomicUsize::new(0),
            stack_size: self.thread_stack_size,
        });

        // Threadpool threads
        let sleep_duration = time::Duration::from_millis(8);
        for i in 0..max_thread_count {
            spawn_in_pool(shared_data.clone(), 
                          receiver_list[i].clone(), 
                          num_jobs_list[i].clone(),
                          thread_closing_list[i].clone());

            while thread_closing_list[i].load(Ordering::SeqCst) != 0 {
                thread::sleep(sleep_duration);
            }
        }

        ThreadPool {
            // jobs: tx,
            shared_data: shared_data,
            context: context,
        }
    }
}

struct ThreadPoolSharedData {
    name: Option<String>,
    empty_trigger: Mutex<()>,
    empty_condvar: Condvar,
    join_generation: AtomicUsize,
    queued_count: AtomicUsize,
    active_count: AtomicUsize,
    num_workers: AtomicUsize,
    max_thread_count: AtomicUsize,
    panic_count: AtomicUsize,
    stack_size: Option<usize>,
}

impl ThreadPoolSharedData {
    fn has_work(&self) -> bool {
        self.queued_count.load(Ordering::SeqCst) > 0 || self.active_count.load(Ordering::SeqCst) > 0
    }

    /// Notify all observers joining this pool if there is no more work to do.
    fn no_work_notify_all(&self) {
        if !self.has_work() {
            *self
                .empty_trigger
                .lock()
                .expect("Unable to notify all joining threads");
            self.empty_condvar.notify_all();
        }
    }
}

struct ThreadPoolContext {
    queued_count: Vec<Arc<AtomicUsize>>,
    senders: Vec<Sender<Thunk<'static>>>,
    receivers: Vec<Arc<Receiver<Thunk<'static>>>>,
    thread_closing: Vec<Arc<AtomicI8>>,
}

/// Abstraction of a thread pool for basic parallelism.
pub struct ThreadPool {
    // How the threadpool communicates with subthreads.
    //
    // This is the only such Sender, so when it is dropped all subthreads will
    // quit.
    // jobs: Sender<Thunk<'static>>,
    shared_data: Arc<ThreadPoolSharedData>,
    context: Arc<ThreadPoolContext>,
}

impl ThreadPool {
    /// Executes the function `job` on a thread in the pool.
    ///
    /// # Examples
    ///
    /// Execute four jobs on a thread pool that can run two jobs concurrently:
    ///
    /// ```
    /// let pool = lft_rust::lft_auto_config();
    /// pool.execute(|| println!("hello"));
    /// pool.execute(|| println!("world"));
    /// pool.execute(|| println!("foo"));
    /// pool.execute(|| println!("bar"));
    /// pool.join();
    /// ```
    pub fn execute<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let max_thread_count = self.shared_data.max_thread_count.load(Ordering::Relaxed);

        loop {
            let mut target_thread_id = max_thread_count + 1;
            let mut min_jobs_counted: usize = 0;
            for i in 0..max_thread_count {
                if self.context.thread_closing[i].load(Ordering::Relaxed) > 0 {
                    // The thread is closed or about to close.
                    continue;
                }
                if self.context.queued_count[i].load(Ordering::SeqCst) == 0 {
                    target_thread_id = i;
                    break;
                }
                if target_thread_id > max_thread_count 
                    || self.context.queued_count[i].load(Ordering::SeqCst) < min_jobs_counted {
                    target_thread_id = i;
                    min_jobs_counted = self.context.queued_count[i].load(Ordering::Relaxed);
                }
            }


            if target_thread_id < max_thread_count && 
                self.context.thread_closing[target_thread_id].load(Ordering::SeqCst) == 0 {
                // trace!("Target thread id: {}.", target_thread_id);
                self.shared_data.queued_count.fetch_add(1, Ordering::SeqCst);
                self.context.queued_count[target_thread_id].fetch_add(1, Ordering::SeqCst);
                self.context.senders[target_thread_id]
                    .send(Box::new(job))
                    .expect("ThreadPool::execute unable to send job into queue.");
                break;
            }
            let ten_millis = time::Duration::from_millis(10);
            thread::sleep(ten_millis);
        }
    }

    /// Returns the number of jobs waiting to executed in the pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let pool = lft_rust::lft_builder()
    ///                     .num_workers(2)
    ///                     .build();
    /// for _ in 0..10 {
    ///     pool.execute(|| {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// }
    ///
    /// sleep(Duration::from_secs(1)); // wait for threads to start
    /// assert_eq!(8, pool.queued_count());
    /// ```
    pub fn queued_count(&self) -> usize {
        self.shared_data.queued_count.load(Ordering::Relaxed)
    }

    /// Returns the number of currently active worker threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let pool = lft_rust::lft_builder()
    ///                     .num_workers(4)
    ///                     .build();
    /// for _ in 0..10 {
    ///     pool.execute(move || {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// }
    ///
    /// sleep(Duration::from_secs(1)); // wait for threads to start
    /// assert_eq!(4, pool.active_count());
    /// ```
    pub fn active_count(&self) -> usize {
        self.shared_data.active_count.load(Ordering::SeqCst)
    }

    /// Returns the maximum number of threads the pool will execute concurrently.
    ///
    /// # Examples
    ///
    /// ```
    /// let pool = lft_rust::lft_builder()
    ///                     .num_workers(4)
    ///                     .build();
    /// assert_eq!(4, pool.max_count());
    ///
    /// pool.set_num_workers(8);
    /// assert_eq!(8, pool.max_count());
    /// ```
    pub fn max_count(&self) -> usize {
        self.shared_data.max_thread_count.load(Ordering::Relaxed)
    }

    /// Returns the number of workers running in the threadpool.
    pub fn num_workers(&self) -> usize {
        self.shared_data.num_workers.load(Ordering::Relaxed)
    }

    /// Returns the number of panicked threads over the lifetime of the pool.
    ///
    /// # Examples
    ///
    /// ```
    /// let pool = lft_rust::lft_builder()
    ///                     .num_workers(4)
    ///                     .build();
    /// for n in 0..10 {
    ///     pool.execute(move || {
    ///         // simulate a panic
    ///         if n % 2 == 0 {
    ///             panic!()
    ///         }
    ///     });
    /// }
    /// pool.join();
    ///
    /// assert_eq!(5, pool.panic_count());
    /// ```
    pub fn panic_count(&self) -> usize {
        self.shared_data.panic_count.load(Ordering::Relaxed)
    }

    /// Sets the number of worker-threads to use as `num_workers`.
    /// Can be used to change the threadpool size during runtime.
    /// Will not abort already running or waiting threads.
    ///
    /// # Panics
    ///
    /// This function will panic if `num_workers` is 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let  pool = threadpool::builder().num_workers(4).build();
    /// for _ in 0..10 {
    ///     pool.execute(move || {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// }
    ///
    /// sleep(Duration::from_secs(1)); // wait for threads to start
    /// assert_eq!(4, pool.active_count());
    /// assert_eq!(6, pool.queued_count());
    ///
    /// // Increase thread capacity of the pool
    /// pool.set_num_workers(8);
    ///
    /// sleep(Duration::from_secs(1)); // wait for new threads to start
    /// assert_eq!(8, pool.active_count());
    /// assert_eq!(2, pool.queued_count());
    ///
    /// // Decrease thread capacity of the pool
    /// // No active threads are killed
    /// pool.set_num_workers(4);
    ///
    /// assert_eq!(8, pool.active_count());
    /// assert_eq!(2, pool.queued_count());
    /// ```
    // pub fn set_num_workers(&self, num_workers: usize) {
    //     assert!(num_workers >= 1);
    //     let prev_num_workers = self
    //         .shared_data
    //         .max_thread_count
    //         .swap(num_workers, Ordering::Release);
    //     if let Some(num_spawn) = num_workers.checked_sub(prev_num_workers) {
    //         // Spawn new threads
    //         for _ in 0..num_spawn {
    //             spawn_in_pool(self.shared_data.clone());
    //         }
    //     }
    // }

    /// Spawn an extra worker thread. Can be used to increase the number of
    /// work threads during runtimes. If the number of threads is already 
    /// the maximum, it will print out an warning.
    ///
    /// # Examples
    /// ```
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let  pool = threadpool::builder().num_workers(4).build();
    /// for _ in 0..10 {
    ///     pool.execute(move || {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// pool.spawn_extra_one_worker();
    /// ```
    pub fn spawn_extra_one_worker(&self) {
        if self.shared_data.num_workers.load(Ordering::Acquire) 
            >= self.shared_data.max_thread_count.load(Ordering::Relaxed) {
                warn!("Max thread number exceeded.");
                ()
        } 
        self.shared_data.num_workers.fetch_add(1, Ordering::SeqCst);

        let mut spawn_completed = false;
        while !spawn_completed {
            let max_thread_count = self.shared_data.max_thread_count.load(Ordering::Relaxed);
            for i in 0..max_thread_count {
                if self.context.thread_closing[i].compare_exchange(3, 
                                                                   1, 
                                                                   Ordering::SeqCst, 
                                                                   Ordering::Relaxed) == Ok(3) {
                    // If one thread is closed, we will try to open this thread.
                    spawn_in_pool(self.shared_data.clone(), 
                                  self.context.receivers[i].clone(), 
                                  self.context.queued_count[i].clone(),
                                  self.context.thread_closing[i].clone());
                    spawn_completed = true;
                    break;
                }
            }

            if self.shared_data.num_workers.load(Ordering::SeqCst) 
                >= self.shared_data.max_thread_count.load(Ordering::Relaxed) {
                    warn!("Max thread number exceeded.");
                    break;
            } 
        }

    }

    /// Shutdown a worker thread in the threadpool. 
    /// Can be used to increase the number of work threads during runtimes. 
    /// If the number of threads is already 0, it will print out an warning.
    ///
    /// # Examples
    /// ```
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let  pool = threadpool::builder().num_workers(4).build();
    /// for _ in 0..10 {
    ///     pool.execute(move || {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// pool.spawn_extra_one_worker();
    /// ```
    pub fn shutdown_one_worker(&self) {
        if self.shared_data.num_workers.load(Ordering::SeqCst) <= 0 {
            warn!("No thread to shutdown");
            ()
        }
        self.shared_data.num_workers.fetch_sub(1, Ordering::SeqCst);

        loop {
            let max_thread_count = self.shared_data.max_thread_count.load(Ordering::Relaxed);
            let mut target_thread_id = max_thread_count + 1;
            let mut min_num_of_jobs = 0;

            for i in 0..max_thread_count {
                if self.context.thread_closing[i].load(Ordering::Relaxed) > 0 {
                    continue;
                }

                if target_thread_id > max_thread_count || 
                    min_num_of_jobs > self.context.queued_count[i].load(Ordering::Relaxed) {
                        target_thread_id = i;
                        min_num_of_jobs = self.context.queued_count[i].load(Ordering::Acquire);
                }
            }

            // CAS to check if the target thread is still running.
            if target_thread_id < max_thread_count && 
                self.context.thread_closing[target_thread_id].compare_exchange(0,
                                                                               2,
                                                                               Ordering::SeqCst,
                                                                               Ordering::Relaxed) == Ok(0) {
                    trace!("Closing thread id: {}.", target_thread_id);
                    break;
            }
           
            if self.shared_data.num_workers.load(Ordering::SeqCst) <= 0 {
                warn!("No thread to shutdown");
                break;
            }
        }
    }

    /// Block the current thread until all jobs in the pool have been executed.
    ///
    /// Calling `join` on an empty pool will cause an immediate return.
    /// `join` may be called from multiple threads concurrently.
    /// A `join` is an atomic point in time. All threads joining before the join
    /// event will exit together even if the pool is processing new jobs by the
    /// time they get scheduled.
    ///
    /// Calling `join` from a thread within the pool will cause a deadlock. This
    /// behavior is considered safe.
    ///
    /// **Note:** Join will not stop the worker threads. You will need to `drop`
    /// all instances of `ThreadPool` for the worker threads to terminate.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// let pool = lft_rust::lft_auto_config();
    /// let test_count = Arc::new(AtomicUsize::new(0));
    ///
    /// for _ in 0..42 {
    ///     let test_count = test_count.clone();
    ///     pool.execute(move || {
    ///         test_count.fetch_add(1, Ordering::Relaxed);
    ///     });
    /// }
    ///
    /// pool.join();
    /// assert_eq!(42, test_count.load(Ordering::Relaxed));
    /// ```
    pub fn join(&self) {
        // fast path requires no mutex
        if self.shared_data.has_work() == false {
            return ();
        }

        let generation = self.shared_data.join_generation.load(Ordering::SeqCst);
        let mut lock = self.shared_data.empty_trigger.lock().unwrap();

        while generation == self.shared_data.join_generation.load(Ordering::Relaxed)
            && self.shared_data.has_work()
        {
            lock = self.shared_data.empty_condvar.wait(lock).unwrap();
        }

        // increase generation if we are the first joining thread to come out of the loop
        let _ = self.shared_data.join_generation.compare_exchange(
            generation,
            generation.wrapping_add(1),
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }
}

impl Clone for ThreadPool {
    /// Cloning a pool will create a new handle to the pool.
    /// The behavior is similar to [Arc](https://doc.rust-lang.org/stable/std/sync/struct.Arc.html).
    ///
    /// We could for example submit jobs from multiple threads concurrently.
    ///
    /// ```
    /// use std::thread;
    /// use crossbeam_channel::unbounded;
    ///
    /// let pool = lft_rust::lft_builder()
    ///                 .worker_name("clone example")
    ///                 .num_workers(2)
    ///                 .build();
    ///
    /// let results = (0..2)
    ///     .map(|i| {
    ///         let pool = pool.clone();
    ///         thread::spawn(move || {
    ///             let (tx, rx) = unbounded();
    ///             for i in 1..12 {
    ///                 let tx = tx.clone();
    ///                 pool.execute(move || {
    ///                     tx.send(i).expect("channel will be waiting");
    ///                 });
    ///             }
    ///             drop(tx);
    ///             if i == 0 {
    ///                 rx.iter().fold(0, |accumulator, element| accumulator + element)
    ///             } else {
    ///                 rx.iter().fold(1, |accumulator, element| accumulator * element)
    ///             }
    ///         })
    ///     })
    ///     .map(|join_handle| join_handle.join().expect("collect results from threads"))
    ///     .collect::<Vec<usize>>();
    ///
    /// assert_eq!(vec![66, 39916800], results);
    /// ```
    fn clone(&self) -> ThreadPool {
        ThreadPool {
            // jobs: self.jobs.clone(),
            shared_data: self.shared_data.clone(),
            context: self.context.clone(),
        }
    }
}

impl fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadPool")
            .field("name", &self.shared_data.name)
            .field("queued_count", &self.queued_count())
            .field("active_count", &self.active_count())
            .field("max_count", &self.max_count())
            .field("num_workers", &self.num_workers())
            .finish()
    }
}

impl PartialEq for ThreadPool {
    /// Check if you are working with the same pool
    ///
    /// ```
    /// let a = lft_rust::lft_auto_config();
    /// let b = lft_rust::lft_auto_config();
    ///
    /// assert_eq!(a, a);
    /// assert_eq!(b, b);
    ///
    /// assert_ne!(a, b);
    /// assert_ne!(b, a);
    /// ```
    fn eq(&self, other: &ThreadPool) -> bool {
        Arc::ptr_eq(&self.shared_data, &other.shared_data)
    }
}
impl Eq for ThreadPool {}

fn spawn_in_pool(shared_data: Arc<ThreadPoolSharedData>, 
                 receiver: Arc<Receiver<Thunk<'static>>>,
                 num_jobs: Arc<AtomicUsize>,
                 thread_closing: Arc<AtomicI8>) {
    let mut builder = thread::Builder::new();
    if let Some(ref name) = shared_data.name {
        builder = builder.name(name.clone());
    }
    if let Some(ref stack_size) = shared_data.stack_size {
        builder = builder.stack_size(stack_size.to_owned());
    }
    builder
        .spawn(move || {
            // Will spawn a new thread on panic unless it is cancelled.
            let sentinel = Sentinel::new(&shared_data, &receiver, &num_jobs, &thread_closing);
            // thread_closing.swap(0, Ordering::SeqCst);

            if thread_closing.compare_exchange(1, 0, Ordering::SeqCst, Ordering::Relaxed) == Ok(1) {
                loop {
                    // Shutdown this thread if the pool has become smaller
                    // let thread_counter_val = shared_data.num_workers.load(Ordering::Acquire);
                    // let max_thread_count_val = shared_data.max_thread_count.load(Ordering::Relaxed);
                    if thread_closing.load(Ordering::SeqCst) == 2
                        && num_jobs.load(Ordering::SeqCst) == 0 {
                        break;
                    }
                    let message = {
                        // Each thread will have a job queue, thread will fetch its own
                        // work from its job queue.
                        receiver.recv()
                    };

                    let job = match message {
                        Ok(job) => job,
                        // The ThreadPool was dropped.
                        Err(..) => break,
                    };
                    // Do not allow IR around the job execution
                    shared_data.active_count.fetch_add(1, Ordering::SeqCst);

                    job.call_box();

                    num_jobs.fetch_sub(1, Ordering::SeqCst);
                    shared_data.queued_count.fetch_sub(1, Ordering::SeqCst);
                    shared_data.active_count.fetch_sub(1, Ordering::SeqCst);
                    if num_jobs.load(Ordering::SeqCst) == 0 {
                        shared_data.no_work_notify_all();
                    }
                }

                if thread_closing.compare_exchange(0, 
                                                   3, 
                                                   Ordering::SeqCst, 
                                                   Ordering::Relaxed) == Ok(0) {
                    shared_data.num_workers.fetch_sub(1, Ordering::SeqCst);
                } else {
                    let _ = thread_closing.compare_exchange(2, 
                                                            3, 
                                                            Ordering::SeqCst, 
                                                            Ordering::Relaxed);
                }
            }

            sentinel.cancel();
        })
        .unwrap();
}
