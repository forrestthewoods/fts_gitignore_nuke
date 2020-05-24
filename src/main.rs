use std::{env, fs};
//use anyhow::anyhow;

fn main() -> anyhow::Result<()> {
    let current_dir = env::current_dir()?;
    println!(
        "Entries modified in the last 24 hours in {:?}:",
        current_dir
    );

    let mut queue : std::collections::VecDeque<std::path::PathBuf> = Default::default();
    queue.push_front(current_dir.clone());

    while !queue.is_empty() {
        let entry = queue.pop_back().unwrap();
        assert!(std::fs::metadata(&entry)?.is_dir());

        println!("{:?}", entry);

        for child in fs::read_dir(entry)? {
            let child_path = child?.path();
            let child_meta = std::fs::metadata(&child_path)?;

            if child_meta.is_file() {
                println!("{:?}", child_path);
            } else {
                queue.push_front(child_path);
            }
        }
    }

        /*
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        println!("{:?}", path);

        let metadata = fs::metadata(&path)?;
        let last_modified = metadata.modified()?.elapsed()?.as_secs();

        if last_modified < 24 * 3600 && metadata.is_file() {
            //let filename = path.file_name().ok_or(anyhow!("uh oh"))?;
            println!(
                "Last modified: {:?} seconds, is read only: {:?}, size: {:?} bytes, filename: {:?}",
                last_modified,
                metadata.permissions().readonly(),
                metadata.len(),
                path.file_name().ok_or(anyhow!("No filename"))?
            );
        }
    }
        */

    Ok(())
}