//! `qiyas-cli`: Qiyas without a window. Converts drawings between DWG and DXF, runs command
//! scripts (`qiyas_core::script`), counts blocks into a take-off workbook, and runs one script on
//! every drawing of a folder or a sheet set with a line per drawing.
//!
//! It opens drawings as the app does (the failsafe reader, the `.qiyas` project file beside a
//! drawing when there is one) and writes only what it is asked to: a converted file, a script's
//! SAVE, SAVEAS and EXPORT, a take-off workbook. It never opens a window and never touches the
//! app's settings, the keychain, MCP or the AI. Any error is one line on stderr
//! (`qiyas-cli: ...`) and a non-zero exit: 1, or 2 for a mistake in the arguments.

use std::any::Any;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use clap::error::ErrorKind;
use clap::{ArgGroup, Parser, Subcommand};
use opencadcodec::DxfVersion;
use qiyas_core::script::{self, Context, Host, Run};
use qiyas_core::{Boq, Command, Outcome, Session, SheetSet, Source};
use serde::Serialize;

#[derive(Parser)]
#[command(
    name = "qiyas-cli",
    version,
    about = "Qiyas without a window: convert drawings, run command scripts, take off, batch."
)]
struct Cli {
    /// Print nothing but errors (on stderr).
    #[arg(long, global = true)]
    quiet: bool,
    /// Print the result as JSON.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Converts a drawing between DWG and DXF, by the output's extension. A Save As: the project
    /// file comes along beside the output.
    Convert {
        input: PathBuf,
        output: PathBuf,
        /// The output's version: 2000, 2004, 2007, 2010, 2013 or 2018 (default: the input's).
        #[arg(long, value_parser = parse_version)]
        version: Option<DxfVersion>,
    },
    /// Runs a command script (.scr) on the drawing, or on what the script OPENs.
    Run {
        script: PathBuf,
        drawing: Option<PathBuf>,
    },
    /// Counts blocks and exports the drawing's take-off to Excel. Writes nothing else.
    #[command(group(ArgGroup::new("what").args(["counts", "export"]).required(true).multiple(true)))]
    Takeoff {
        drawing: PathBuf,
        /// A block to count; repeat for more.
        #[arg(long = "count", value_name = "BLOCK")]
        counts: Vec<String>,
        /// The workbook to write the take-off to.
        #[arg(long, value_name = "XLSX")]
        export: Option<PathBuf>,
    },
    /// Runs a script on every drawing (.dwg, .dxf) of a folder or a sheet set (.qss).
    Batch {
        /// A folder (its own drawings, not its subfolders) or a sheet set (.qss).
        source: PathBuf,
        #[arg(long)]
        script: PathBuf,
        /// Where the scripts' written files go (default: the script's folder).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Drawings at a time.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..=64))]
        jobs: u16,
    },
}

fn parse_version(text: &str) -> Result<DxfVersion, String> {
    Ok(match text {
        "2000" => DxfVersion::AC1015,
        "2004" => DxfVersion::AC1018,
        "2007" => DxfVersion::AC1021,
        "2010" => DxfVersion::AC1024,
        "2013" => DxfVersion::AC1027,
        "2018" => DxfVersion::AC1032,
        _ => return Err("give 2000, 2004, 2007, 2010, 2013 or 2018".to_string()),
    })
}

fn main() -> ExitCode {
    // A panic (the reader on a damaged drawing) becomes a one-line error below, not a backtrace.
    std::panic::set_hook(Box::new(|_| {}));
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            e.exit()
        }
        Err(e) => {
            eprintln!("qiyas-cli: {}", usage_error(&e));
            return ExitCode::from(2);
        }
    };
    let out = Out {
        quiet: cli.quiet,
        json: cli.json,
    };
    match catch_unwind(AssertUnwindSafe(|| dispatch(&cli.command, &out))) {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(message)) => {
            eprintln!("qiyas-cli: {message}");
            ExitCode::FAILURE
        }
        Err(panic) => {
            eprintln!("qiyas-cli: crashed: {}", panic_text(&*panic));
            ExitCode::FAILURE
        }
    }
}

/// Clap's message in one line: its text before the usage and the pointer to --help, without
/// "error: ".
fn usage_error(e: &clap::Error) -> String {
    if e.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand {
        return "give a command: convert, run, takeoff or batch (see --help)".to_string();
    }
    let text = e.to_string();
    let text = text.split("\n\nUsage:").next().unwrap_or(&text);
    let text = text
        .split("\n\nFor more information")
        .next()
        .unwrap_or(text);
    let text = text.strip_prefix("error: ").unwrap_or(text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn panic_text(panic: &(dyn Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown error".to_string())
}

/// What goes on stdout: text, JSON or nothing.
struct Out {
    quiet: bool,
    json: bool,
}

impl Out {
    fn text(&self, line: impl std::fmt::Display) {
        if !self.quiet && !self.json {
            println!("{line}");
        }
    }

    fn json(&self, value: &impl Serialize) {
        if !self.quiet && self.json {
            println!(
                "{}",
                serde_json::to_string_pretty(value).expect("plain data")
            );
        }
    }
}

fn dispatch(cmd: &Cmd, out: &Out) -> Result<(), String> {
    match cmd {
        Cmd::Convert {
            input,
            output,
            version,
        } => convert(out, input, output, *version),
        Cmd::Run { script, drawing } => run(out, script, drawing.as_deref()),
        Cmd::Takeoff {
            drawing,
            counts,
            export,
        } => takeoff(out, drawing, counts, export.as_deref()),
        Cmd::Batch {
            source,
            script,
            out: out_dir,
            jobs,
        } => batch(out, source, script, out_dir.as_deref(), usize::from(*jobs)),
    }
}

/// The Excel and comparison steps of a script, as the desktop does them.
struct Excel;

impl Host for Excel {
    fn export_takeoff(&mut self, session: &Session, path: &Path) -> Result<(), String> {
        let title = match session.drawing.path.file_name() {
            Some(name) => format!("{} take-off", name.to_string_lossy()),
            None => "Take-off".to_string(),
        };
        qiyas_boq::export_takeoff(
            path,
            &session.takeoff.items,
            &session.takeoff.totals(),
            &title,
        )
        .map_err(|e| e.to_string())
    }

    fn import_boq(&mut self, path: &Path) -> Result<Boq, String> {
        qiyas_boq::import_boq(path).map_err(|e| e.to_string())
    }

    fn compare(&mut self, session: &mut Session) -> Result<usize, String> {
        qiyas_takeoff::compare_session(session, qiyas_takeoff::DEFAULT_TOLERANCE, Source::User)
            .map(|conflicts| conflicts.len())
            .map_err(|e| e.to_string())
    }
}

/// `path` opened, or the error naming it.
fn open(path: &Path) -> Result<Session, String> {
    Session::open(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_script(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn convert(
    out: &Out,
    input: &Path,
    output: &Path,
    version: Option<DxfVersion>,
) -> Result<(), String> {
    let mut session = open(input)?;
    if let Some(version) = version {
        session.drawing.doc.version = version;
    }
    let version = session.drawing.doc.version;
    session
        .save_as(output)
        .map_err(|e| format!("{}: {e}", output.display()))?;
    let problems = session.drawing.problems.len();
    out.text(format!(
        "converted {} to {} ({}){}",
        input.display(),
        output.display(),
        version.as_str(),
        problems_note(problems)
    ));
    out.json(&serde_json::json!({
        "input": input,
        "output": output,
        "version": version.as_str(),
        "problems": problems,
    }));
    Ok(())
}

/// ", N problems reading it" when there were any.
fn problems_note(problems: usize) -> String {
    match problems {
        0 => String::new(),
        n => format!(", {n} problems reading it"),
    }
}

#[derive(Serialize)]
struct RunReport<'a> {
    /// What the reader had to skip or repair in the drawing given on the command line.
    problems: usize,
    #[serde(flatten)]
    run: &'a Run,
}

fn run(out: &Out, script_path: &Path, drawing: Option<&Path>) -> Result<(), String> {
    let text = read_script(script_path)?;
    let cx = Context {
        script: script_path,
        drawing,
        out_dir: None,
    };
    let in_script = |e: script::ScriptError| format!("{}: {e}", script_path.display());
    let script = script::parse(&text, &cx).map_err(in_script)?;
    let session = drawing.map(open).transpose()?;
    let problems = session.as_ref().map_or(0, |s| s.drawing.problems.len());
    if let (Some(path), true) = (drawing, problems > 0) {
        out.text(format!(
            "{}: {problems} problems reading it",
            path.display()
        ));
    }
    let run = script::run(&script, session, &mut Excel).map_err(in_script)?;
    for step in &run.steps {
        out.text(format!("line {}: {}", step.line, step.text));
    }
    out.json(&RunReport {
        problems,
        run: &run,
    });
    Ok(())
}

fn takeoff(
    out: &Out,
    drawing: &Path,
    blocks: &[String],
    export: Option<&Path>,
) -> Result<(), String> {
    let mut session = open(drawing)?;
    let mut counts = BTreeMap::new();
    for name in blocks {
        let outcome = session
            .run(Command::CountBlock {
                block_name: name.clone(),
                layer: None,
            })
            .map_err(|e| format!("COUNT {name}: {e}"))?;
        if let Outcome::Measured { measurement: m } = outcome {
            out.text(format!("{} = {:.0} {}", m.description, m.shown, m.unit));
            counts.insert(name.clone(), m.shown);
        }
    }
    if let Some(path) = export {
        Excel
            .export_takeoff(&session, path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        out.text(format!("exported {}", path.display()));
    }
    out.json(&serde_json::json!({
        "drawing": drawing,
        "problems": session.drawing.problems.len(),
        "counts": counts,
        "export": export,
    }));
    Ok(())
}

/// One drawing of a batch: whether its script ran, with how many problems reading it, its block
/// counts, or why it failed.
#[derive(Serialize)]
struct FileReport {
    path: PathBuf,
    /// "ok", "warning" (it ran, but the reader had problems with the drawing) or "error".
    status: &'static str,
    problems: usize,
    counts: BTreeMap<String, f64>,
    error: Option<String>,
}

impl FileReport {
    fn line(&self) -> String {
        let path = self.path.display();
        if let Some(error) = &self.error {
            return format!("{path} error {error}");
        }
        let mut line = format!("{path} {}", self.status);
        if self.problems > 0 {
            line.push_str(&format!(" ({} problems reading it)", self.problems));
        }
        for (name, count) in &self.counts {
            line.push_str(&format!(" {name}={count}"));
        }
        line
    }
}

#[derive(Serialize)]
struct BatchReport {
    files: Vec<FileReport>,
    total: usize,
    ok: usize,
    warnings: usize,
    failed: usize,
}

/// The drawings of a folder (sorted by name) or of a sheet set (in its order).
fn drawings_of(source: &Path) -> Result<Vec<PathBuf>, String> {
    let is_drawing = |p: &Path| {
        p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("dwg") || e.eq_ignore_ascii_case("dxf"))
    };
    let files: Vec<PathBuf> = if source.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(source)
            .map_err(|e| format!("{}: {e}", source.display()))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| is_drawing(p))
            .collect();
        files.sort();
        files
    } else if source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("qss"))
    {
        let set = SheetSet::load(source).map_err(|e| format!("{}: {e}", source.display()))?;
        qiyas_core::sheet_set_entries(source, &set)
            .into_iter()
            .map(|entry| PathBuf::from(entry.full))
            .collect()
    } else {
        return Err(format!(
            "{} is neither a folder nor a sheet set (.qss)",
            source.display()
        ));
    };
    if files.is_empty() {
        return Err(format!("no drawings in {}", source.display()));
    }
    Ok(files)
}

fn batch(
    out: &Out,
    source: &Path,
    script_path: &Path,
    out_dir: Option<&Path>,
    jobs: usize,
) -> Result<(), String> {
    let files = drawings_of(source)?;
    let text = read_script(script_path)?;
    // A bad script fails before any drawing is touched.
    script::parse(
        &text,
        &Context {
            script: script_path,
            drawing: Some(&files[0]),
            out_dir,
        },
    )
    .map_err(|e| format!("{}: {e}", script_path.display()))?;
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    // Workers take the next drawing in turn, each opening its own session; the report is
    // printed in the folder's order as soon as the drawings before it are done.
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    let mut reports: Vec<FileReport> = Vec::with_capacity(files.len());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(files.len()) {
            let tx = tx.clone();
            let (next, files, text) = (&next, &files, &text);
            std::thread::Builder::new()
                // The reader recurses on nested blocks: room for deep drawings.
                .stack_size(16 << 20)
                .spawn_scoped(scope, move || loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(file) = files.get(i) else { break };
                    let report = run_one(file, text, script_path, out_dir);
                    if tx.send((i, report)).is_err() {
                        break;
                    }
                })
                .expect("a worker thread");
        }
        drop(tx);
        let mut waiting = BTreeMap::new();
        for (i, report) in rx {
            waiting.insert(i, report);
            while let Some(report) = waiting.remove(&reports.len()) {
                out.text(report.line());
                reports.push(report);
            }
        }
    });

    let count = |status: &str| reports.iter().filter(|r| r.status == status).count();
    let (ok, warnings, failed) = (count("ok"), count("warning"), count("error"));
    let total = reports.len();
    out.text(format!(
        "{total} drawings: {ok} ok, {warnings} with warnings, {failed} failed"
    ));
    out.json(&BatchReport {
        files: reports,
        total,
        ok,
        warnings,
        failed,
    });
    match failed {
        0 => Ok(()),
        n => Err(format!("{n} of {total} drawings failed")),
    }
}

/// Runs the script on one drawing of a batch; a panic (the reader on a damaged file) is that
/// drawing's error, not the batch's end.
fn run_one(file: &Path, text: &str, script_path: &Path, out_dir: Option<&Path>) -> FileReport {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let cx = Context {
            script: script_path,
            drawing: Some(file),
            out_dir,
        };
        let script = script::parse(text, &cx).map_err(|e| e.to_string())?;
        let session = Session::open(file).map_err(|e| e.to_string())?;
        let problems = session.drawing.problems.len();
        let run = script::run(&script, Some(session), &mut Excel).map_err(|e| e.to_string())?;
        Ok((problems, run.counts))
    }))
    .unwrap_or_else(|panic| Err(format!("crashed: {}", panic_text(&*panic))));
    match result {
        Ok((problems, counts)) => FileReport {
            path: file.to_path_buf(),
            status: if problems > 0 { "warning" } else { "ok" },
            problems,
            counts,
            error: None,
        },
        Err(error) => FileReport {
            path: file.to_path_buf(),
            status: "error",
            problems: 0,
            counts: BTreeMap::new(),
            error: Some(error),
        },
    }
}
