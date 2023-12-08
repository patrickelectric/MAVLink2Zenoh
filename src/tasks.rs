use std::collections::HashMap;
use std::future::Future;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

// More information about this can be detailed explained here:
// https://www.youtube.com/watch?v=tP0ZrX-2EiE
pub struct TaskMaster {
    runtime: Runtime,
    tasks: HashMap<String, JoinHandle<()>>,
}

impl TaskMaster {
    pub fn new() -> Self {
        Self {
            runtime: Runtime::new().unwrap(),
            tasks: HashMap::new(),
        }
    }

    pub fn spawn<F>(&mut self, name: String, f: F)
    where
        F: Future<Output = ()> + Send + 'static,
        F::Output: Send + 'static,
    {
        let task = self.runtime.spawn(f);
        self.tasks.insert(name.clone(), task);
    }

    pub fn clear_finished(&mut self) {
        self.tasks.retain(|_, task| !task.is_finished());
    }

    pub fn get_task(&self, name: &str) -> Option<&JoinHandle<()>> {
        self.tasks.get(name)
    }

    pub fn list_running_tasks(&mut self) -> Vec<String> {
        self.clear_finished();
        self.tasks.keys().cloned().collect()
    }
}

impl Drop for TaskMaster {
    fn drop(&mut self) {
        self.clear_finished();

        loop {
            let running_tasks = self.list_running_tasks();
            if running_tasks.is_empty() {
                break;
            }

            println!("Waiting for tasks to finish: {:?}", running_tasks);
            std::thread::sleep(std::time::Duration::from_millis(2000));
        }
    }
}
