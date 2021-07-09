//! This module has containers for storing the tasks spawned on a scheduler. The
//! `OwnedTasks` container is thread-safe but can only store tasks that
//! implement Send. The `LocalOwnedTasks` container is not thread safe, but can
//! store non-Send tasks.
//!
//! The collections can be closed to prevent adding new tasks during shutdown of
//! the scheduler with the collection.

use crate::loom::sync::Mutex;
use crate::runtime::task::{Task, UnboundTask, Notified, Schedule};
use crate::util::linked_list::{Link, LinkedList};

use std::marker::PhantomData;
use std::future::Future;

pub(crate) struct OwnedTasks<S: 'static> {
    inner: Mutex<OwnedTasksInner<S>>,
}
struct OwnedTasksInner<S: 'static> {
    list: LinkedList<Task<S>, <Task<S> as Link>::Target>,
    closed: bool,
}

pub(crate) struct LocalOwnedTasks<S: 'static> {
    list: LinkedList<Task<S>, <Task<S> as Link>::Target>,
    closed: bool,
    _not_send: PhantomData<*const ()>,
}

impl<S: 'static> OwnedTasks<S> {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(OwnedTasksInner {
                list: LinkedList::new(),
                closed: false,
            }),
        }
    }

    /// Bind the provided task to this OwnedTasks instance. This fails if the
    /// OwnedTasks has been closed.
    pub(crate) fn bind<T>(
        &self,
        task: UnboundTask<T, S>,
        scheduler: S
    ) -> Result<Notified<S>, UnboundTask<T, S>>
    where
        S: Schedule,
        T: Future + Send + 'static,
    {
        let mut lock = self.inner.lock();
        if lock.closed {
            Err(task)
        } else {
            let (notified, task) = task.bind_and_split(scheduler);
            lock.list.push_front(task);
            Ok(notified)
        }
    }

    pub(crate) fn pop_back(&self) -> Option<Task<S>> {
        self.inner.lock().list.pop_back()
    }

    /// The caller must ensure that if the provided task is stored in a
    /// linked list, then it is in this linked list.
    pub(crate) unsafe fn remove(&self, task: &Task<S>) -> Option<Task<S>> {
        self.inner.lock().list.remove(task.header().into())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.lock().list.is_empty()
    }

    /// Close the OwnedTasks. This prevents adding new tasks to the collection.
    pub(crate) fn close(&self) {
        self.inner.lock().closed = true;
    }
}

impl<S: 'static> LocalOwnedTasks<S> {
    pub(crate) fn new() -> Self {
        Self {
            list: LinkedList::new(),
            closed: false,
            _not_send: PhantomData,
        }
    }

    pub(crate) fn bind<T>(
        &mut self,
        task: UnboundTask<T, S>,
        scheduler: S
    ) -> Result<Notified<S>, UnboundTask<T, S>>
    where
        S: Schedule,
        T: Future + 'static,
    {
        if self.closed {
            Err(task)
        } else {
            let (notified, task) = task.bind_and_split(scheduler);
            self.list.push_front(task);
            Ok(notified)
        }
    }

    pub(crate) fn pop_back(&mut self) -> Option<Task<S>> {
        self.list.pop_back()
    }

    /// The caller must ensure that if the provided task is stored in a
    /// linked list, then it is in this linked list.
    pub(crate) unsafe fn remove(&mut self, task: &Task<S>) -> Option<Task<S>> {
        self.list.remove(task.header().into())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Close the LocalOwnedTasks. This prevents adding new tasks to the
    /// collection.
    pub(crate) fn close(&mut self) {
        self.closed = true;
    }
}
