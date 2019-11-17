use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time,
};

use anyhow::{format_err, Result};
use rls_analysis::{AnalysisHost, Span, Target};
use rls_span::{Column, Range, Row, ZeroIndexed};

struct Query {
    file: PathBuf,
    line: usize,
    column: usize,
}

fn main() -> Result<()> {
    let query = parse_args()?;
    let range = find_ident_range(&query)?;

    let analysis = check_with_save_analysis()?;

    let span = Span::from_range(range, &query.file);
    let id = analysis.id(&span)?;
    let refs = analysis.find_all_refs_by_id(id)?;

    for span in refs {
        println!(
            "{}:{}:{}-{}",
            span.file.display(),
            span.range.row_start.0 + 1,
            span.range.col_start.0 + 1,
            span.range.col_end.0 + 1
        )
    }

    Ok(())
}

fn parse_args() -> Result<Query> {
    return parse().ok_or_else(|| format_err!("Usage: renamer path/to/file.rs:line:column"));

    fn parse() -> Option<Query> {
        let mut args = env::args();
        let arg = args.nth(1)?;

        if args.next().is_some() {
            return None;
        }
        let mut bits = arg.split(":");
        let file = bits.next().map(PathBuf::from)?;
        let line = bits.next()?.parse::<usize>().ok()?.checked_sub(1)?;
        let column = bits.next()?.parse::<usize>().ok()?.checked_sub(1)?;
        if bits.next().is_some() {
            return None;
        }
        Some(Query { file, line, column })
    }
}

fn check_with_save_analysis() -> Result<AnalysisHost> {
    Command::new("cargo")
        .arg("check")
        .env("RUSTC_BOOTSTRAP", "1")
        .env("CARGO_TARGET_DIR", "target/rls")
        .env("RUSTFLAGS", "-Zunstable-options -Zsave-analysis")
        .status()?;

    let analysis = AnalysisHost::new(Target::Debug);

    let start = time::Instant::now();
    eprintln!("Loading analysis ...");
    analysis.reload(Path::new("."), Path::new("."))?;
    eprintln!("... loaded ({:?})!", start.elapsed());

    Ok(analysis)
}

fn find_ident_range(q: &Query) -> Result<Range<ZeroIndexed>> {
    let text = fs::read_to_string(&q.file)?;
    let (start, end) = find_ident_range(&text, q.line, q.column)
        .ok_or_else(|| format_err!("Can't find identifier"))?;

    return Ok(Range::new(
        Row::new_zero_indexed(q.line as u32),
        Row::new_zero_indexed(q.line as u32),
        Column::new_zero_indexed(start as u32),
        Column::new_zero_indexed(end as u32),
    ));

    fn find_ident_range(text: &str, line: usize, column: usize) -> Option<(usize, usize)> {
        let line = text.lines().nth(line)?;
        for (start, end) in word_ranges(&line) {
            if start <= column && column <= end {
                return Some((start, end));
            }
        }
        None
    }
}

fn word_ranges(s: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut res = Vec::new();
    let mut offset = 0;
    let mut word = None;

    for c in s.chars() {
        let c_len = c.len_utf8();

        if c.is_ascii_alphabetic() {
            word.get_or_insert((offset, offset)).1 += c_len;
        } else {
            if let Some(word) = word {
                res.push(word)
            }
            word = None;
        }
        offset += c_len;
    }
    if let Some(word) = word {
        res.push(word)
    }
    res.into_iter()
}
