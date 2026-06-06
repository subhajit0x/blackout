//! BLACKOUT CLI — the runnable front-end for the CLEAN module.
//!
//! Two commands:
//!   blackout clean   <files/dirs...>  → writes privacy-safe copies
//!   blackout inspect <files/dirs...>  → shows exposed metadata, writes nothing

use std::path::{Path, PathBuf};

use blackout_core::{clean_file, ffmpeg_available, inspect_file, CleanReport};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "blackout",
    version,
    about = "BLACKOUT · CLEAN — remove hidden metadata from your files, entirely on your device.",
    long_about = "BLACKOUT CLEAN strips GPS coordinates, device fingerprints, author names and other\nhidden metadata from images, audio, documents and PDFs. Nothing leaves your machine:\nno accounts, no network, no telemetry. Originals are never modified — only copies are written."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Strip metadata and write privacy-safe copies.
    Clean {
        /// Files or directories to clean (directories are searched recursively).
        #[arg(required = true)]
        paths: Vec<PathBuf>,

        /// Output directory for cleaned copies.
        #[arg(short, long, default_value = "BLACKOUT-clean")]
        out: PathBuf,

        /// Emit a machine-readable JSON report instead of the human summary.
        #[arg(long)]
        json: bool,
    },

    /// Show what hidden metadata a file is carrying. Writes nothing.
    Inspect {
        /// Files or directories to inspect (directories are searched recursively).
        #[arg(required = true)]
        paths: Vec<PathBuf>,

        /// Emit a machine-readable JSON report.
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Clean { paths, out, json } => run_clean(&paths, &out, json),
        Command::Inspect { paths, json } => run_inspect(&paths, json),
    };
    std::process::exit(code);
}

fn run_clean(paths: &[PathBuf], out: &Path, json: bool) -> i32 {
    let files = collect_files(paths);
    if files.is_empty() {
        eprintln!("No files found to clean.");
        return 1;
    }

    let ffmpeg_ok = ffmpeg_available();
    let reports: Vec<CleanReport> = files
        .iter()
        .map(|f| clean_file(f, out, ffmpeg_ok))
        .collect();

    if json {
        print_json(&reports);
    } else {
        print_clean_human(&reports, out, ffmpeg_ok);
    }
    exit_code(&reports)
}

fn run_inspect(paths: &[PathBuf], json: bool) -> i32 {
    let files = collect_files(paths);
    if files.is_empty() {
        eprintln!("No files found to inspect.");
        return 1;
    }

    let reports: Vec<CleanReport> = files.iter().map(|f| inspect_file(f)).collect();

    if json {
        print_json(&reports);
    } else {
        print_inspect_human(&reports);
    }
    exit_code(&reports)
}

// ---- output helpers ----

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

fn header() {
    println!("{BOLD}BLACKOUT · CLEAN{RESET}  {DIM}One tap. Less exposure.{RESET}");
    println!("{DIM}Everything runs locally — no network, no accounts, no telemetry.{RESET}\n");
}

fn print_clean_human(reports: &[CleanReport], out: &Path, ffmpeg_ok: bool) {
    header();
    let mut cleaned = 0;
    let mut copied = 0;
    let mut skipped = 0;
    let mut errored = 0;

    for r in reports {
        let name = r.source.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        match r.status.as_str() {
            "cleaned" => {
                cleaned += 1;
                let size = size_delta(r);
                println!("{GREEN}✓{RESET} {BOLD}{name}{RESET}  {DIM}({}){RESET}{size}", r.category);
                for item in &r.removed {
                    println!("    {GREEN}removed{RESET} {item}");
                }
            }
            "copied" => {
                copied += 1;
                println!("{GREEN}✓{RESET} {BOLD}{name}{RESET}  {DIM}({}) — no embedded metadata found{RESET}", r.category);
            }
            "unsupported" => {
                skipped += 1;
                println!("{YELLOW}–{RESET} {BOLD}{name}{RESET}  {DIM}({}){RESET}", r.category);
            }
            _ => {
                errored += 1;
                println!("{RED}✗{RESET} {BOLD}{name}{RESET}  {DIM}({}){RESET}", r.category);
            }
        }
        for note in &r.notes {
            println!("    {DIM}note: {note}{RESET}");
        }
    }

    println!();
    println!(
        "{} files · {GREEN}{cleaned} cleaned{RESET} · {copied} copied · {YELLOW}{skipped} skipped{RESET} · {errored} errored",
        reports.len()
    );
    if cleaned > 0 || copied > 0 {
        println!("{DIM}Safe copies written to {}{RESET}", out.display());
    }
    if !ffmpeg_ok && reports.iter().any(|r| r.status == "unsupported") {
        println!("{DIM}Tip: install ffmpeg to clean video/HEIC/M4A → brew install ffmpeg{RESET}");
    }
}

fn print_inspect_human(reports: &[CleanReport]) {
    header();
    for r in reports {
        let name = r.source.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        if r.removed.is_empty() && r.status != "error" && r.status != "unsupported" {
            println!("{GREEN}clean{RESET}  {BOLD}{name}{RESET}  {DIM}({}) — nothing exposed{RESET}", r.category);
        } else if r.status == "error" {
            println!("{RED}error{RESET}  {BOLD}{name}{RESET}");
        } else {
            println!("{YELLOW}exposed{RESET}  {BOLD}{name}{RESET}  {DIM}({}){RESET}", r.category);
            for f in &r.findings {
                let mark = if f.kind == "location" { "📍" } else { "•" };
                println!("    {mark} {BOLD}{}{RESET}: {}", f.label, f.value);
            }
            for item in &r.removed {
                println!("    {DIM}↳ {item}{RESET}");
            }
        }
        for note in &r.notes {
            println!("    {DIM}note: {note}{RESET}");
        }
    }
    println!("\n{DIM}Run `blackout clean` to write safe copies with this metadata removed.{RESET}");
}

fn size_delta(r: &CleanReport) -> String {
    match (r.bytes_before, r.bytes_after) {
        (Some(b), Some(a)) if b >= a => format!("  {DIM}-{} bytes{RESET}", b - a),
        _ => String::new(),
    }
}

fn print_json(reports: &[CleanReport]) {
    match serde_json::to_string_pretty(reports) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("failed to serialize report: {e}"),
    }
}

fn exit_code(reports: &[CleanReport]) -> i32 {
    if reports.iter().any(|r| r.status == "error") {
        2
    } else {
        0
    }
}

/// Expand the given paths into a flat list of files. Directories are walked
/// recursively so a user can point BLACKOUT at a whole folder.
fn collect_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        push_path(p, &mut out);
    }
    out
}

fn push_path(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        if let Ok(entries) = std::fs::read_dir(p) {
            let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            children.sort();
            for c in children {
                push_path(&c, out);
            }
        }
    } else if p.is_file() {
        out.push(p.to_path_buf());
    } else {
        eprintln!("warning: skipping '{}' (not found)", p.display());
    }
}
