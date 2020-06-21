# fts_gitignore_nuke

fts_gitignore_nuke is a Rust-written CLI tool to find files and folders hidden by .gitignore files so they can be deleted.

This is useful because it allows deleting build output from many projects in one action. All operations are performed manually and `git` is never invoked. This is because `.gitignore` files are increasingly used in contexts outside Git. For example Mercurial, perforce or custom tooling may leverage `.gitignore` files.

![](/screenshots/nuclear_launch.png?raw=true)

# Installation

`fts_gitignore_nuke` can currently be installed via `cargo install fts_gitignore_nuke`.

# Usage
Compile `fts_gitignore_nuke` and run from or on any directory. No files will be deleted without explicit user inputs.

Default behavior starts from the current directory and tests all children. `.gitignore` files are stacked and evaluated in LIFO order.

Default behavior does NOT include parent or `.gitignore` files. Both can be included with `--include_parent_ignores` and `--include_global_ignore` respectively.

```
Deletes files hidden by .gitignore files

USAGE:
    fts_gitignore_nuke.exe [FLAGS] [OPTIONS]

FLAGS:
    -b, --benchmark                Auto-quit after walking directory
    -h, --help                     Prints help information
        --include-global-ignore    Include global .gitignore for matches
        --print-errors             Prints errors if encountered
        --print-glob-matches       Prints which glob and which .gitignore matched each path
    -V, --version                  Prints version information

OPTIONS:
    -d, --directory <directory>            Root directory to start search
        --min-file-size <min-file-size>    Minimum size, in bytes, to nuke [default: 0]
        --num-threads <num-threads>        Number of threads to use. Default: num physical cores
    -r, --root <root>                      Include .gitignores between root and target directory
```

# Support

`fts_gitignore_nuke` was built for Windows. It has also been tested on Ubuntu, and should support other platforms. This tool was written for personal use cases and may require slight modification to support different environments or workflows. Pull requests welcome!

# Performance

`fts_gitignore_nuke` is relatively fast and multithreaded by default. Disk IO is the clear bottleneck.


# How to prevent critical files from being deleted?

In addition to `.gitignore` files, `fts_gitignore_nuke` will also look for `.gitnuke` files. A `.gitnuke` file is loaded exactly as a regular `.gitignore` behavior. Expected user behavior is for `.gitnuke` files to contain whitelist patterns (example: `!foo.key`) for files and folders that are not part of a Git repo but should not be nuked. Examples of such content are private keys, local content, or expensive build artifacts.

When matching a path `fts_gitignore_nuke` will run through all hierarchical `.gitnuke` files and then all `.gitignore` files. This means that every `.gitnuke` file has higher precedence than every `.gitignore` file.

As always, please carefully review the list of files to be deleted before nuking them.
