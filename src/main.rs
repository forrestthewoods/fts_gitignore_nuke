use anyhow::anyhow;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use itertools::Itertools;
use std::{env, fs};

mod immutable_stack;
use immutable_stack::ImmutableStack;

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
                result += meta.len();
            } else {
                dirs.push(child_path);
            }
        }
    }

    Ok(result)
}

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

fn main() -> anyhow::Result<()> {
    let current_dir = env::current_dir()?;
    println!("Starting Dir: [{:?}]", current_dir);
    let current_dir : std::path::PathBuf = "c:/source_control".into();

    // Root ignore (empty)
    let ignore_stack = ImmutableStack::new();
    let mut ignore_tip = ignore_stack.clone();
    
    // Add global ignore (if exists)
    let (global_ignore, err) = GitignoreBuilder::new(current_dir.clone()).build_global();
    if err.is_none() && global_ignore.num_ignores() > 0 {
        ignore_tip = ignore_stack.push(global_ignore).clone();
    }

    // List of all ignored paths
    let mut ignored_paths : Vec<std::path::PathBuf> = Default::default();

    // Queue of paths to consider
    let mut queue : std::collections::VecDeque<(ImmutableStack<Gitignore>, std::path::PathBuf)> = Default::default();
    queue.push_back((ignore_tip, current_dir));


    // Process all paths
    while !queue.is_empty() {
        let (mut ignore_tip, entry) = queue.pop_front().unwrap();
        assert!(std::fs::metadata(&entry)?.is_dir());

        // Dirs must be deferred so they can consider potential ignore files
        let mut dirs : Vec<std::path::PathBuf> = Default::default();

        // Process each child in directory
        for child in fs::read_dir(entry)? {
            let child_path = child?.path();
            let child_meta = std::fs::metadata(&child_path)?;

            // Child is file
            if child_meta.is_file() {
                // Child is ignorefile. Push it onto ignore stack
                if child_path.file_name().unwrap() == ".gitignore" {
                    // Parse new .gitignore
                    let parent_path = child_path.parent().ok_or(anyhow!("Failed to get parent for [{:?}]", child_path))?;
                    let mut ignore_builder = ignore::gitignore::GitignoreBuilder::new(parent_path);
                    ignore_builder.add(child_path);
                    let new_ignore = ignore_builder.build()?;
                    ignore_tip = ignore_tip.push(new_ignore);
                } else {
                    // Child is file. Perform ignore test.
                    let is_ignored = ignore_tip.iter()
                        .map(|i| i.matched(&child_path, true))
                        .any(|m| m.is_ignore());

                    if is_ignored {
                        ignored_paths.push(child_path);
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
                ignored_paths.push(dir);
            } else {
                queue.push_back((ignore_tip.clone(), dir));
            }
        }
    }

    // Print all ignored content
    println!("Ignored:");
    //for path in ignored_paths
    //    .into_iter() 
    ignored_paths.into_iter()
        .map(|dir| (dir_size(dir.clone()), dir))
        .filter_map(|(bytes, dir)| if let Ok(bytes) = bytes { Some((bytes, dir)) } else { None })
        .sorted_by_key(|kvp| kvp.0)
        .for_each(|(bytes, path)| {
            println!("  {:10} {:?}", pretty_bytes(bytes), path);
        });

    Ok(())
}