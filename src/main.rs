#![allow(unused)]
#![debugger_visualizer(natvis_file = "treee.natvis")]
//#![debugger_visualizer(natvis_file = "../intrinsic.natvis")]
mod parent_ref;

#[macro_use]
mod extend;

use parent_ref::ParentRef;


/*
	TODO: Currently -p <str> doesn't differentiate between dirs and files. this can make it hard to get appropriate strings. So we should somehow specify a way to do dir and/or file string
	Add some way to do an anti-match which will prevent such matches
	TODO: There seems to be a bug when specifying only -d, we stil get files
 */

use anyhow::Result;
use clap::Parser;
use colored::*;
use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;
use std::rc::{Rc};
use std::cell::RefCell;
use std::time::SystemTime;
use chrono::offset::Utc; use chrono::DateTime;use chrono::SecondsFormat;

use winapi::um::fileapi::{GetCompressedFileSizeW, INVALID_FILE_SIZE};
use std::os::windows::ffi::OsStrExt;


// https://doc.rust-lang.org/std/fs/struct.Metadata.html
// https://docs.rs/winapi/latest/winapi/

// L:\gh\OS\UTIL\treee\target\debug\treee -S -f ".*json$"
// L:\gh\OS\UTIL\treee\target\debug\treee -S -f ".*tmp$" -f ".*data.*" -f ".*Notify.*" -d ".*chrome.*"
// L:\gh\OS\UTIL\treee\target\debug\treee -S -f ".*tmp$" -f ".*data.*" -f ".*Notify.*" -m "size:>:190000"

// TODO: debug, add more of the metadata searches, add summary, format string(with using env if exists), and summary, etc


macro_rules! to_dt {
	($dt:expr) => {{
		Into::<DateTime<Utc>>::into($dt.unwrap()).to_rfc3339_opts(SecondsFormat::Secs, true)
	}};
}



macro_rules! updm {
	($rc:expr, $update:expr) => {{
        let value = $update;
        $rc.$field += value;
    }};

	($target:expr, += $source:expr) => {{
		let v = $source;
        $target += v;
    }};

    ($target:expr, $target_field:ident = $source:expr, $source_field:ident) => {{
        $target.$target_field = $source.borrow().$source_field;
    }};

}

macro_rules! upd {
	($rc:expr, $field:ident, $update:expr) => {{
        let value = $update;
        $rc.borrow_mut().$field += value;
    }};

	($target:expr, $target_field:ident += $source:expr, $source_field:ident) => {{
		let v = $source.borrow().$source_field;
        $target.borrow_mut().$target_field += v;
    }};

    ($target:expr, $target_field:ident = $source:expr, $source_field:ident) => {{
		let v = $source.borrow().$source_field
        $target.borrow_mut().$target_field = v;
    }};

}


// Directory colors - warm/bright colors
const DIR_COLORS: &[fn(&str) -> ColoredString] = &[
    |s| s.blue(),
    |s| s.bright_green(),
    |s| s.green(),
    |s| s.bright_cyan(),
	|s| s.cyan(),
    |s| s.truecolor(70, 130, 180),     // Steel blue
    |s| s.truecolor(46, 139, 87),      // Sea green
];

// File colors - cool/subtle colors
const FILE_COLORS: &[fn(&str) -> ColoredString] = &[
    |s| s.yellow(),
    |s| s.magenta(),
    |s| s.truecolor(255, 165, 0),      // Orange
    |s| s.truecolor(255, 105, 180),    // Hot pink
    |s| s.truecolor(128, 0, 128),      // Purple
    |s| s.truecolor(0, 128, 128),      // Teal
    |s| s.truecolor(139, 69, 19),      // Brown
    |s| s.truecolor(210, 105, 30),     // Chocolate
];

// Alternative: Combine all matching colors (for multiple matches)
fn get_combined_color(
    name: &str,
    match_details: &[bool],
    is_directory: bool
) -> ColoredString {
    let color_palettes = if is_directory { DIR_COLORS } else { FILE_COLORS };

    // Find all matching indices
    let matching_indices: Vec<usize> = match_details
        .iter()
        .enumerate()
        .filter_map(|(i, &m)| if m { Some(i) } else { None })
        .collect();

    if matching_indices.is_empty() {
        return if is_directory { name.bright_blue() } else { name.red() };
    }

    // For multiple matches, use the first one (or you could blend colors, but that's complex in terminal)
    let color_idx = matching_indices[0] % color_palettes.len();
    (color_palettes[color_idx])(name)
}




#[derive(Debug)]
struct MetaSearch {
    field: String,
    pattern: String,
    operator: MetaOperator,
}

#[derive(Debug, PartialEq)]
enum MetaOperator {
    Equals,
    GreaterThan,
    LessThan,
    Contains,
    Regex,
}

// Parse metadata search strings
fn parse_meta_search(search: &str) -> Result<MetaSearch, String> {
    // Format: "field:operator:pattern" or "field:pattern" (default to contains)
    let parts: Vec<&str> = search.splitn(3, ':').collect();

    match parts.len() {
        2 => Ok(MetaSearch {
            field: parts[0].to_string(),
            operator: MetaOperator::Contains, // default
            pattern: parts[1].to_string(),
        }),
        3 => {
            let operator = match parts[1] {
                ">" => MetaOperator::GreaterThan,
                "<" => MetaOperator::LessThan,
                "=" => MetaOperator::Equals,
                "~" => MetaOperator::Contains,
                "^>" => MetaOperator::GreaterThan,
                "^<" => MetaOperator::LessThan,
                "^=" => MetaOperator::Equals,
                "^~" => MetaOperator::Contains,
                _ => MetaOperator::Regex,
            };
			// We have to combine parts 1 and 2 when a colon exists in the regex
			let res = if operator == MetaOperator::Regex { search[parts[0].len()+1..].to_string() } else { parts[2].to_string() };
            Ok(MetaSearch {
                field: parts[0].to_string(),
                operator,
                pattern: res,
            })
        }
        _ => Err("Invalid meta search format".to_string()),
    }
}

// Metadata matching function
fn matches_metadata(meta: &fs::Metadata, search: &MetaSearch) -> bool {
    match search.field.as_str() {
        "size" => {
            let size = meta.len();
            match search.operator {
                MetaOperator::Equals => size.to_string().contains(&search.pattern),
                MetaOperator::GreaterThan => {
                    search.pattern.parse::<u64>().map_or(false, |min_size| size >= min_size)
                }
                MetaOperator::LessThan => {
                    search.pattern.parse::<u64>().map_or(false, |max_size| size <= max_size)
                }
                MetaOperator::Contains | MetaOperator::Regex => {
                    let regex = Regex::new(&search.pattern).unwrap_or_else(|_| Regex::new(".*").unwrap());
                    regex.is_match(&size.to_string())
                }
            }
        }
		// Search on "metadata"
        "modified" | "created" | "accessed"| "readonly" => {
            let res = match search.field.as_str() {
                "modified" => to_dt!(meta.modified()),
                "created" => to_dt!(meta.created()),
                "accessed" => to_dt!(meta.accessed()),
				"readonly" => meta.permissions().readonly().to_string(),
                _ => return false,
            };
            match search.operator {
				MetaOperator::Contains | MetaOperator::Regex => {
					let regex = Regex::new(&search.pattern).unwrap_or_else(|_| Regex::new(".*").unwrap());
					regex.is_match(&res)
				},
				MetaOperator::Equals => {
					res == search.pattern
				},
				_ => res.contains(&search.pattern),
			}
        }
        _ => false, // Unknown field
    }
}


use std::collections::HashMap;

// Format a string by replacing placeholders with actual values
fn format_string(format_str: &str, values: &HashMap<&str, String>, is_dir: bool) -> String {
    let mut result = format_str.to_string();

    for (placeholder, value) in values {
        let placeholder_key = format!("%{}%", placeholder);
        result = result.replace(&placeholder_key, value);
    }

    result
}

// Get available format values for a file
fn get_file_format_values(file: &FileInfo, metadata: &fs::Metadata) -> HashMap<&'static str, String> {
    let mut values = HashMap::new();

    values.insert("name", file.name.clone());
    values.insert("size", file.size.to_string());
    values.insert("path", file.path.to_string_lossy().to_string());

    // Add metadata values
    if let Ok(modified) = metadata.modified() {
        values.insert("modified", to_dt!(Some(modified)));
    }
    if let Ok(created) = metadata.created() {
        values.insert("created", to_dt!(Some(created)));
    }
    if let Ok(accessed) = metadata.accessed() {
        values.insert("accessed", to_dt!(Some(accessed)));
    }

    values.insert("is_file", "true".to_string());
    values.insert("is_dir", "false".to_string());
    values.insert("readonly", metadata.permissions().readonly().to_string());

    values
}

// Get available format values for a directory
fn get_dir_format_values(dir: &DirInfo, metadata: &fs::Metadata) -> HashMap<&'static str, String> {
    let mut values = HashMap::new();

    values.insert("name", dir.name.clone());
	values.insert("size", dir.total_size.to_string());
    values.insert("path", dir.path.to_string_lossy().to_string());

    // Add directory statistics
    values.insert("immediate_files_size", dir.immediate_files_size.to_string());
	values.insert("total_size", dir.total_size.to_string());
    values.insert("total_files", dir.total_files.to_string());
    values.insert("total_dirs", dir.total_dirs.to_string());
    values.insert("p_immediate_files_size", dir.p_immediate_files_size.to_string());
    values.insert("p_total_size", dir.p_total_size.to_string());
    values.insert("p_total_files", dir.p_total_files.to_string());
    values.insert("p_total_dirs", dir.p_total_dirs.to_string());
    values.insert("sub_dirs_count", dir.sub_dirs.len().to_string());
    values.insert("sub_files_count", dir.sub_files.len().to_string());
    values.insert("depth", dir.depth.to_string());

    // Add metadata values
    if let Ok(modified) = metadata.modified() {
        values.insert("modified", to_dt!(Some(modified)));
    }
    if let Ok(created) = metadata.created() {
        values.insert("created", to_dt!(Some(created)));
    }
    if let Ok(accessed) = metadata.accessed() {
        values.insert("accessed", to_dt!(Some(accessed)));
    }

    values.insert("is_file", "false".to_string());
    values.insert("is_dir", "true".to_string());
    values.insert("readonly", metadata.permissions().readonly().to_string());

    values
}



// Add default format strings as constants
const DEFAULT_FILE_FORMAT: &str = " (size = %size%, created %created%, accessed %accessed%, modified %modified%)";
const DEFAULT_DIR_FORMAT: &str = " (size = %p_total_size%/%total_size%, dirs = %sub_dirs_count%/%p_total_dirs%, files = %sub_files_count%/%p_total_files%)";

// Helper to get the appropriate default format
fn get_format_string(args: &Args, is_dir: bool) -> String {
	let e = match std::env::var(if is_dir { "TREEE_FORMAT_DIR" } else { "TREEE_FORMAT_FILE"}) { Ok(v) => { v }, Err(v) => {"".to_string()} };

	let e = e.replace("^%", "%");

	if is_dir {
	    match &args.print_formatP {
			Some(Some(format_str)) => format_str.clone(), // -p "custom" used
			Some(None) => if is_dir {DEFAULT_DIR_FORMAT.to_string()} else {DEFAULT_FILE_FORMAT.to_string()},
			None => e
		}
	}
	else {
   		match &args.print_formatp {
			Some(Some(format_str)) => format_str.clone(), // -p "custom" used
			Some(None) => if is_dir {DEFAULT_DIR_FORMAT.to_string()} else {DEFAULT_FILE_FORMAT.to_string()},
			None => e
		}
	}
}


#[derive(Debug)]
struct FileInfo {
	name: String,
	path: PathBuf,
    size: u64,
	regex_matched: bool,
	parent: ParentRef<DirInfo>,
}

#[derive(Debug)]
struct DirInfo {
	path: PathBuf,
	name: String,
    // Size of immediate files only (not including subdirectories)
    immediate_files_size: u64,

    total_size: u64,	// Total size including all subdirectories
	total_files: u64,	// Total files
	total_dirs: u64,	// Total dirs


	// statistics after parsing	of matched files and dirs
	p_immediate_files_size: u64,	// sum of immediate matched file sizes
	p_total_size: u64,				// sum of all matched file sizes
	p_total_files: u64,				// total number of matched files found
	p_total_dirs: u64,				// total number of matched dirs


	regex_matched: bool,
	contains_dir_matching_regex: bool,
	contains_file_matching_regex: bool,
	contains_meta_matching_regex: bool,
	depth: usize,
	parent: ParentRef<DirInfo>,
	sub_dirs: Vec<Rc<RefCell<DirInfo>>>,
	sub_files: Vec<Rc<RefCell<FileInfo>>>,
}


#[derive(Parser, Clone)]
#[command(name = "tree")]
#[command(about = "A tree command with regex filtering")]
#[command(version = "1.0.0")]
#[command(after_help = "Usable %token%s: path, full_path, immediate_files_size, total_size, total_files, total_dirs, \n\tp_immediate_files_size, p_total_size, p_total_files, p_total_dirs, \n\tsub_dirs_count, sub_files_count, depth, modified, created, accessed, is_dir, readonly")]
struct Args {
    /// Directory to traverse
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Maximum depth to traverse
    #[arg(short = 'L', long, default_value_t = 100)]
    depth: usize,

    /// Do not show hidden files and directories (those starting with '.')
    #[arg(short = 'H', long)]
    no_hidden: bool,

	/// Intersect all matches
	#[arg(short = 'a', long = "all", default_value_t = false)]
    all: bool,

    /// Don't use colors in the output
    #[arg(long)]
    no_color: bool,

	/// Display a summary at end
    #[arg(short = 'S', long = "summary", default_value_t = false)]
    summary: bool,

    /// Regular expression to filter file names (default: ".*")
    #[arg(short = 'f', long = "file-regex", value_name = "PATTERN", default_value = "")]
    file_regex: Vec<String>,

    /// Regular expression to filter directory names (default: ".*")
    #[arg(short = 'd', long = "dir-regex", value_name = "PATTERN", default_value = "")]
    dir_regex: Vec<String>,

	/// The case sensitive version of f and d
    #[arg(short = 'F', long = "file-regex-c", value_name = "PATTERN", default_value = "")]
    file_regex_c: Vec<String>,

    /// Regular expression to filter directory names (default: ".*")
    #[arg(short = 'D', long = "dir-regex-c", value_name = "PATTERN", default_value = "")]
    dir_regex_c: Vec<String>,


	/// Regular expression to filter by metadata (format: "field:pattern", e.g., "size:>1024", "modified:.*2023.*")
    #[arg(short = 'm', long = "meta-search", value_name = "FIELD:PATTERN")]
    meta_search: Vec<String>,

    /// Prune directory traversal: skip directories whose names don't match.
    #[arg(long)]
    prune_dirs: bool,



    /// Print full paths instead of the tree format
    #[arg(short = 'i', long = "flat", default_value_t = false)]
    no_indent: bool,


	/// Format string for file output(or use TREEE_FORMAT_FILE env) (e.g., "", "size=%size%, creation=%creation%")
    #[arg(short = 'p', long = "print-format", value_name = "FORMAT", required=false)]
    print_formatp: Option<Option<String>>,

	/// Format string for dir output(or use TREEE_FORMAT_DIR env) (e.g., "", "size=%size%, creation=%creation%")
    #[arg(short = 'P', long = "print-format", value_name = "FORMAT", required=false)]
    print_formatP: Option<Option<String>>,

}













fn main() -> Result<()> {
    let args = Args::parse();
    if !args.path.exists() {
        anyhow::bail!("Path '{}' does not exist or is not accessible.", args.path.display());
    }



    let use_color = !args.no_color && atty::is(atty::Stream::Stdout);
    colored::control::set_override(use_color);




	// handle meta search
	let meta_specified = !args.meta_search.is_empty();
	let meta_searches: Vec<MetaSearch> = args.meta_search.clone().iter().filter_map(|s| parse_meta_search(s).ok()).collect::<Vec<MetaSearch>>();

	let meta_matcher = move  |m: &fs::Metadata| -> (bool, Vec<bool>) {
		if !meta_specified { return (true, vec![]) }
		let mut matches: Vec<bool> = meta_searches.iter().map(|ms| matches_metadata(m,ms)).collect();
		if args.all { return (matches.iter().all(|&m| m), matches) }
		(matches.iter().any(|&m| m), matches)
    };


	// Build regex matching and closures to match files against cl regexes.
	let file_regex: Vec<String> = args.file_regex.clone().into_iter().map(|s| s.trim().to_string().to_lowercase()).filter(|s| !s.is_empty()).collect::<Vec<String>>();
	let dir_regex: Vec<String> = args.dir_regex.clone().into_iter().map(|s| s.trim().to_string().to_lowercase()).filter(|s| !s.is_empty()).collect::<Vec<String>>();
	let file_regex_c: Vec<String> = args.file_regex_c.clone().into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<String>>();
	let dir_regex_c: Vec<String> = args.dir_regex_c.clone().into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<String>>();

    let files_specified = !file_regex.is_empty() || !file_regex_c.is_empty();
    let dirs_specified = !dir_regex.is_empty() || !dir_regex_c.is_empty();


	let file_regexes: Result<Vec<Regex>, _> = file_regex.iter().map(|r| Regex::new( &r)).collect();
	let file_regexes = file_regexes?;
	let dir_regexes: Result<Vec<Regex>, _> = dir_regex.iter().map(|r| Regex::new( &r)).collect();
	let dir_regexes = dir_regexes?;

	let file_regexes_c: Result<Vec<Regex>, _> = file_regex_c.iter().map(|r| Regex::new(&r)).collect();
	let file_regexes_c = file_regexes_c?;
	let dir_regexes_c: Result<Vec<Regex>, _> = dir_regex_c.iter().map(|r| Regex::new(&r)).collect();
	let dir_regexes_c = dir_regexes_c?;

	let file_matcher = |file: &FileInfo| -> (bool, Vec<bool>) {
		let name = file.name.clone();
		let metadata = &fs::metadata(file.path.clone()).ok().expect("");
		if name.is_empty() { return (false, vec![]); }
		if !files_specified && !meta_specified { return (!dirs_specified, vec![]); }
		let mut matches: Vec<bool> = file_regexes.iter().map(|re| if files_specified { re.is_match(&name.to_lowercase()) } else { !dirs_specified }).collect();
		let mut matches_c: Vec<bool> = file_regexes_c.iter().map(|re| if files_specified { re.is_match(&name) } else { !dirs_specified }).collect();
		matches.append(&mut matches_c);
		if meta_specified { matches.append(&mut meta_matcher(metadata).1) }
		if args.all { return (matches.iter().all(|&m| m), matches) }
		(matches.iter().any(|&m| m), matches)
	};

	let dir_matcher = |dir: &DirInfo| -> (bool, Vec<bool>) {
		let name = dir.name.clone();
		let metadata = &fs::metadata(dir.path.clone()).ok().expect("");
		if name.is_empty() { return (false, vec![]); }
		if !dirs_specified && !meta_specified { return (!files_specified, vec![]); }
		let mut matches: Vec<bool> = dir_regexes.iter().map(|re| if dirs_specified { re.is_match(&name.to_lowercase()) } else { !files_specified }).collect();
		let mut matches_c: Vec<bool> = dir_regexes_c.iter().map(|re| if dirs_specified { re.is_match(&name) } else { !files_specified }).collect();
		matches.append(&mut matches_c);
		if meta_specified { matches.append(&mut meta_matcher(&metadata).1) }
		if args.all { return (matches.iter().all(|&m| m), matches) }
		(matches.iter().any(|&m| m), matches)
	};




    // Build the root EntryInfo
    let root_entry = build_directory_tree(
        &args.path,
        0,
        &file_matcher,
        &dir_matcher,
        &args,
		ParentRef::none()
    )?;



	fix_tree_recursive(&root_entry);

    // Print the tree
	let _ = print_tree_recursive(&root_entry, "", &file_matcher, &dir_matcher, &args);


    if args.summary {
        let total_dirs = root_entry.borrow().total_dirs;
		let total_files = root_entry.borrow().total_files;
	    let total_size: u64 = root_entry.borrow().total_size;

		let p_total_dirs = root_entry.borrow().p_total_dirs;
		let p_total_files = root_entry.borrow().p_total_files;
	    let p_total_size: u64 = root_entry.borrow().p_total_size;

		if files_specified || dirs_specified || meta_specified {
	    	println!("\nMatched {} directories, {} files", p_total_dirs, p_total_files);
        	println!("Matched total size: {} bytes", p_total_size);
		} else { println!() }

        println!("{} directories, {} files", total_dirs, total_files);
        println!("Total size: {} bytes", total_size);
    }

    Ok(())
}














fn build_directory_tree(
    path: &Path,
    current_depth: usize,
    file_matcher: &dyn Fn(&FileInfo) -> (bool, Vec<bool>),
    dir_matcher: &dyn Fn(&DirInfo) -> (bool, Vec<bool>),
    args: &Args,
    parent: ParentRef<DirInfo>,
) -> Result<Rc<RefCell<DirInfo>>> {

    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
	let full_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir =  Rc::new(RefCell::new(DirInfo {
        path: path.to_path_buf(),
        name,
        depth: current_depth,
        regex_matched: false,
        parent: parent.clone(),

		// total data
        immediate_files_size: 0,
        total_size: 0,
		total_files: 0,
		total_dirs: 0,

		// parsed data,
		p_immediate_files_size: 0,
		p_total_size: 0,
		p_total_files: 0,
		p_total_dirs: 0,

        sub_dirs: Vec::new(),
		sub_files: Vec::new(),
        contains_dir_matching_regex: false,
        contains_file_matching_regex: false,
		contains_meta_matching_regex: false,
    }));

	let drm = dir_matcher(&dir.borrow()).0;
	dir.borrow_mut().regex_matched = drm;



    // Max depth reached
    if current_depth >= args.depth { return Ok(dir); }

    // Read directory
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(dir),
    };

	// Loop through the elements
    for entry_result in entries {
		let entry = entry_result?;
		let path = entry.path();
		let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

		// Get the file size using windows. Using metadata  doesn't return the right file size in some cases for some reason
		let mut high: u32 = 0;
		let mut low: u32 = 0;
		{
			let pathos = path.clone().into_os_string();
			let mut pathw: Vec<u16> = Vec::with_capacity(pathos.len() + 1);
			pathw.extend(pathos.encode_wide());
			pathw.push(0);
			low = unsafe { GetCompressedFileSizeW(pathw.as_ptr(), &mut high) };
			if low == INVALID_FILE_SIZE { low = 0; high = 0; }
		}

		// Handle file case
    	if !path.is_dir() {
			let file =  Rc::new(RefCell::new(FileInfo {
				size: (u64::from(high) << 32 | u64::from(low)),
				path: path,
				name,
				regex_matched: false,
				parent: ParentRef::from_rc(&dir),
			}));
			let fm = file_matcher(&file.borrow()).0;
			file.borrow_mut().regex_matched = fm;
			dir.borrow_mut().sub_files.push(file);


			// Update ancestors containing file matching
			let fm = fm | dir.borrow().contains_file_matching_regex;
			dir.borrow_mut().contains_file_matching_regex = fm;
			let mut parent = dir.borrow_mut().parent.clone();
			while parent.is_some() && parent.is_valid() {
				let p = parent.upgrade().unwrap();
				let mut p: std::cell::RefMut<'_, DirInfo> = p.borrow_mut();
				p.contains_file_matching_regex = fm | p.contains_file_matching_regex;
				parent = p.parent.clone();
			}
			continue;
		} // -- file handling end





        // Recurse with current entry as parent
        let subdir = build_directory_tree(
            &path,
            current_depth + 1,
        	&file_matcher,
    		&dir_matcher,
            args,
			ParentRef::from_rc(&dir)
        )?;


		// update ancestors containing dir matching
		let dm = subdir.borrow().regex_matched | subdir.borrow().contains_dir_matching_regex | dir.borrow().contains_dir_matching_regex;
		dir.borrow_mut().contains_dir_matching_regex = dm;
		let mut parent = dir.borrow_mut().parent.clone();
		while parent.is_some() && parent.is_valid() {
			let p = parent.upgrade().unwrap();
			let mut p = p.borrow_mut();
			p.contains_dir_matching_regex = dm | p.contains_dir_matching_regex;
			parent = p.parent.clone()
		}

		// Update immediate statistics
		//let mut s = dir.borrow().immediate_files_size;
		//for i in &dir.borrow().sub_files { s += i.borrow().size; }
		//dir.borrow_mut().immediate_files_size = s;

        // Now update stats based on child
		dir.borrow_mut().sub_dirs.push(subdir.clone());


    }

	// We must order the sub-entries correctly as to get a nice output display that isn't too cluttered. We display files first then sub-directories.
    dir.borrow_mut().sub_dirs.sort_by(|a, b| { a.borrow().path.cmp(&b.borrow().path) });
	dir.borrow_mut().sub_files.sort_by(|a, b| { a.borrow().name.cmp(&b.borrow().name) });




    Ok(dir)
}




fn fix_tree_recursive(dir: &Rc<RefCell<DirInfo>>) {


	let mut dir = dir.borrow_mut();
	let mut ids: Vec<usize> = Vec::new();

	// Loop through files
	if dir.sub_files.len() > 0 { for i in 0..dir.sub_files.len() {
		let file = dir.sub_files[i].borrow();
		let rm = file.regex_matched;
		let size = file.size;
		drop(file);

		// update total statistics
		dir.immediate_files_size += size;
		dir.total_size += size;
		dir.total_files += 1;

		// If file not matched then skip
		if !rm { ids.push(i); continue; }

		// update parsed statistics
		dir.p_immediate_files_size += size;
		dir.p_total_size += size;
		dir.p_total_files += 1;
	}}

	// Remove unmatched files
	for i in ids.iter().rev() { dir.sub_files.remove(*i); }



	// Recurse over matched subdirs
	ids.clear();
	if dir.sub_dirs.len() > 0 { for i in 0..dir.sub_dirs.len() {
		let subdir = dir.sub_dirs[i].borrow_mut();
		let rm = subdir.regex_matched || subdir.contains_file_matching_regex || subdir.contains_dir_matching_regex;
		drop(subdir);

		// recurse matched directories
		fix_tree_recursive(&dir.sub_dirs[i].clone());

		// update total statistics
		updm!(dir.total_size, += dir.sub_dirs[i].borrow().total_size);
		updm!(dir.total_files, += dir.sub_dirs[i].borrow().total_files);
		updm!(dir.total_dirs, += 1 + dir.sub_dirs[i].borrow().total_dirs);

		// Skip directory if not matched
		if !rm { ids.push(i); continue; }



		// update parsed statistics
		updm!(dir.p_total_size, += dir.sub_dirs[i].borrow().p_total_size);
		updm!(dir.p_total_files, += dir.sub_dirs[i].borrow().p_total_files);
		updm!(dir.p_total_dirs, += 1 + dir.sub_dirs[i].borrow().p_total_dirs);

	}}


	// remove unmatched dirs
	for i in ids.iter().rev() { dir.sub_dirs.remove(*i); }




}



fn print_tree_recursive(
    dir: &Rc<RefCell<DirInfo>>,
    prefix: &str,
    file_matcher: &dyn Fn(&FileInfo) -> (bool, Vec<bool>),
    dir_matcher: &dyn Fn(&DirInfo) -> (bool, Vec<bool>),
    args: &Args,
) -> Result<()> {
    let dir_ref = dir.borrow();

	let fformat_str = get_format_string(&args, false);
	let dformat_str = get_format_string(&args, true);

    // Print files
    if dir_ref.sub_files.len() > 0 {
        for i in 0..dir_ref.sub_files.len() {

            let file = dir_ref.sub_files[i].borrow();
            let metadata = fs::metadata(&file.path).unwrap_or_else(|_| fs::metadata(&args.path).unwrap()); // fallback
            let s = if args.no_indent {
                "".to_string()
            } else {
                if i == dir_ref.sub_files.len() - 1 && dir_ref.sub_dirs.len() == 0 {
                    "└── ".to_string()
                } else {
                    "├── ".to_string()
                }
            };

            // Format the additional info using the format string
            let format_values = get_file_format_values(&*file, &metadata);
            let formatted_info = format_string(&fformat_str, &format_values, false);

            println!("{}{}{} {}",
                prefix,
                s,
                get_combined_color(&file.name, &file_matcher(&*file).1, false),
                formatted_info.dimmed()
            );
        }
    }

    // Print directories
    if dir_ref.sub_dirs.len() > 0 {
        for i in 0..dir_ref.sub_dirs.len() {
            let subdir = dir_ref.sub_dirs[i].borrow();
            let metadata = fs::metadata(&subdir.path).unwrap_or_else(|_| fs::metadata(&args.path).unwrap()); // fallback
            let s = if args.no_indent {
                "".to_string()
            } else {
                if i == dir_ref.sub_dirs.len() - 1 {
                    "└── ".to_string()
                } else {
                    "├── ".to_string()
                }
            };

            // Format the additional info using the format string
            let format_values = get_dir_format_values(&*subdir, &metadata);
            let formatted_info = format_string(&dformat_str, &format_values, true);

            println!("{}{}{} {}",
                prefix,
                s,
                get_combined_color(&subdir.name, &dir_matcher(&*subdir).1, true),
                formatted_info.dimmed()
            );

            let child_prefix = if args.no_indent {
                "".to_string()
            } else {
                prefix.to_owned() + if i == dir_ref.sub_dirs.len() - 1 {
                    "    "
                } else {
                    "│   "
                }
            };

            print_tree_recursive(&dir_ref.sub_dirs[i], &child_prefix, file_matcher, dir_matcher, args)?;
        }
    }

    Ok(())
}


