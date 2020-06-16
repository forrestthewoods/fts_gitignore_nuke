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


# How to prevent critical files from being deleted ?

After loading `.gitignore`, `fts_gitignore_nuke` will also look for `.gitnuke`. Any pattern included or excluded from `.gitnuke` has higher precedence than whatever is in `.gitignore`.

Additionally, `fts_gitignore_nuke` will NEVER delete any of the following:
* `.git`
* `.hg`
* `.gitignore`
* `.gitnuke`

You may safely add any of these patterns to your `.gitignore` (or even `.gitnuke`), the corresponding items will not be removed.

As an example, the three following patterns are absolutely equivalent, and will result in the same files being deleted regardless of the layout of the directory tree.

```
.gitignore
    foo/*
    bar
    baz/quux
```
```
.gitignore
    foo/*

.gitnuke
    bar
    baz/quux
    .gitignore
```
```
.gitignore
    foo/*
    bar
    baz/quux
    spam
    eggs/*
    .gitnuke

.gitnuke
    !spam
    !eggs/*
```

Despite this mechanism, you should still carefully review the list of files to be deleted before nuking them.
