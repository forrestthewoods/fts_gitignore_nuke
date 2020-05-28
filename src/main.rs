use anyhow::anyhow;
use clap::{Arg, App};
use crossbeam_deque::{Injector, Stealer, Worker};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use itertools::Itertools;
use num_format::{Locale, ToFormattedString};
use std::{env, fs};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

mod immutable_stack;
use immutable_stack::ImmutableStack;

mod job_system;

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let cpus_str = num_cpus::get_physical().to_string();

    // Parse cmdline args
    let matches = App::new("☢️ fts_gitignore_nuke ☢️")
        .version(".1")
        .author("Forrest S. <forrestthewoods@gmail.com>")
        .about("Deletes files hidden by .gitignore files")
        .arg(Arg::with_name("directory")
            .short("d")
            .long("directory")
            .value_name("DIRECTORY")
            .help("Root directory to start search"))
        .arg(Arg::with_name("min_file_size")
            .short("mfs")
            .long("min_file_size")
            .value_name("MIN_FILE_SIZE")
            .help("Minimum size, in bytes, to nuke")
            .default_value("0"))
        .arg(Arg::with_name("num_threads")
            .short("t")
            .long("num_threads")
            .value_name("NUM_THREADS")
            .help("Number of threads to use")
            .default_value(&cpus_str))
        .arg(Arg::with_name("benchmark")
            .short("b")
            .long("bench")
            .value_name("BENCHMARK")
            .help("Run benchmark. Auto-quit after walking directory")
            .required(false)
            .takes_value(false))
        .get_matches();

    let benchmark_mode = matches.is_present("benchmark");
    
    // Determine starting dir
    let starting_dir : std::path::PathBuf = match matches.value_of_os("directory") {
        Some(path) => path.into(),
        None => env::current_dir()?
    };
    
    // Verify starting dir is valid
    if !starting_dir.exists() {
        return Err(anyhow!("Directory [{:?}] does not exist", starting_dir));
    } else if !starting_dir.is_dir() {
        return Err(anyhow!("[{:?}] is not a directory", starting_dir));
    }
    let starting_dir = std::fs::canonicalize(starting_dir)?;

    println!("🔍 scanning for targets from [{:?}]", starting_dir);

    let min_filesize_in_bytes : u64 = matches.value_of("min_file_size").unwrap().parse().unwrap();

    // Start .gitignore stack with an empty root
    let ignore_stack = ImmutableStack::new();
    let mut ignore_tip = ignore_stack.clone();
    
    // Add global ignore (if exists)
    let (global_ignore, err) = GitignoreBuilder::new(starting_dir.clone()).build_global();
    if err.is_none() && global_ignore.num_ignores() > 0 {
        ignore_tip = ignore_stack.push(global_ignore);
    }

    // Search for ignores in parent directories
    let mut parent_ignores : Vec<_>  = Default::default();
    let mut dir : &std::path::Path = &starting_dir;
    while let Some(parent_path) = dir.parent() {
        let ignore_path = parent_path.join(".gitignore");
        if ignore_path.exists() {
            let mut ignore_builder = GitignoreBuilder::new(parent_path);
            ignore_builder.add(ignore_path);
            if let Ok(ignore) = ignore_builder.build() {
                parent_ignores.push(ignore);
            }
        }

        dir = parent_path;
    }

    // Push parent ignores onto ignore_stack
    for ignore in parent_ignores.into_iter().rev() {
        ignore_tip = ignore_stack.push(ignore);
    }

    let initial_data = vec![(ignore_tip, starting_dir)];
    let job = |(mut ignore_tip, entry): (ImmutableStack<Gitignore>, PathBuf), worker: &Worker<_>| -> Option<Vec<PathBuf>> {
        
        let mut job_ignores : Vec<_> = Default::default();
        let mut dirs : Vec<std::path::PathBuf> = Default::default();

        // Process each child in directory
        let read_dir = match fs::read_dir(entry) {
            Ok(inner) => inner,
            Err(_) => return None,
        };

        for child in read_dir {
            let child_path = match child {
                Ok(child) => child.path(),
                Err(_) => continue
            };

            let child_meta = match std::fs::metadata(&child_path) {
                Ok(inner) => inner,
                Err(_) => continue
            };

            // Child is file
            if child_meta.is_file() {
                // Child is ignorefile. Push it onto ignore stack
                if child_path.file_name().unwrap() == ".gitignore" {
                    // Parse new .gitignore
                    let parent_path = match child_path.parent() {
                        Some(parent_path) => parent_path,
                        None => continue
                    };

                    let mut ignore_builder = GitignoreBuilder::new(parent_path);
                    ignore_builder.add(child_path);
                    let new_ignore = match ignore_builder.build() {
                        Ok(ignore) => ignore,
                        Err(_) => continue
                    };

                    ignore_tip = ignore_tip.push(new_ignore);
                } else {
                    // Child is file. Perform ignore test.
                    let is_ignored = ignore_tip.iter()
                        .map(|i| i.matched(&child_path, true))
                        .any(|m| m.is_ignore());

                    if is_ignored {
                        job_ignores.push(child_path);
                    }
                }
            } else {
                // Child is directory. Add to deferred dirs list
                dirs.push(child_path);
            }
        }

        // Process directories
        for dir in dirs.into_iter() {

            // Check if ignored
            let is_ignored = ignore_tip.iter()
                .map(|i| i.matched(&dir, true))
                .any(|m| m.is_ignore());

            if is_ignored {
                job_ignores.push(dir);
            } else {
                worker.push((ignore_tip.clone(), dir));
            }
        }
        
        Some(job_ignores)
    };

    let num_threads : usize = matches.value_of("num_threads").unwrap().parse().unwrap();
    let job_result = job_system::run_job(initial_data, job, num_threads);
    let ignored_paths : Vec<PathBuf> = job_result.into_iter()
        .flatten()
        .collect();

/*
    // Create crossbeam structs for work-stealing job queue
    let num_threads : usize = matches.value_of("num_threads").unwrap().parse().unwrap();
    let injector = Injector::new();
    let workers : Vec<_> = (0..num_threads).map(|_| Worker::new_lifo()).collect();
    let stealers : Vec<_> = workers.iter().map(|w| w.stealer()).collect();
    let active_counter = ActiveCounter::new();
    
    // Seed injector
    injector.push((ignore_tip, starting_dir));

    let ignored_paths : Vec<std::path::PathBuf> = crossbeam_utils::thread::scope(|scope|
    {   
        let mut scopes : Vec<_> = Default::default();

        for worker in workers.into_iter() {
            let injector_borrow = &injector;
            let stealers_copy = stealers.clone();
            let mut counter_copy = active_counter.clone();

            let s = scope.spawn(move |_| {
                let backoff = crossbeam_utils::Backoff::new();
                let mut worker_ignores : Vec<_> = Default::default();

                // Loop until all workers idle
                loop {
                    // Do work
                    {
                        let _work_token = counter_copy.take_token();
                        while let Some((mut ignore_tip, entry)) = find_task(&worker, injector_borrow, &stealers_copy) {
                            backoff.reset();

                            || -> anyhow::Result<()> {
                                // Dirs must be deferred so they can consider potential ignore files
                                let mut dirs : Vec<std::path::PathBuf> = Default::default();

                                // Process each child in directory
                                let read_dir = match fs::read_dir(entry) {
                                    Ok(inner) => inner,
                                    Err(_) => return Ok(()),
                                };

                                for child in read_dir {
                                    let child_path = child?.path();
                                    let child_meta = match std::fs::metadata(&child_path) {
                                        Ok(inner) => inner,
                                        Err(_) => continue
                                    };

                                    // Child is file
                                    if child_meta.is_file() {
                                        // Child is ignorefile. Push it onto ignore stack
                                        if child_path.file_name().unwrap() == ".gitignore" {
                                            // Parse new .gitignore
                                            let parent_path = child_path.parent()
                                                .ok_or_else(|| anyhow!("Failed to get parent for [{:?}]", child_path))?;
                                            
                                            let mut ignore_builder = GitignoreBuilder::new(parent_path);
                                            ignore_builder.add(child_path);
                                            let new_ignore = ignore_builder.build()?;
                                            ignore_tip = ignore_tip.push(new_ignore);
                                        } else {
                                            // Child is file. Perform ignore test.
                                            let is_ignored = ignore_tip.iter()
                                                .map(|i| i.matched(&child_path, true))
                                                .any(|m| m.is_ignore());

                                            if is_ignored {
                                                worker_ignores.push(child_path);
                                            }
                                        }
                                    } else {
                                        // Child is directory. Add to deferred dirs list
                                        dirs.push(child_path);
                                    }
                                }

                                // Process directories
                                for dir in dirs.into_iter() {

                                    // Check if ignored
                                    let is_ignored = ignore_tip.iter()
                                        .map(|i| i.matched(&dir, true))
                                        .any(|m| m.is_ignore());

                                    if is_ignored {
                                        worker_ignores.push(dir);
                                    } else {
                                        worker.push((ignore_tip.clone(), dir));
                                    }
                                }
                                
                                Ok(())
                            }().unwrap();
                        } 
                    }

                    backoff.spin();

                    if counter_copy.is_zero() {
                        break;
                    }
                }

                worker_ignores
            });

            scopes.push(s);
        }

        let result : Vec<std::path::PathBuf> = scopes.into_iter()
            .map(|s| s.join().unwrap())
            .flatten()
            .collect();
        result

    }).unwrap();
*/

    // Print all ignored content, sorted by bytes
    // TODO: parallelize with rayon
    println!("Ignored:");
    let mut total_bytes = 0;
    let mut final_ignore_paths : Vec<_> = Default::default();
    ignored_paths.into_iter()
        .map(|dir| (dir_size(dir.clone()), dir))
        .filter_map(|(bytes, dir)| if let Ok(bytes) = bytes { Some((bytes, dir)) } else { None })
        .filter(|(bytes, _)| *bytes >= min_filesize_in_bytes)
        .sorted_by_key(|kvp| kvp.0)
        .for_each(|(bytes, path)| {
            total_bytes += bytes;
            println!("  {:10} {:?}", pretty_bytes(bytes), path);
            final_ignore_paths.push(path);
        });
    println!("Total Bytes: {}", total_bytes.to_formatted_string(&Locale::en));
    println!("Search Time: {:?}", start.elapsed());

    if final_ignore_paths.is_empty() {
        println!("No ignore paths to delete.");
        return Ok(());
    }

    // Quit if we're in benchmark mode
    if benchmark_mode {
        return Ok(());
    }

    // Verify nuke
    const NUKE_STRING : &str = "NUKE";
    const QUIT_STRING : &str = "QUIT";

    // Helper to remove either a file or a directory
    let remove_path = |path: &std::path::Path| {
        let meta = match fs::metadata(&path) {
            Ok(meta) => meta,
            Err(e) => {
                println!("Unable to nuke [{:?}]. Failed to query metadata. Error: [{:?}]", path, e);
                return;
            }
        };

        if meta.is_file() {
            // Delete file
            match std::fs::remove_file(&path) {
                Ok(_) => (),
                Err(e) => println!("Unable to nuke file [{:?}]. Error: [{:?}]", path, e)
            };
        } else {
            // Delete directory
            match std::fs::remove_dir_all(&path) {
                Ok(_) => (),
                Err(e) => println!("Unable to nuke directory [{:?}]. Directory may be partially deleted. Error: [{:?}]", path, e)
            };
        }
    };

    // Loop to get confirmation to nuke data or quit
    loop {
        println!("\n⚠️⚠️⚠️ Do you wish to delete? This action can not be undone! ⚠️⚠️⚠️");
        println!("Type {} to proceed, {} to quit:", NUKE_STRING, QUIT_STRING);
        let mut input = String::new();

        std::io::stdin().read_line(&mut input)?;
        let trimmed_input = input.trim();
        if trimmed_input == NUKE_STRING {
            println!("\n☢️☢️☢️ nuclear launch detected ☢️☢️☢️");

            // Delete all the things
            for path in final_ignore_paths {
                remove_path(&path);
            }

            println!("☠️☠️☠️ nuclear deletion complete ☠️☠️☠️");
            break;
        } 
        else if trimmed_input.eq_ignore_ascii_case(QUIT_STRING) {
            println!("😇😇😇 Nuclear launch aborted. Thank you and have a nice day. 😇😇😇");
            break;
        }
        else {
            println!("Invalid input. Input was [{}] but must exactly match [{}] to irrevocably nuke. Please try again.", trimmed_input, NUKE_STRING);
        }
    }

    Ok(())
}

// Recursively calculate size of all files within a single directory
fn dir_size(path: std::path::PathBuf) -> anyhow::Result<u64> {
    // Input path may be file
    let meta = fs::metadata(&path)?;
    if meta.is_file() {
        return Ok(meta.len());
    }
    
    // Recursively compute pathsize
    let mut result = 0;

    let mut dirs = vec![path];
    while !dirs.is_empty() {
        let dir = dirs.pop().unwrap();

        // Process each element in dir
        for child in fs::read_dir(dir)? {
            let child_path = child?.path();
            let meta = fs::metadata(&child_path)?;
            if meta.is_file() {
                // Add file sizes to result
                result += meta.len();
            } else {
                // Add directories to queue for processing
                dirs.push(child_path);
            }
        }
    }

    Ok(result)
}

// Print u64 bytes value as a suffixed string
fn pretty_bytes(orig_amount: u64) -> String {
    let mut amount = orig_amount;
    let mut order = 0;
    while amount > 1000 {
        amount /= 1000;
        order += 1;
    }

    match order {
        0 => format!("{} b", amount),
        1 => format!("{} Kb", amount),
        2 => format!("{} Mb", amount),
        3 => format!("{} Gb", amount),
        4 => format!("{} Tb", amount),
        5 => format!("{} Pb", amount),
        6 => format!("{} Exa", amount),
        7 => format!("{} Zetta", amount),
        8 => format!("{} Yotta", amount),
        _ => format!("{}", orig_amount)
    }
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

// Helpers to track when all workers are done
#[derive(Clone)]
struct ActiveCounter {
    active_count: Arc<AtomicUsize>
}

impl ActiveCounter {
    pub fn take_token(&mut self) -> ActiveToken {
        self.active_count.fetch_add(1, Ordering::SeqCst);
        ActiveToken { active_count: self.active_count.clone() }
    }

    pub fn new() -> ActiveCounter {
        ActiveCounter { active_count: Arc::new(AtomicUsize::new(0)) }
    }

    pub fn is_zero(&self) -> bool {
        self.active_count.load(Ordering::SeqCst) == 0
    }
}


struct ActiveToken {
    active_count: Arc<AtomicUsize>
}

impl Drop for ActiveToken {
    fn drop(&mut self) {
        self.active_count.fetch_sub(1, Ordering::SeqCst);
    }
}

