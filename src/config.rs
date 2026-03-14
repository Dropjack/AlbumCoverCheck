use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use serde::Deserialize;

const DEFAULT_OUTPUT_DIR: &str = r"D:\";
const DEFAULT_SCAN_ROOT: &str = r"E:\Music";
const CONFIG_FILE_NAME: &str = "album_cover_check.toml";

#[derive(Debug, Clone, Parser)]
    #[command(
    name = "album_cover_check",
    version,
    about = "Scan a music library for embedded Front Cover artwork.",
    long_about = "Scan a music library for embedded Front Cover artwork.\n\nCLI values override album_cover_check.toml when both are present."
)]
pub struct CliArgs {
    #[arg(value_name = "SCAN_ROOT")]
    pub scan_root: Option<PathBuf>,

    #[arg(
        long = "output-dir",
        value_name = "PATH",
        help = "Write reports into this directory using fixed file names"
    )]
    pub output_dir: Option<PathBuf>,

    #[arg(
        long,
        value_name = "FORMAT",
        help = "Choose the main output format",
        long_help = "Choose the main output format. Supported values: text, csv, json."
    )]
    pub format: Option<OutputFormat>,

    #[arg(
        long = "modified-within-days",
        value_name = "DAYS",
        help = "Only scan audio files modified within the last N days"
    )]
    pub modified_within_days: Option<u64>,

    #[arg(
        long,
        value_name = "PATH",
        help = "Load settings from a specific album_cover_check.toml file"
    )]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Use plain text output instead of the terminal UI")]
    pub plain: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            OutputFormat::Text => "text",
            OutputFormat::Csv => "csv",
            OutputFormat::Json => "json",
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    scan_root: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    output_format: Option<OutputFormat>,
    modified_within_days: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub scan_root: PathBuf,
    pub output_dir: PathBuf,
    pub output_path: PathBuf,
    pub error_path: PathBuf,
    pub output_format: OutputFormat,
    pub modified_within_days: Option<u64>,
    pub config_path: Option<PathBuf>,
    pub plain: bool,
}

pub fn resolve_config() -> Result<ResolvedConfig> {
    let cli = CliArgs::parse();
    let config_path = resolve_config_path(cli.config.as_deref())?;
    let file_config = match config_path.as_deref() {
        Some(path) => load_file_config(path)?,
        None => FileConfig::default(),
    };

    let scan_root = cli
        .scan_root
        .or(file_config.scan_root)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SCAN_ROOT));

    let output_format = cli
        .format
        .or(file_config.output_format)
        .unwrap_or(OutputFormat::Text);

    let output_dir = cli
        .output_dir
        .or(file_config.output_dir)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR));

    let (output_path, error_path) = build_report_paths(&output_dir, output_format);

    let modified_within_days = cli
        .modified_within_days
        .or(file_config.modified_within_days);

    Ok(ResolvedConfig {
        scan_root,
        output_dir,
        output_path,
        error_path,
        output_format,
        modified_within_days,
        config_path,
        plain: cli.plain,
    })
}

fn resolve_config_path(explicit_path: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit_path {
        if !path.exists() {
            bail!("config file does not exist: {}", path.display());
        }
        return Ok(Some(path.to_path_buf()));
    }

    let current_dir_path = env::current_dir()
        .context("failed to resolve current working directory")?
        .join(CONFIG_FILE_NAME);
    if current_dir_path.exists() {
        return Ok(Some(current_dir_path));
    }

    let exe_dir_path = env::current_exe()
        .context("failed to resolve current executable path")?
        .parent()
        .map(|parent| parent.join(CONFIG_FILE_NAME));

    Ok(exe_dir_path.filter(|path| path.exists()))
}

fn load_file_config(path: &Path) -> Result<FileConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse config file {}", path.display()))
}

fn build_report_paths(output_dir: &Path, output_format: OutputFormat) -> (PathBuf, PathBuf) {
    let extension = match output_format {
        OutputFormat::Text => "txt",
        OutputFormat::Csv => "csv",
        OutputFormat::Json => "json",
    };

    (
        output_dir.join(format!("cover_checklist.{extension}")),
        output_dir.join(format!("cover_check_errors.{extension}")),
    )
}
