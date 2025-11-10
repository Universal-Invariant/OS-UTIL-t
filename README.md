# t - A Tree Command with regex support

A tree command-line tool written in Rust.

## Usage

```
Usage: t [OPTIONS] [PATH]

Arguments:
  [PATH]  Directory to traverse [default: .]

Options:
  -L, --depth <DEPTH>                Maximum depth to traverse [default: 100]
  -H, --no-hidden                    Do not show hidden files and directories (those starting with '.')
  -a, --all                          Intersect all matches
      --no-color                     Don't use colors in the output
  -S, --summary                      Display a summary at end
  -f, --file-regex <PATTERN>         Regular expression to filter file names (default: ".*") [default: ]
  -d, --dir-regex <PATTERN>          Regular expression to filter directory names (default: ".*") [default: ]
  -F, --file-regex-c <PATTERN>       The case sensitive version of f and d [default: ]
  -D, --dir-regex-c <PATTERN>        Regular expression to filter directory names (default: ".*") [default: ]
  -m, --meta-search <FIELD:PATTERN>  Regular expression to filter by metadata (format: "field:pattern", e.g., "size:>1024", "modified:.*2023.*")
      --prune-dirs                   Prune directory traversal: skip directories whose names don't match
  -i, --flat                         Print full paths instead of the tree format
  -p, --print-format [<FORMAT>]      Format string for file output(or use TREEE_FORMAT_FILE env) (e.g., "", "size=%size%, creation=%creation%")
  -P, --print-format [<FORMAT>]      Format string for dir output(or use TREEE_FORMAT_DIR env) (e.g., "", "size=%size%, creation=%creation%")
  -h, --help                         Print help
  -V, --version                      Print version

Usable %token%s: path, full_path, immediate_files_size, total_size, total_files, total_dirs,
        p_immediate_files_size, p_total_size, p_total_files, p_total_dirs,
        sub_dirs_count, sub_files_count, depth, modified, created, accessed, is_dir, readonly
		

f,d, and m can be used multiple times each getting it's own color
```

<img width="345" height="241" alt="Screenshot 2025-11-09 182924" src="https://github.com/user-attachments/assets/e308979b-47e0-4793-bd92-dbc8d800a9b3" />
