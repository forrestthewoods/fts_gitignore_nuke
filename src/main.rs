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

/*
struct IgnoreChainNode {
    gitignore : Arc<Gitignore>,
    prev : Option<Arc<IgnoreChainNode>>
}

impl IgnoreChainNode {
    fn new(gitignore: Arc<Gitignore>) -> Self {
        Self { 
            gitignore, 
            prev: None 
        }
    }
}
*/


#[derive(Clone)]
struct IgnoreChainIter {
    inner: Option<(Arc<Gitignore>, Arc<IgnoreChainIter>)>
}

impl IgnoreChainIter {
    fn empty() -> Arc<Self> {
        Arc::new(
            Self { 
                inner : None
            }
        )
    }

    fn chain(ignore: Arc<Gitignore>, prev: Arc<IgnoreChainIter>) -> Arc<IgnoreChainIter> {
        Arc::new(
            Self {
                inner: Some((ignore, prev))
            }
        )
    }
}

impl Iterator for IgnoreChainIter {
    type Item = Arc<Gitignore>;

    fn next(&mut self) -> Option<Arc<Gitignore>> {
        match self.inner.clone() {
            Some((ignore, prev)) => {
                self.inner = prev.inner.clone();
                Some(ignore.clone())
            },
            None => None,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let current_dir = env::current_dir()?;
    println!("Starting Dir: [{:?}]", current_dir);
    let current_dir : std::path::PathBuf = "c:/source_control".into();
    
    // Root ignore (empty)
    let mut ignore_chain = IgnoreChainIter::empty();

    // Add global ignore (if exists)
    let (global_ignore, err) = GitignoreBuilder::new(current_dir.clone()).build_global();
    if err.is_none() && global_ignore.num_ignores() > 0 {
        let arc_global_ignore = Arc::new(global_ignore);
        ignore_chain = IgnoreChainIter::chain(arc_global_ignore, ignore_chain);
    }

    // List of all ignored paths
    let mut ignored : Vec<std::path::PathBuf> = Default::default();

    // Queue of paths to consider
    let mut queue : std::collections::VecDeque<(Arc<IgnoreChainIter>, std::path::PathBuf)> = Default::default();
    queue.push_back((ignore_chain, current_dir));

    // Process all paths
    while !queue.is_empty() {
        let (ignore_chain, entry) = queue.pop_front().unwrap();
        assert!(std::fs::metadata(&entry)?.is_dir());

        //println!("Dir: {:?}", entry);

        let mut dirs : Vec<std::path::PathBuf> = Default::default();
        let mut new_ignore_chain = ignore_chain.clone();

        for child in fs::read_dir(entry)? {
            let child_path = child?.path();
            let child_meta = std::fs::metadata(&child_path)?;

            if child_meta.is_file() {
                if child_path.file_name().unwrap() == ".gitignore" {
                    let parent_path = child_path.parent().ok_or(anyhow!("Failed to get parent for [{:?}]", child_path))?;
                    let mut ignore_builder = ignore::gitignore::GitignoreBuilder::new(parent_path);
                    ignore_builder.add(child_path);
                    let new_ignore = Arc::new(ignore_builder.build()?);
                    new_ignore_chain = IgnoreChainIter::chain(new_ignore, ignore_chain.clone());
                } else {
                    let is_ignored = ignore_chain.clone()
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

            let is_ignored = new_ignore_chain.clone()
                .map(|i| i.matched(&dir, true))
                .any(|m| m.is_ignore());


            if is_ignored {
                ignored.push(dir);
            } else {
                queue.push_back((new_ignore_chain.clone(), dir));
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