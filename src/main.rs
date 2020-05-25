use std::{env, fs};
use anyhow::anyhow;

fn main() -> anyhow::Result<()> {
    let current_dir = env::current_dir()?;
    println!("Starting Dir: [{:?}]", current_dir);

    let mut queue : std::collections::VecDeque<std::path::PathBuf> = Default::default();
    queue.push_back(current_dir.clone());

    //let mut dirs : Vec<std::path::PathBuf> = Default::default();
    let mut ignores : Vec<ignore::gitignore::Gitignore> = Default::default();

    while !queue.is_empty() {
        let entry = queue.pop_front().unwrap();
        assert!(std::fs::metadata(&entry)?.is_dir());

        println!("Dir: {:?}", entry);

        let mut dirs : Vec<std::path::PathBuf> = Default::default();

        for child in fs::read_dir(entry)? {
            let child_path = child?.path();
            let child_meta = std::fs::metadata(&child_path)?;

            if child_meta.is_file() {
                println!("File: {:?}", child_path);

                if child_path.file_name().unwrap() == ".gitignore" {
                    println!("!!! IGNORE: {:?}", child_path);
                    let parent_path = child_path.parent().ok_or(anyhow!("Failed to get parent for [{:?}]", child_path))?;
                    let mut ignore_builder = ignore::gitignore::GitignoreBuilder::new(parent_path);
                    ignore_builder.add(child_path);
                    ignores.push(ignore_builder.build()?);

                    // TODO: Make gitignore builder
                }
            } else {
                dirs.push(child_path);
            }
        }

        for dir in dirs.into_iter() {

            let ignored = ignores.iter()
                .map(|i| i.matched(&dir, true))
                .any(|m| m.is_ignore());

            println!("Ignored?: [{}] [{:?}]", ignored, dir);

            
            if !ignored {
                //queue.push_back(dir);
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