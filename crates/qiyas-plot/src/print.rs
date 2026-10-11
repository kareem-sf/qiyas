//! Printing hands the plotted PDF to the operating system: Linux `lp -d <printer>` (or the PDF
//! viewer through `xdg-open` when no printer is chosen), macOS `lpr`, Windows the PDF handler's
//! Print or PrintTo verb. Qiyas draws no print dialog of its own.

use std::path::Path;
use std::process::Command;

use crate::PlotError;

/// What printing a PDF runs: `program` with `args` (no shell), waited for or not, and what to
/// tell the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintPlan {
    pub program: String,
    pub args: Vec<String>,
    /// Wait for it and report its error (a print command); a viewer is left running.
    pub wait: bool,
    pub message: String,
}

/// A single-quoted PowerShell string.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// How to print `pdf` on `os` (`std::env::consts::OS`), on `printer` or the default one.
pub fn print_plan(os: &str, pdf: &Path, printer: Option<&str>) -> PrintPlan {
    let file = pdf.display().to_string();
    let sent = |p: &str| format!("Sent to {p}");
    match (os, printer) {
        ("windows", printer) => {
            let script = match printer {
                Some(p) => format!(
                    "Start-Process -FilePath {} -Verb PrintTo -ArgumentList {}",
                    ps_quote(&file),
                    ps_quote(&format!("\"{p}\""))
                ),
                None => format!("Start-Process -FilePath {} -Verb Print", ps_quote(&file)),
            };
            PrintPlan {
                program: "powershell".into(),
                args: vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    script,
                ],
                wait: true,
                message: match printer {
                    Some(p) => sent(p),
                    None => "Sent to the default printer".into(),
                },
            }
        }
        ("macos", printer) => {
            let mut args = Vec::new();
            if let Some(p) = printer {
                args.extend(["-P".to_string(), p.to_string()]);
            }
            args.push(file);
            PrintPlan {
                program: "lpr".into(),
                args,
                wait: true,
                message: printer.map_or("Sent to the default printer".into(), sent),
            }
        }
        (_, Some(p)) => PrintPlan {
            program: "lp".into(),
            args: vec!["-d".into(), p.into(), file],
            wait: true,
            message: sent(p),
        },
        (_, None) => PrintPlan {
            program: "xdg-open".into(),
            args: vec![file],
            wait: false,
            message: "Opened in the PDF viewer: print from there".into(),
        },
    }
}

/// The plan for printing `pdf` here, after checking it: an existing `.pdf` file (its absolute
/// path is used) and a printer name that can't pass for an option.
pub fn print_dry_run(pdf: &Path, printer: Option<&str>) -> Result<PrintPlan, PlotError> {
    let bad = |why: &str| PlotError::Print(why.to_string());
    let is_pdf = pdf
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if !is_pdf {
        return Err(bad("Only a PDF can be printed"));
    }
    let pdf = pdf
        .canonicalize()
        .map_err(|_| bad("The PDF to print is not there"))?;
    if !pdf.is_file() {
        return Err(bad("The PDF to print is not there"));
    }
    if let Some(p) = printer {
        if p.is_empty() || p.starts_with('-') || p.chars().any(char::is_control) || p.len() > 256 {
            return Err(bad("That is not a printer name"));
        }
    }
    Ok(print_plan(std::env::consts::OS, &pdf, printer))
}

/// Prints `pdf` on `printer` (or the default; on Linux without one, opens the PDF viewer);
/// what happened, for the status bar.
pub fn print_pdf(pdf: &Path, printer: Option<&str>) -> Result<String, PlotError> {
    let plan = print_dry_run(pdf, printer)?;
    let mut command = Command::new(&plan.program);
    command.args(&plan.args);
    let failed = |e: std::io::Error| PlotError::Print(format!("Can't run {}: {e}", plan.program));
    if plan.wait {
        let out = command.output().map_err(failed)?;
        if !out.status.success() {
            let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(PlotError::Print(if why.is_empty() {
                format!("{} failed", plan.program)
            } else {
                why
            }));
        }
    } else {
        command.spawn().map_err(failed)?;
    }
    Ok(plan.message)
}

/// Printer names from `lpstat -a`: the first word of each line.
pub fn parse_printers(lpstat: &str) -> Vec<String> {
    lpstat
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// The default printer from `lpstat -d`.
pub fn parse_default_printer(lpstat: &str) -> Option<String> {
    lpstat.lines().find_map(|l| {
        let (_, name) = l.split_once("system default destination:")?;
        let name = name.trim();
        (!name.is_empty()).then(|| name.to_string())
    })
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The printers this computer knows; none when it can't tell.
pub fn list_printers() -> Vec<String> {
    if cfg!(windows) {
        run(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-Printer | Select-Object -ExpandProperty Name",
            ],
        )
        .map(|o| {
            o.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
    } else {
        run("lpstat", &["-a"])
            .map(|o| parse_printers(&o))
            .unwrap_or_default()
    }
}

/// The default printer, when there is one.
pub fn default_printer() -> Option<String> {
    if cfg!(windows) {
        run(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-CimInstance Win32_Printer -Filter 'Default=true').Name",
            ],
        )
        .map(|o| o.trim().to_string())
        .filter(|n| !n.is_empty())
    } else {
        run("lpstat", &["-d"]).and_then(|o| parse_default_printer(&o))
    }
}
