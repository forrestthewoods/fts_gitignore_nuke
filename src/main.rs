use anyhow::{anyhow, Context};
use cactus::ArcCactus;
use crossbeam_deque::Worker;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use itertools::Itertools;
use num_format::{Locale, ToFormattedString};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs};
use structopt::StructOpt;

mod job_system;

#[derive(StructOpt, Debug)]
#[structopt(
    name = "☢️ fts_gitignore_nuke ☢️",
    author = "Forrest Smith <forrestthewoods@gmail.com>",
    about = "Deletes files hidden by .gitignore files.
If a .gitnuke is found its patterns will be used with higher precedence than any .gitignore from the same directory."
)]
struct Opts {
    #[structopt(short, long, parse(from_os_str), help = "Root directory to start search")]
    directory: Option<PathBuf>,

    #[structopt(
        short,
        long,
        parse(from_os_str),
        help = "Include .gitignores between root and target directory"
    )]
    root: Option<PathBuf>,

    #[structopt(long, default_value = "0", help = "Minimum size, in bytes, to nuke")]
    min_file_size: u64,

    #[structopt(long, help = "Number of threads to use. Default: num physical cores")]
    num_threads: Option<usize>,

    #[structopt(short, long, help = "Auto-quit after walking directory")]
    benchmark: bool,

    #[structopt(long, help = "Prints which glob and which .gitignore matched each path")]
    print_glob_matches: bool,

    #[structopt(long, help = "Include global .gitignore for matches")]
    include_global_ignore: bool,

    #[structopt(long, help = "Prints errors if encountered")]
    print_errors: bool,
}

fn main() -> anyhow::Result<()> {
    let start = Instant::now();

    // Parse args
    let opt = Opts::from_args();
    let num_threads: usize = opt.num_threads.unwrap_or_else(|| num_cpus::get_physical());
    let root: Option<PathBuf> = opt
        .root
        .clone()
        .map(|s| s.into())
        .map(|s: PathBuf| std::fs::canonicalize(s).unwrap());

    // Helpers to print Result error if print_errors is true
    let check_error = |e: anyhow::Result<()>| {
        if opt.print_errors {
            if let Err(e) = e {
                println!("Error: [{:#}]", e);
            }
        }
    };

    // Helper to generate a context string if print_errors is true
    macro_rules! path_context {
        ($func:expr, $path:expr) => {{
            if opt.print_errors {
                format!("{} {}", $func, $path.display())
            } else {
                "".to_owned()
            }
        }};
    }

    // Helper to add .gitignore and .gitnuke files
    let build_ignore = |dir: &Path, filename: &str| -> Option<Gitignore> {
        let ignore_path = dir.join(filename);
        if ignore_path.exists() {
            let mut ignore_builder = GitignoreBuilder::new(dir);
            ignore_builder.add(ignore_path.clone());
            if let Ok(ignore) = ignore_builder.build() {
                if opt.print_glob_matches {
                    println!("Loaded: [{}]", ignore_path.display());
                }
                return Some(ignore);
            }
        }
        None
    };

    // Determine starting dir
    let starting_dir: std::path::PathBuf = match &opt.directory {
        Some(path) => env::current_dir()?.join(path),
        None => env::current_dir()?,
    };

    // Verify starting dir is valid
    if !starting_dir.exists() {
        return Err(anyhow!("Directory [{:?}] does not exist", starting_dir));
    } else if !starting_dir.is_dir() {
        return Err(anyhow!("[{:?}] is not a directory", starting_dir));
    }
    let starting_dir = std::fs::canonicalize(starting_dir)?;

    // Start .gitignore and .gitnuke stacks with empty root
    let mut gitignore_tip = ArcCactus::new();
    let mut gitnuke_tip = ArcCactus::new();

    // Add whitelist to gitnuke stack
    let ignore_whitelist = GitignoreBuilder::new(starting_dir.clone())
        .add_line(None, "!.git")?
        .add_line(None, "!.hg")?
        .add_line(None, "!.gitignore")?
        .add_line(None, "!.gitnuke")?
        // TODO: cmdline whitelist?
        .build()?;
    gitnuke_tip = gitnuke_tip.child(ignore_whitelist);

    // Add global ignore (if requested)
    let mut global_ignore = gitignore_tip.clone();
    if opt.include_global_ignore {
        let (global_gitignore, err) = GitignoreBuilder::new(starting_dir.clone()).build_global();
        if err.is_none() && global_gitignore.num_ignores() > 0 {
            gitignore_tip = gitignore_tip.child(global_gitignore);
            global_ignore = gitignore_tip.clone();
        }
    }

    // Search for ignores in parent directories
    // Stop if .git or .hg is present
    if root.is_some() {
        let mut parent_gitignore: Vec<_> = Default::default();
        let mut parent_gitnuke: Vec<_> = Default::default();
        let mut dir: &std::path::Path = &starting_dir;
        while let Some(parent_path) = dir.parent() {
            // Push `.gitignore` patterns
            if let Some(ignore) = build_ignore(&parent_path, ".gitignore") {
                parent_gitignore.push(ignore);
            }

            // Push `.gitnuke` patterns (higher priority than `.gitignore`)
            if let Some(ignore) = build_ignore(&parent_path, ".gitnuke") {
                parent_gitnuke.push(ignore);
            }

            // Stop at source control roots
            if parent_path.join(".git").exists() || parent_path.join(".hg").exists() {
                break;
            }

            // Stop at specified root
            if let Some(root) = &root {
                if root == parent_path {
                    break;
                }
            }

            dir = parent_path;
        }

        // Push parent gitignores onto gitignore_stack
        for ignore in parent_gitignore.into_iter().rev() {
            gitignore_tip = gitignore_tip.child(ignore);
        }

        // Push parent gitnukes onto gitnuke_stack
        for ignore in parent_gitnuke.into_iter().rev() {
            gitnuke_tip = gitnuke_tip.child(ignore);
        }
    }

    // Recursive job takes a path, checks if it's ignored, and recurses into subdirs if needed
    // Return value is result for the path only. Sub-directories will run separately
    // and return their own result.
    let recursive_job =
        |(mut gitignore_tip, mut gitnuke_tip, path): (ArcCactus<Gitignore>, ArcCactus<Gitignore>, PathBuf),
         worker: &Worker<_>|
         -> Option<Vec<PathBuf>> {
            let mut job_ignores: Vec<_> = Default::default();

            // Get iterator to directory children
            let read_dir = fs::read_dir(&path).ok()?;

            // Check for source control root
            if path.join(".git").exists() || path.join(".hg").exists() {
                // Reset ignore tip
                gitignore_tip = global_ignore.clone();
            }

            // Add `.gitignore` patterns
            if let Some(ignore) = build_ignore(&path, ".gitignore") {
                gitignore_tip = gitignore_tip.child(ignore);
            }

            // Add `.gitnuke` patterns
            if let Some(ignore) = build_ignore(&path, ".gitnuke") {
                gitnuke_tip = gitnuke_tip.child(ignore);
            }

            // Process each child in directory
            for child in read_dir {
                let result = || -> anyhow::Result<()> {
                    let child_path = child.with_context(|| path_context!("fs::read_dir", &path))?.path();
                    let child_meta =
                        std::fs::metadata(&child_path).with_context(|| path_context!("fs::metadata", &child_path))?;

                    // Test if child_path is ignored, whitelisted, or neither
                    // Return first match that is either ignored or whitelisted
                    let is_dir = child_meta.is_dir();
                    let ignore_match = gitnuke_tip
                        .vals()
                        .chain(gitignore_tip.vals())
                        .map(|i| i.matched(&child_path, is_dir))
                        .find(|m| !m.is_none());

                    // Handle ignored/whitelisted/neither
                    match ignore_match {
                        Some(m) => {
                            //  ignored or whitelisted
                            if opt.print_glob_matches {
                                let glob = m.inner().unwrap();
                                println!(
                                    "Glob [{:?}] from Gitignore [{:?}] matched path [{:?}]",
                                    glob.original(),
                                    glob.from(),
                                    child_path
                                );
                            }

                            // Add ignores to the list. Do nothing if whitelisted
                            if m.is_ignore() {
                                job_ignores.push(child_path);
                            } else {
                                assert!(m.is_whitelist());
                            }
                        }
                        None => {
                            // No match, recurse into directories
                            if is_dir {
                                worker.push((gitignore_tip.clone(), gitnuke_tip.clone(), child_path));
                            }
                        }
                    }

                    // Child tested
                    Ok(())
                }();

                // Print error if child could not be checked
                check_error(result);
            }

            // Return ignored paths for path
            Some(job_ignores)
        };

    // Initialize data
    println!("🔍 scanning for targets from [{:?}]", starting_dir);
    let initial_data = vec![(gitignore_tip, gitnuke_tip, starting_dir)];

    // Run recursive jobs
    let ignored_paths: Vec<(usize, PathBuf)> = job_system::run_recursive_job(initial_data, recursive_job, num_threads)
        .into_iter()
        .flatten()
        .enumerate()
        .collect();

    // Second recursive job to compute size of ignored directories
    let recursive_dir_size_job = |(root_idx, path): (usize, PathBuf), worker: &Worker<_>| -> Option<(usize, u64)> {
        // Get type of path
        let path_meta = fs::metadata(&path).ok()?;

        // If file, return result immediately
        if path_meta.is_file() {
            return Some((root_idx, path_meta.len()));
        }

        // Get director iterator
        let read_dir = fs::read_dir(&path).ok()?;

        // Iterate children
        let mut files_size = 0;
        for child in read_dir {
            let result = || -> anyhow::Result<()> {
                // Ignore errors
                let child_path = child.with_context(|| path_context!("read_dir", &path))?.path();
                let child_meta =
                    fs::metadata(&child_path).with_context(|| path_context!("fs::metadata", &child_path))?;

                // Accumualte file size
                // Add directories to the worker
                if child_meta.is_file() {
                    files_size += child_meta.len();
                } else {
                    worker.push((root_idx, child_path));
                }

                Ok(())
            }();
            check_error(result);
        }

        Some((root_idx, files_size))
    };

    // Compute path sizes
    let dir_sizes = job_system::run_recursive_job(ignored_paths.clone(), recursive_dir_size_job, num_threads);

    // Sum sizes
    let mut ignore_path_sizes: Vec<u64> = Default::default();
    ignore_path_sizes.resize(ignored_paths.len(), 0);
    for (idx, size) in dir_sizes {
        ignore_path_sizes[idx] += size;
    }

    // Sort ignored paths by size
    let final_ignore_paths: Vec<_> = ignored_paths
        .into_iter()
        .zip(ignore_path_sizes)
        .map(|((_, path), size)| (path, size))
        .filter(|(_, size)| *size >= opt.min_file_size)
        .sorted_by_key(|kvp| kvp.1)
        .collect();

    // No ignores found
    if final_ignore_paths.is_empty() {
        println!("No ignore paths to delete.");
        return Ok(());
    }

    // Print ignores
    let mut total_bytes = 0;
    for (path, bytes) in &final_ignore_paths {
        total_bytes += bytes;
        if !opt.benchmark {
            println!("  {:10} {:?}", pretty_bytes(*bytes), path);
        }
    }
    println!("Total Bytes: {}", total_bytes.to_formatted_string(&Locale::en));
    println!("Time: {:?}", start.elapsed());

    // Skip NUKE op in benchmark mode
    if opt.benchmark {
        return Ok(());
    }

    // Verify nuke
    const NUKE_STRING: &str = "NUKE";
    const QUIT_STRING: &str = "QUIT";

    // Helper to remove either a file or a directory
    let remove_path = |path: &std::path::Path| {
        // Try to remove path
        let result = || -> anyhow::Result<()> {
            let meta = fs::metadata(&path).with_context(|| format!("{} {}", "fs::metadata", path.display()))?;

            // Remove file or directory
            if meta.is_file() {
                std::fs::remove_file(&path).with_context(|| format!("{} {}", "fs::remove_file", path.display()))?;
            } else {
                std::fs::remove_dir_all(&path)
                    .with_context(|| format!("{} {}", "fs::remove_dir_all", path.display()))?;
            }

            Ok(())
        }();

        // Always print removal errors
        match result {
            Ok(_) => (),
            Err(e) => println!("Error: {}", e),
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
            for (path, _) in final_ignore_paths {
                remove_path(&path);
            }

            println!("☠️☠️☠️ nuclear deletion complete ☠️☠️☠️");
            break;
        } else if trimmed_input.eq_ignore_ascii_case(QUIT_STRING) {
            println!("😇😇😇 Nuclear launch aborted. Thank you and have a nice day. 😇😇😇");
            break;
        } else {
            println!(
                "Invalid input. Input was [{}] but must exactly match [{}] to irrevocably nuke. Please try again.",
                trimmed_input, NUKE_STRING
            );
        }
    }

    // Mission accomplished
    Ok(())
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
        _ => format!("{}", orig_amount),
    }
}
