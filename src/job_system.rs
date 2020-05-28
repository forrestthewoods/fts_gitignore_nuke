use crossbeam_deque::{Injector, Stealer, Worker};

pub fn run_job<IN,OUT,JOB>(initial: Vec<IN>, job: JOB, num_workers: usize) -> Vec<OUT> 
where 
    IN: Send,
    OUT: Send,
    JOB: Fn(IN, &Worker<IN>) -> Option<OUT> + Clone + Send,
{
    let injector = Injector::new();
    let workers : Vec<_> = (0..num_workers).map(|_| Worker::new_lifo()).collect();
    let stealers : Vec<_> = workers.iter().map(|w| w.stealer()).collect();
    let active_counter = crate::ActiveCounter::new();

    // Seed injector
    for item in initial.into_iter() {
        injector.push(item);
    }

    let result : Vec<OUT> = crossbeam_utils::thread::scope(|scope|
    {   
        let mut scopes : Vec<_> = Default::default();

        for worker in workers.into_iter() {
            let injector_borrow = &injector;
            let stealers_copy = stealers.clone();
            let job_copy = job.clone();
            let mut counter_copy = active_counter.clone();

            let s = scope.spawn(move |_| {
                let backoff = crossbeam_utils::Backoff::new();

                let mut worker_results : Vec<_> = Default::default();

                // Loop until all workers idle
                loop {
                    // Do work
                    {
                        let _work_token = counter_copy.take_token();
                        while let Some(item) = find_task(&worker, injector_borrow, &stealers_copy) {
                            backoff.reset();

                            if let Some(result) = job_copy(item, &worker) {
                                worker_results.push(result);
                            }
                        } 
                    }

                    backoff.spin();

                    if counter_copy.is_zero() {
                        break;
                    }
                }

                worker_results
            });

            scopes.push(s);
        }

        scopes.into_iter()
            .filter_map(|s| s.join().ok())
            .flatten()
            .collect()
    }).unwrap();

    result
}

fn find_task<T>(
    local: &Worker<T>,
    global: &Injector<T>,
    stealers: &[Stealer<T>],
) -> Option<T> {
    // Pop a task from the local queue, if not empty.
    local.pop().or_else(|| {
        // Otherwise, we need to look for a task elsewhere.
        std::iter::repeat_with(|| {
            // Try stealing a batch of tasks from the global queue.
            global.steal_batch_and_pop(local)
                // Or try stealing a task from one of the other threads.
                .or_else(|| stealers.iter().map(|s| s.steal()).collect())
        })
        // Loop while no task was stolen and any steal operation needs to be retried.
        .find(|s| !s.is_retry())
        // Extract the stolen task, if there is one.
        .and_then(|s| s.success())
    })
}

#[test]
fn basic_scheduler() {
    let data = vec![3];
    let job = |x, worker: &Worker<_>| -> Option<i32> {
        if x > 0 {
            worker.push(x-1);
            Some(x*2)
        } else {
            None
        }
    };

    let result = run_job(data, job, 1);
    assert_eq!(result, vec![6,4,2]);
}

#[test]
fn bigger_scheduler() {
    let data = vec![1,2,3,4,5,6,7,8,9,10];
    let job = |x, worker: &Worker<_>| -> Option<i32> {
        if x > 0 {
            worker.push(x-1);
            Some(x*2)
        } else {
            None
        }
    };

    let expected_result : i32 = data.iter()
        .map(|x| (x*(x+1)))
        .sum();

    let result = run_job(data, job, 1);
    assert_eq!(result.iter().sum::<i32>(), expected_result);
}