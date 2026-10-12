//! Printing hands the PDF to the operating system; nothing is printed here (dry runs only).

use std::path::Path;

use qiyas_plot::{
    parse_default_printer, parse_printers, plain_path, print_dry_run, print_plan, PlotError,
};

#[test]
fn linux_prints_with_lp_or_opens_the_pdf_viewer() {
    let pdf = Path::new("/home/q/plan-Model.pdf");
    let plan = print_plan("linux", pdf, Some("Office_HP"));
    assert_eq!(plan.program, "lp");
    assert_eq!(plan.args, ["-d", "Office_HP", "/home/q/plan-Model.pdf"]);
    assert!(plan.wait);
    let plan = print_plan("linux", pdf, None);
    assert_eq!(plan.program, "xdg-open");
    assert_eq!(plan.args, ["/home/q/plan-Model.pdf"]);
    assert!(!plan.wait);
    assert!(plan.message.contains("PDF viewer"), "{}", plan.message);
}

#[test]
fn macos_prints_with_lpr() {
    let pdf = Path::new("/Users/q/plan.pdf");
    let plan = print_plan("macos", pdf, Some("HP"));
    assert_eq!(plan.program, "lpr");
    assert_eq!(plan.args, ["-P", "HP", "/Users/q/plan.pdf"]);
    let plan = print_plan("macos", pdf, None);
    assert_eq!(plan.args, ["/Users/q/plan.pdf"]);
}

/// Base64 back to bytes, for reading the encoded PowerShell command.
fn unbase64(s: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {c}"),
    };
    let mut out = Vec::new();
    for chunk in s.trim_end_matches('=').as_bytes().chunks(4) {
        let v: Vec<u32> = chunk.iter().map(|c| u32::from(value(*c))).collect();
        let n = v.iter().fold(0, |acc, x| acc << 6 | x) << (6 * (4 - v.len()));
        out.extend(&n.to_be_bytes()[1..v.len()]);
    }
    out
}

/// Windows prints through the PDF handler's Print or PrintTo verb from a fixed PowerShell
/// script: the file and the printer travel in environment variables, never inside the script,
/// so a name with any quote, `$( )` or `[ ]` stays a name.
#[test]
fn windows_passes_the_file_and_printer_outside_the_script() {
    let hostile = r"C:\Users\q\plan-x’$(calc)’ ‘a‚b‛ 'c' `d` [rev A].pdf";
    let printer = "HP ’$(calc)’ ; & | [x]";
    for printer in [None, Some(printer)] {
        let plan = print_plan("windows", Path::new(hostile), printer);
        assert_eq!(plan.program, "powershell");
        assert_eq!(
            plan.args[..5],
            [
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-EncodedCommand"
            ]
        );
        assert_eq!(plan.args.len(), 6);
        for arg in &plan.args {
            assert!(!arg.contains("calc") && !arg.contains("plan-x"), "{arg}");
        }
        let utf16: Vec<u16> = unbase64(&plan.args[5])
            .chunks(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let script = String::from_utf16(&utf16).unwrap();
        assert!(script.contains("$env:QIYAS_PRINT_FILE"), "{script}");
        assert!(script.contains("UseShellExecute"), "{script}");
        assert!(
            !script.contains("calc") && !script.contains("plan-x"),
            "{script}"
        );
        let env: Vec<(&str, &str)> = plan
            .env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert!(env.contains(&("QIYAS_PRINT_FILE", hostile)), "{env:?}");
        match printer {
            Some(p) => assert!(env.contains(&("QIYAS_PRINT_PRINTER", p)), "{env:?}"),
            None => assert!(env.iter().all(|(k, _)| *k != "QIYAS_PRINT_PRINTER")),
        }
    }
}

/// Windows' verbatim prefix (`\\?\`, from canonicalize) is taken off: the PDF handlers and
/// the window read plain paths.
#[test]
fn verbatim_windows_paths_become_plain() {
    assert_eq!(
        plain_path(Path::new(r"\\?\C:\Users\q\plan.pdf")),
        Path::new(r"C:\Users\q\plan.pdf")
    );
    assert_eq!(
        plain_path(Path::new(r"\\?\UNC\server\share\plan.pdf")),
        Path::new(r"\\server\share\plan.pdf")
    );
    assert_eq!(
        plain_path(Path::new("/home/q/plan.pdf")),
        Path::new("/home/q/plan.pdf")
    );
    // A verbatim path that is not a drive or a share stays as it is.
    assert_eq!(
        plain_path(Path::new(r"\\?\Volume{x}\plan.pdf")),
        Path::new(r"\\?\Volume{x}\plan.pdf")
    );
}

#[test]
fn printer_lists_are_read_from_lpstat() {
    let out = "Office_HP accepting requests since Mon 12 Oct 2026 09:00:00\n\
               PDF accepting requests since Sun 11 Oct 2026\n\n";
    assert_eq!(parse_printers(out), ["Office_HP", "PDF"]);
    assert_eq!(
        parse_default_printer("system default destination: Office_HP\n"),
        Some("Office_HP".to_string())
    );
    assert_eq!(
        parse_default_printer("no system default destination\n"),
        None
    );
}

#[test]
fn only_an_existing_pdf_is_handed_over_and_printer_names_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("plan.pdf");
    std::fs::write(&pdf, b"%PDF-1.5\n").unwrap();
    let plan = print_dry_run(&pdf, Some("Office_HP")).unwrap();
    assert!(plan.args.iter().any(|a| a.contains("plan.pdf")));

    let txt = dir.path().join("plan.txt");
    std::fs::write(&txt, b"x").unwrap();
    for (path, printer) in [
        (dir.path().join("missing.pdf"), None),
        (txt, None),
        (pdf.clone(), Some("-o evil")),
        (pdf.clone(), Some("bad\nname")),
        (pdf.clone(), Some("")),
        (pdf.clone(), Some("say \"hi\"")),
    ] {
        assert!(
            matches!(print_dry_run(&path, printer), Err(PlotError::Print(_))),
            "{path:?} {printer:?}"
        );
    }
}

/// The spool PDF goes as soon as `lp` or `lpr` has handed the print system its copy; the PDF
/// viewer and Windows' handler read it after the command returns, so it stays for them.
#[test]
fn the_spool_pdf_goes_once_the_print_system_has_its_copy() {
    assert!(qiyas_plot::hands_off_a_copy("linux", Some("office")));
    assert!(!qiyas_plot::hands_off_a_copy("linux", None));
    assert!(qiyas_plot::hands_off_a_copy("macos", None));
    assert!(!qiyas_plot::hands_off_a_copy("windows", Some("office")));
}

/// The print spool is the user's own folder: 0700 on Unix (made so again when it was opened
/// up), never a shared `/tmp/qiyas-plot`; spool PDFs a day old go.
#[cfg(unix)]
#[test]
fn the_print_spool_is_a_private_folder_of_the_users() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = qiyas_plot::spool_dir().unwrap();
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("qiyas-plot-"), "{name}");
    let meta = std::fs::symlink_metadata(&dir).unwrap();
    assert!(meta.is_dir());
    assert_eq!(meta.mode() & 0o777, 0o700);

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let old = dir.join(format!("old-{}.pdf", std::process::id()));
    let new = dir.join(format!("new-{}.pdf", std::process::id()));
    for f in [&old, &new] {
        std::fs::write(f, b"%PDF").unwrap();
    }
    let two_days = std::time::Duration::from_secs(2 * 24 * 60 * 60);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - two_days)
        .unwrap();
    assert_eq!(qiyas_plot::spool_dir().unwrap(), dir);
    let mode = std::fs::metadata(&dir).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o700);
    assert!(!old.exists(), "a day old: gone");
    assert!(new.exists());
    std::fs::remove_file(&new).unwrap();
}
