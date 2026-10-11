//! Printing hands the PDF to the operating system; nothing is printed here (dry runs only).

use std::path::Path;

use qiyas_plot::{parse_default_printer, parse_printers, print_dry_run, print_plan, PlotError};

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

#[test]
fn windows_uses_the_pdf_handlers_print_verb() {
    let pdf = Path::new(r"C:\Users\q\o'brien plan.pdf");
    let plan = print_plan("windows", pdf, None);
    assert_eq!(plan.program, "powershell");
    let script = plan.args.last().unwrap();
    assert!(script.contains("-Verb Print"), "{script}");
    assert!(
        script.contains(r"'C:\Users\q\o''brien plan.pdf'"),
        "{script}"
    );
    let plan = print_plan("windows", pdf, Some("Office HP"));
    let script = plan.args.last().unwrap();
    assert!(script.contains("-Verb PrintTo"), "{script}");
    assert!(script.contains("'\"Office HP\"'"), "{script}");
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
    ] {
        assert!(
            matches!(print_dry_run(&path, printer), Err(PlotError::Print(_))),
            "{path:?} {printer:?}"
        );
    }
}
