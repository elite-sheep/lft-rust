// Copyright @yucwang 2022

use async_std::task::{ Task };

pub struct SequentialTaskQueue<'a> {
    tasks: Vec<&'a Task>,
}

impl<'a> SequentialTaskQueue<'a> {
    pub fn new(capacity: usize) -> Self {
        SequentialTaskQueue { 
            tasks: Vec::with_capacity(capacity),
        }
    }

    pub fn length(&self) -> usize {
        self.tasks.len()
    }

    pub fn enqueue(&mut self, new_task: &'a Task) {
        self.tasks.push(new_task)
    }

    pub fn dequeue(&mut self) -> Option<&Task> {
        if self.is_empty() {
            None
        } else {
            Some(self.tasks.remove(0))
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn peek(&self) -> Option<&&Task> {
        if self.is_empty() {
            None
        } else {
            self.tasks.first()
        }
    }
}
