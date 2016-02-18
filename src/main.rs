// The MIT License
//
// Copyright 2016 Marek Baczynski
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.


extern crate rustc_serialize;
extern crate docopt;
extern crate walkdir;
extern crate glob;
extern crate filetime;
extern crate chrono;



use std::cmp;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

use docopt::Docopt;
use walkdir::WalkDir;
use glob::Pattern;
use filetime::FileTime;
use chrono::*;

const USAGE: &'static str = "
Organize folders by mtime of files.

Usage:
  organize-by-time  [--oldest | --newest] \
                    [--pattern=PATTERN]... \
                    [--not-pattern=PATTERN]... \
                    [--output-dir=OUTPUT] \
                    [--strip=N] \
                    [--dry-run] \
                    [--force] \
                    <directory>...
  organize-by-time (-h | --help)
  organize-by-time --version

Options:
  -O OUTPUT --output-dir=OUTPUT     Output directory. [default: .]
  -P PATTERN --not-pattern=PATTERN  Ignore files with this pattern.
  -d --dry-run                      Only print, do not move any files.
  -f --force                        Overwrite files if conflict found.
  -n --newest                       Use the newest file in the directory.
  -o --oldest                       Use the oldest file in the directory (default).
  -p PATTERN --pattern=PATTERN      Only consider files with this pattern.
  -s N --strip N                    Strip N leftmost directories [default: 0]
  -h --help                         Show this screen.
  --version                         Show version.
";

#[derive(Debug, RustcDecodable)]
struct Args {
    arg_directory: Vec<String>,
    flag_oldest: bool,
    flag_newest: bool,
    flag_pattern: Vec<String>,
    flag_not_pattern: Vec<String>,
    flag_output_dir: String,
    flag_strip: usize,
    flag_dry_run: bool,
    flag_force: bool,
    flag_version: bool,
}


#[derive(Clone, Copy, Debug)]
enum AgePolicy {
    Default,
    Oldest,
    Newest,
}

use AgePolicy::*;


use std::io::Write;

macro_rules! println_stderr(
    ($($arg:tt)*) => (
        match writeln!(&mut ::std::io::stderr(), $($arg)* ) {
            Ok(_) => {},
            Err(x) => panic!("Unable to write to stderr: {}", x),
        }
    )
);


fn move_single_file(src: &Path, dst: &Path, force: bool) -> io::Result<()> {
    if let Some(dstparent) = dst.parent() {
        try!(fs::create_dir_all(dstparent));
        if !force && dst.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists,
                                      "destination file already exists"));
        }
        try!(fs::rename(src, dst));
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "parent path impossible to compute"))
    }
}


// returns error count
fn move_batch(batch: &mut Vec<(PathBuf, PathBuf)>,
              datetime: &NaiveDateTime,
              output_dir: &Path,
              force: bool,
              dry_run: bool)
              -> isize {
    let mut errors: isize = 0;
    for e in batch.iter() {
        let src = &e.0;
        let dst = &e.1;
        let mut fin = PathBuf::from(output_dir);
        fin.push(datetime.year().to_string());
        fin.push(dst);
        println!("move {:?} {:?}", src, fin);
        if !dry_run {
            if let Err(e) = move_single_file(&src, &fin, force) {
                println_stderr!("Error: dest: {:?}: {}", fin, e);
                errors += 1;
            }
        }
    }
    batch.clear();
    errors
}

// returns error count
fn process_dir(dir: &str,
               policy: AgePolicy,
               match_patterns: &[String],
               not_match_patterns: &[String],
               output_dir: &str,
               strip: usize,
               force: bool,
               dry_run: bool)
               -> isize {

    // matching patterns
    let mps: Vec<_> = if !match_patterns.is_empty() {
        match_patterns.iter().map(|s| Pattern::new(s).unwrap()).collect()
    } else {
        vec![Pattern::new("*").unwrap()]
    };
    // not-matching patterns
    let notps: Vec<_> = not_match_patterns.iter().map(|s| Pattern::new(s).unwrap()).collect();

    // the batch to move
    let mut curfiles: Vec<(PathBuf, PathBuf)> = vec![];
    // for tracking batch extreme mtime
    let mut datetime = match policy {
        Newest => NaiveDateTime::from_timestamp(0, 0),
        _ => NaiveDateTime::from_timestamp(1i64 << 40, 0),
    };

    let output_pathbuf = PathBuf::from(output_dir);
    let mut errors: isize = 0;

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if entry.path().is_file() {
            let matched = mps.iter().any(|p| p.matches(&*entry.file_name().to_string_lossy()));
            let not_matched = notps.iter()
                                   .any(|p| p.matches(&*entry.file_name().to_string_lossy()));
            if !matched || not_matched {
                continue;
            }
            // get current mtime
            let md = fs::metadata(&*entry.path().to_string_lossy()).unwrap();
            let mtime = FileTime::from_last_modification_time(&md);
            let dt = NaiveDateTime::from_timestamp(mtime.seconds_relative_to_1970() as i64,
                                                   mtime.nanoseconds());
            // strip leftmost directories if neccessary
            let mut output = PathBuf::new();
            let mut components = entry.path().components();
            for _ in 0..strip {
                components.next();
            }
            output.push(components.as_path());
            // add file to the batch
            curfiles.push((PathBuf::from(entry.path()), output));
            // update desired time of whole batch
            datetime = match policy {
                Newest => cmp::max(datetime, dt),
                _ => cmp::min(datetime, dt),
            };

        } else if entry.path().is_dir() {
            // if back to depth 2, create folders and move paths
            if entry.depth() <= 2 {
                errors += move_batch(&mut curfiles, &datetime, &output_pathbuf, force, dry_run);
                // reinitialize datetime
                datetime = match policy {
                    Newest => NaiveDateTime::from_timestamp(0, 0),
                    _ => NaiveDateTime::from_timestamp(1i64 << 40, 0),
                };
            }
        }
    }
    // move after exiting the loop
    errors += move_batch(&mut curfiles, &datetime, &output_pathbuf, force, dry_run);
    errors
}


fn main() {
    let args: Args = Docopt::new(USAGE)
                         .and_then(|d| d.decode())
                         .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("organize-by-mtime v1.0.0");
        return;
    }

    let agepolicy: AgePolicy = match (args.flag_oldest, args.flag_newest) {
        (false, false) => Default,
        (true, false) => Oldest,
        (false, true) => Newest,
        (true, true) => panic!("Can't specify both newest and oldest."),
    };

    let mut errors: isize = 0;

    for dir in args.arg_directory {
        errors += process_dir(&dir,
                              agepolicy,
                              &args.flag_pattern[..],
                              &args.flag_not_pattern[..],
                              &args.flag_output_dir,
                              args.flag_strip,
                              args.flag_force,
                              args.flag_dry_run)
    }

    if errors > 0 {
        println_stderr!("total errors: {}", errors);
        process::exit(1);
    }
}
