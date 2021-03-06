#![crate_name = "uu_join"]

/*
 * This file is part of the uutils coreutils package.
 *
 * (c) Konstantin Pospelov <kupospelov@gmail.com>
 *
 * For the full copyright and license information, please view the LICENSE
 * file that was distributed with this source code.
 */

extern crate clap;

#[macro_use]
extern crate uucore;

use std::fs::File;
use std::io::{BufRead, BufReader, Lines, Stdin, stdin};
use std::cmp::Ordering;
use clap::{App, Arg};

static NAME: &'static str = "join";
static VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(PartialEq)]
enum FileNum {
    None,
    File1,
    File2,
}

#[derive(Copy, Clone)]
enum Sep {
    Char(char),
    Line,
    Whitespaces,
}

struct Settings {
    key1: usize,
    key2: usize,
    print_unpaired: FileNum,
    ignore_case: bool,
    separator: Sep,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            key1: 0,
            key2: 0,
            print_unpaired: FileNum::None,
            ignore_case: false,
            separator: Sep::Whitespaces,
        }
    }
}

struct Line {
    fields: Vec<String>,
}

impl Line {
    fn new(string: String, separator: Sep) -> Line {
        let fields = match separator {
            Sep::Whitespaces => string.split_whitespace().map(String::from).collect(),
            Sep::Char(sep) => string.split(sep).map(String::from).collect(),
            Sep::Line => vec![string],
        };

        Line { fields }
    }

    /// Get field at index.
    fn get_field(&self, index: usize) -> &str {
        if index < self.fields.len() {
            &self.fields[index]
        } else {
            ""
        }
    }

    /// Print each field except the one at the index.
    fn print_fields(&self, index: usize, separator: char) {
        for i in 0..self.fields.len() {
            if i != index {
                print!("{}{}", separator, self.fields[i]);
            }
        }
    }
}

struct State<'a> {
    key: usize,
    print_unpaired: bool,
    lines: Lines<Box<BufRead + 'a>>,
    seq: Vec<Line>,
}

impl<'a> State<'a> {
    fn new(name: &str, stdin: &'a Stdin, key: usize, print_unpaired: bool) -> State<'a> {
        let f = if name == "-" {
            Box::new(stdin.lock()) as Box<BufRead>
        } else {
            match File::open(name) {
                Ok(file) => Box::new(BufReader::new(file)) as Box<BufRead>,
                Err(err) => crash!(1, "{}: {}", name, err),
            }
        };

        State {
            key: key,
            print_unpaired: print_unpaired,
            lines: f.lines(),
            seq: Vec::new(),
        }
    }

    /// Compare the key fields of the two current lines.
    fn compare(&self, other: &State, ignore_case: bool) -> Ordering {
        let key1 = self.seq[0].get_field(self.key);
        let key2 = other.seq[0].get_field(other.key);

        compare(key1, key2, ignore_case)
    }

    /// Skip the current unpaired line.
    fn skip_line(&mut self, read_sep: Sep, write_sep: char) {
        if self.print_unpaired {
            self.print_unpaired_line(&self.seq[0], write_sep);
        }

        match self.read_line(read_sep) {
            Some(line) => self.seq[0] = line,
            None => self.seq.clear(),
        }
    }

    /// Keep reading line sequence until the key does not change, return
    /// the first line whose key differs.
    fn extend(&mut self, read_sep: Sep, ignore_case: bool) -> Option<Line> {
        while let Some(line) = self.read_line(read_sep) {
            let diff = compare(
                self.seq[0].get_field(self.key),
                line.get_field(self.key),
                ignore_case,
            );

            if diff == Ordering::Equal {
                self.seq.push(line);
            } else {
                return Some(line);
            }
        }

        return None;
    }

    /// Combine two line sequences.
    fn combine(&self, other: &State, write_sep: char) {
        let key = self.seq[0].get_field(self.key);

        for line1 in &self.seq {
            for line2 in &other.seq {
                print!("{}", key);
                line1.print_fields(self.key, write_sep);
                line2.print_fields(other.key, write_sep);
                println!();
            }
        }
    }

    /// Reset with the next line.
    fn reset(&mut self, next_line: Option<Line>) {
        self.seq.clear();

        if let Some(line) = next_line {
            self.seq.push(line);
        }
    }

    fn has_line(&self) -> bool {
        !self.seq.is_empty()
    }

    fn initialize(&mut self, read_sep: Sep) {
        if let Some(line) = self.read_line(read_sep) {
            self.seq.push(line);
        }
    }

    fn finalize(&mut self, read_sep: Sep, write_sep: char) {
        if self.has_line() && self.print_unpaired {
            self.print_unpaired_line(&self.seq[0], write_sep);

            while let Some(line) = self.read_line(read_sep) {
                self.print_unpaired_line(&line, write_sep);
            }
        }
    }

    fn read_line(&mut self, sep: Sep) -> Option<Line> {
        match self.lines.next() {
            Some(value) => Some(Line::new(crash_if_err!(1, value), sep)),
            None => None,
        }
    }

    fn print_unpaired_line(&self, line: &Line, sep: char) {
        print!("{}", line.get_field(self.key));
        line.print_fields(self.key, sep);
        println!();
    }
}

pub fn uumain(args: Vec<String>) -> i32 {
    let matches = App::new(NAME)
        .version(VERSION)
        .about(
            "For each pair of input lines with identical join fields, write a line to
standard output. The default join field is the first, delimited by blanks.

When FILE1 or FILE2 (not both) is -, read standard input.")
        .help_message("display this help and exit")
        .version_message("display version and exit")
        .arg(Arg::with_name("a")
            .short("a")
            .takes_value(true)
            .possible_values(&["1", "2"])
            .value_name("FILENUM")
            .help("also print unpairable lines from file FILENUM, where
FILENUM is 1 or 2, corresponding to FILE1 or FILE2"))
        .arg(Arg::with_name("i")
            .short("i")
            .long("ignore-case")
            .help("ignore differences in case when comparing fields"))
        .arg(Arg::with_name("j")
            .short("j")
            .takes_value(true)
            .value_name("FIELD")
            .help("equivalent to '-1 FIELD -2 FIELD'"))
        .arg(Arg::with_name("t")
            .short("t")
            .takes_value(true)
            .value_name("CHAR")
            .help("use CHAR as input and output field separator"))
        .arg(Arg::with_name("1")
            .short("1")
            .takes_value(true)
            .value_name("FIELD")
            .help("join on this FIELD of file 1"))
        .arg(Arg::with_name("2")
            .short("2")
            .takes_value(true)
            .value_name("FIELD")
            .help("join on this FIELD of file 2"))
        .arg(Arg::with_name("file1")
            .required(true)
            .value_name("FILE1")
            .hidden(true))
        .arg(Arg::with_name("file2")
            .required(true)
            .value_name("FILE2")
            .hidden(true))
        .get_matches_from(args);

    let keys = parse_field_number(matches.value_of("j"));
    let key1 = parse_field_number(matches.value_of("1"));
    let key2 = parse_field_number(matches.value_of("2"));

    let mut settings: Settings = Default::default();
    settings.print_unpaired = match matches.value_of("a") {
        Some(value) => {
            match value {
                "1" => FileNum::File1,
                "2" => FileNum::File2,
                value => crash!(1, "invalid file number: {}", value),
            }
        }
        None => FileNum::None,
    };
    settings.ignore_case = matches.is_present("i");
    settings.key1 = get_field_number(keys, key1);
    settings.key2 = get_field_number(keys, key2);

    if let Some(value) = matches.value_of("t") {
        settings.separator = match value.len() {
            0 => Sep::Line,
            1 => Sep::Char(value.chars().nth(0).unwrap()),
            _ => crash!(1, "multi-character tab {}", value),
        };
    }

    let file1 = matches.value_of("file1").unwrap();
    let file2 = matches.value_of("file2").unwrap();

    if file1 == "-" && file2 == "-" {
        crash!(1, "both files cannot be standard input");
    }

    exec(file1, file2, &settings)
}

fn exec(file1: &str, file2: &str, settings: &Settings) -> i32 {
    let stdin = stdin();

    let mut state1 = State::new(
        &file1,
        &stdin,
        settings.key1,
        settings.print_unpaired == FileNum::File1,
    );

    let mut state2 = State::new(
        &file2,
        &stdin,
        settings.key2,
        settings.print_unpaired == FileNum::File2,
    );

    let write_sep = match settings.separator {
        Sep::Char(sep) => sep,
        _ => ' ',
    };

    state1.initialize(settings.separator);
    state2.initialize(settings.separator);

    while state1.has_line() && state2.has_line() {
        let diff = state1.compare(&state2, settings.ignore_case);

        match diff {
            Ordering::Less => {
                state1.skip_line(settings.separator, write_sep);
            }
            Ordering::Greater => {
                state2.skip_line(settings.separator, write_sep);
            }
            Ordering::Equal => {
                let next_line1 = state1.extend(settings.separator, settings.ignore_case);
                let next_line2 = state2.extend(settings.separator, settings.ignore_case);

                state1.combine(&state2, write_sep);

                state1.reset(next_line1);
                state2.reset(next_line2);
            }
        }
    }

    state1.finalize(settings.separator, write_sep);
    state2.finalize(settings.separator, write_sep);

    0
}

/// Check that keys for both files and for a particular file are not
/// contradictory and return the zero-based key index.
fn get_field_number(keys: Option<usize>, key: Option<usize>) -> usize {
    if let Some(keys) = keys {
        if let Some(key) = key {
            if keys != key {
                crash!(1, "incompatible join fields {}, {}", keys, key);
            }
        }

        return keys - 1;
    }

    match key {
        Some(key) => key - 1,
        None => 0,
    }
}

/// Parse the specified field string as a natural number and return it.
fn parse_field_number(value: Option<&str>) -> Option<usize> {
    match value {
        Some(value) => {
            match value.parse() {
                Ok(result) if result > 0 => Some(result),
                _ => crash!(1, "invalid field number: '{}'", value),
            }
        }
        None => None,
    }
}

fn compare(field1: &str, field2: &str, ignore_case: bool) -> Ordering {
    if ignore_case {
        field1.to_lowercase().cmp(&field2.to_lowercase())
    } else {
        field1.cmp(field2)
    }
}
