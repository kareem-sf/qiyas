//! Printing hands the plotted PDF to the operating system: Linux `lp -d <printer>` (or the PDF
//! viewer through `xdg-open` when no printer is chosen), macOS `lpr`, Windows the PDF handler's
//! Print or PrintTo verb. Qiyas draws no print dialog of its own.
//!
//! No name or path is ever put inside a command string: on Windows a fixed PowerShell script
//! (`-EncodedCommand`) reads the file and the printer from environment variables and starts the
//! handler through `ProcessStartInfo` (no wildcards, no quoting); elsewhere they are separate
//! arguments of `lp`, `lpr` or `xdg-open`.

use std::path::{Path, PathBuf};
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
    /// Environment variables for it (Windows: the file and the printer).
    pub env: Vec<(String, String)>,
}

/// The Windows print script: the file and printer come from the environment only.
const WINDOWS_SCRIPT: &str = "$ErrorActionPreference = 'Stop'
$i = New-Object System.Diagnostics.ProcessStartInfo
$i.FileName = $env:QIYAS_PRINT_FILE
$i.UseShellExecute = $true
$i.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
if ($env:QIYAS_PRINT_PRINTER) {
  $i.Verb = 'PrintTo'
  $i.Arguments = '\"' + $env:QIYAS_PRINT_PRINTER + '\"'
} else {
  $i.Verb = 'Print'
}
[void][System.Diagnostics.Process]::Start($i)
";

/// Standard base64.
fn base64(data: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | u32::from(*b) << (16 - 8 * i));
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(char::from(ABC[(n >> (18 - 6 * k) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `path` without Windows' verbatim prefix (`\\?\C:\…`, `\\?\UNC\server\…`) that
/// `canonicalize` gives: PDF handlers, PowerShell and the window read plain paths.
pub fn plain_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

/// How to print `pdf` on `os` (`std::env::consts::OS`), on `printer` or the default one.
pub fn print_plan(os: &str, pdf: &Path, printer: Option<&str>) -> PrintPlan {
    let file = pdf.display().to_string();
    let sent = |p: &str| format!("Sent to {p}");
    match (os, printer) {
        ("windows", printer) => {
            let utf16: Vec<u8> = WINDOWS_SCRIPT
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            let mut env = vec![("QIYAS_PRINT_FILE".to_string(), file)];
            if let Some(p) = printer {
                env.push(("QIYAS_PRINT_PRINTER".to_string(), p.to_string()));
            }
            PrintPlan {
                program: "powershell".into(),
                args: vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-EncodedCommand".into(),
                    base64(&utf16),
                ],
                env,
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
                env: Vec::new(),
                wait: true,
                message: printer.map_or("Sent to the default printer".into(), sent),
            }
        }
        (_, Some(p)) => PrintPlan {
            program: "lp".into(),
            args: vec!["-d".into(), p.into(), file],
            env: Vec::new(),
            wait: true,
            message: sent(p),
        },
        (_, None) => PrintPlan {
            program: "xdg-open".into(),
            args: vec![file],
            env: Vec::new(),
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
        if p.is_empty()
            || p.starts_with('-')
            || p.contains('"')
            || p.chars().any(char::is_control)
            || p.len() > 256
        {
            return Err(bad("That is not a printer name"));
        }
    }
    Ok(print_plan(std::env::consts::OS, &plain_path(&pdf), printer))
}

/// Prints `pdf` on `printer` (or the default; on Linux without one, opens the PDF viewer);
/// what happened, for the status bar.
pub fn print_pdf(pdf: &Path, printer: Option<&str>) -> Result<String, PlotError> {
    let plan = print_dry_run(pdf, printer)?;
    let mut command = Command::new(&plan.program);
    command.args(&plan.args);
    command.envs(plan.env.iter().map(|(k, v)| (k, v)));
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
