use anyhow::anyhow;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{env, fs};
use std::sync::Arc;
/*
struct IgnoreIter<'a> {
    _gitignore : std::rc::Rc<Gitignore>,
    _returned : bool
}

impl<'a> Iterator for IgnoreIter<'a> {
    type Item = &'a Gitignore;

    fn next(&mut self) -> Option<Self::Item> {
        if self.returned {
            None
        } else {
            *self._gitignore
        }
    }
}
*/

struct IgnoresList {
    _gitignore : std::rc::Rc<Gitignore>,
    _next : Option<Arc<IgnoresList>>
}

fn main() -> anyhow::Result<()> {
    let current_dir = env::current_dir()?;
    println!("Starting Dir: [{:?}]", current_dir);
    let current_dir : std::path::PathBuf = "c:/source_control".into();

    let mut gitignores : Vec<Gitignore> = Default::default();
    type IgnoreIter = Box<dyn Iterator<Item=Arc<Gitignore>>>;

    //type IgnoreIter = dyn Iterator<Item=Arc<Gitignore>>;
    //let mut ignores_iter : IgnoreIter = std::iter::empty::<Arc<Gitignore>>();
    let mut ignores_iter = std::iter::empty::<Arc<Gitignore>>();

    let (global_ignore, err) = GitignoreBuilder::new(current_dir.clone()).build_global();
    if err.is_none() && global_ignore.num_ignores() > 0 {
        let arc_global_ignore = Arc::new(global_ignore);
        let global_ignore_iter = std::iter::from_fn(move || Some(arc_global_ignore.clone()));
        let ignores_iter = ignores_iter.chain(global_ignore_iter);
        //gitignores.push(global_ignore);
    }

    let mut ignored : Vec<std::path::PathBuf> = Default::default();

    let mut queue : std::collections::VecDeque<(_, std::path::PathBuf)> = Default::default();
    queue.push_back((gitignores.iter(), current_dir));

    while !queue.is_empty() {
        let (ignore_iter, entry) = queue.pop_front().unwrap();
        assert!(std::fs::metadata(&entry)?.is_dir());

        //println!("Dir: {:?}", entry);

        let mut dirs : Vec<std::path::PathBuf> = Default::default();

        for child in fs::read_dir(entry)? {
            let child_path = child?.path();
            let child_meta = std::fs::metadata(&child_path)?;

            if child_meta.is_file() {
                //println!("File: {:?}", child_path);

                

                if child_path.file_name().unwrap() == ".gitignore" {
                    //println!("!!! IGNORE: {:?}", child_path);
                    let parent_path = child_path.parent().ok_or(anyhow!("Failed to get parent for [{:?}]", child_path))?;
                    let mut ignore_builder = ignore::gitignore::GitignoreBuilder::new(parent_path);
                    ignore_builder.add(child_path);
                    //gitignores.push(ignore_builder.build()?);
                } else {
                    let is_ignored = gitignores.iter()
                        .map(|i| i.matched(&child_path, true))
                        .any(|m| {
                            if m.is_ignore() {
                                println!("{:?}", m.inner().unwrap());
                                true
                            } else {
                                false
                            }
                        });

                    if is_ignored {
                        ignored.push(child_path);
                    }
                }
            } else {
                dirs.push(child_path);
            }
        }

        for dir in dirs.into_iter() {

            let is_ignored = gitignores.iter()
                .map(|i| i.matched(&dir, true))
                .any(|m| m.is_ignore());

            //println!("Ignored?: [{}] [{:?}]", is_ignored, dir);

            if is_ignored {
                ignored.push(dir);
            } else {
                queue.push_back((ignore_iter.clone(), dir));
            }
        }
    }

    println!("Ignored");
    for path in ignored.into_iter() {
        println!("  {:?}", path);
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