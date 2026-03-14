mod audio;
mod config;
mod model;
mod report;
mod scanner;
mod ui;

use std::collections::BTreeMap;
use std::io::{self, IsTerminal};
use std::time::Instant;

use anyhow::{Result, bail};
use config::resolve_config;
use model::{RunConfig, ScanOptions, ScanSummary};

fn main() {
    if let Err(error) = run() {
        eprintln!("Fatal error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let resolved = resolve_config()?;
    let run_config = RunConfig {
        scan_root: resolved.scan_root,
        output_dir: resolved.output_dir,
        output_path: resolved.output_path,
        error_path: resolved.error_path,
        output_format: resolved.output_format,
        modified_within_days: resolved.modified_within_days,
        config_path: resolved.config_path,
        plain: resolved.plain,
    };

    validate_config(&run_config)?;

    if should_use_tui(&run_config) {
        return ui::run_tui(run_config);
    }

    let started_at = Instant::now();
    print_run_header(&run_config);

    let result = scanner::scan_library(
        &run_config.scan_root,
        &ScanOptions {
            modified_within_days: run_config.modified_within_days,
        },
        |_| {},
    )?;

    println!();
    println!("Writing reports...");

    report::write_reports(
        &run_config.output_path,
        &run_config.error_path,
        run_config.output_format,
        &run_config.scan_root,
        &result.scanned_files,
        &result.errors,
        &result.summary,
        &result.supported_format_counts,
        &result.skipped_format_counts,
        run_config.modified_within_days,
    )?;

    print_summary(
        &result.summary,
        &result.supported_format_counts,
        &result.skipped_format_counts,
        &run_config,
        started_at.elapsed(),
    );

    Ok(())
}

fn validate_config(config: &RunConfig) -> Result<()> {
    if !config.scan_root.exists() {
        bail!("scan root does not exist: {}", config.scan_root.display());
    }

    if !config.scan_root.is_dir() {
        bail!(
            "scan root is not a directory: {}",
            config.scan_root.display()
        );
    }

    std::fs::create_dir_all(&config.output_dir).map_err(anyhow::Error::from)?;

    Ok(())
}

fn should_use_tui(config: &RunConfig) -> bool {
    !config.plain && io::stdout().is_terminal()
}

fn print_run_header(config: &RunConfig) {
    println!("Album Cover Check");
    println!();
    println!("Configuration");
    println!("  Scan root: {}", config.scan_root.display());
    println!("  Output dir: {}", config.output_dir.display());
    println!("  Output format: {}", config.output_format.as_str());
    println!("  Main report: {}", config.output_path.display());
    println!("  Error log: {}", config.error_path.display());
    println!(
        "  Modified filter: {}",
        format_modified_filter(config.modified_within_days)
    );
    println!(
        "  Config file: {}",
        config
            .config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("[none]"))
    );
    println!();
}

fn print_summary(
    summary: &ScanSummary,
    supported_format_counts: &BTreeMap<String, usize>,
    skipped_format_counts: &BTreeMap<String, usize>,
    config: &RunConfig,
    elapsed: std::time::Duration,
) {
    println!();
    println!("Scan Complete");
    println!("  Processed audio: {}", summary.processed_audio);
    println!("  Supported scanned: {}", summary.scanned_supported);
    println!(
        "  Supported formats: {}",
        format_extension_counts(supported_format_counts)
    );
    println!(
        "  Missing embedded Front Cover: {}",
        summary.missing_front_cover
    );
    println!("  Errors: {}", summary.errors);
    println!(
        "  Unsupported audio skipped: {}",
        summary.skipped_unsupported
    );
    if summary.skipped_unsupported > 0 {
        println!(
            "  Unsupported formats: {}",
            format_extension_counts(skipped_format_counts)
        );
    }
    println!("  Elapsed: {:.1}s", elapsed.as_secs_f64());
    println!();
    println!("Outputs");
    println!("  Main report: {}", config.output_path.display());
    println!("  Error log: {}", config.error_path.display());
}

fn format_extension_counts(counts: &BTreeMap<String, usize>) -> String {
    if counts.is_empty() {
        return String::from("[none]");
    }

    counts
        .iter()
        .map(|(extension, count)| format!(".{extension} x{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_modified_filter(days: Option<u64>) -> String {
    match days {
        Some(days) => format!("last {days} days"),
        None => String::from("none"),
    }
}
