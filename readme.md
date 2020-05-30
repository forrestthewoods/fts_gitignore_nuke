# fts_gitignore_nuke

fts_gitignore_nuke is a Rust-written CLI tool to find files and folders hidden by .gitignore files so they can be deleted.

This is useful because it allows deleting build output from many projects in one action. All operations are performed manually and `git` is never invoked. This is because `.gitignore` files are increasingly used in contexts outside Git. For example Mercurial, Perforce or custom tooling may leverage `.gitignore` files.

![](/screenshots/nuclear_launch.png?raw=true)

# Installation

`fts_gitignore_nuke` can currently be run by cloning this repo and `cargo build --release`. Cargo installation coming soon. Pre-built binaries may be provided if there is sufficient interest.

# Usage
Compile `fts_gitignore_nuke` and run from or on any directory. No files will be deleted without explicit user inputs.

Default behavior starts from the current directory and tests all children. `.gitignore` files are stacked and evaluated in LIFO order.

Default behavior does NOT include parent or `.gitignore` files. Both can be included with `--include_parent_ignores` and `--include_global_ignore` respectively.

```
USAGE:
    fts_gitignore_nuke.exe [FLAGS] [OPTIONS]

FLAGS:
    -b, --bench                     Run benchmark. Auto-quit after walking directory
    -h, --help                      Prints help information
        --include_global_ignore     Include global .gitignore for matches
        --include_parent_ignores    Include .gitignore files from parent directories
        --print_errors              Prints errors if encountered
        --print_glob_matches        Prints which glob and which .gitignore matched each path
    -V, --version                   Prints version information

OPTIONS:
    -d, --directory <DIRECTORY>            Root directory to start search
    -m, --min_file_size <MIN_FILE_SIZE>    Minimum size, in bytes, to nuke [default: 0]
    -t, --num_threads <NUM_THREADS>        Number of threads to use
```

# Support

`fts_gitignore_nuke` was built for Windows. It should work on other platforms, but has not been tested.

# Performance

`fts_gitignore_nuke` is relatively fast and multithreaded by default. Disk IO is the clear bottleneck.
