use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::config::OutputFormat;

#[derive(Debug, Clone)]
pub struct SongRecord {
    pub album: String,
    pub artist: String,
    pub parent_directory: PathBuf,
    pub has_front_cover: bool,
    pub has_external_cover_hint: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorRecord {
    pub path: PathBuf,
    pub album: String,
    pub message: String,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct ScanSummary {
    pub processed_audio: usize,
    pub scanned_supported: usize,
    pub missing_front_cover: usize,
    pub errors: usize,
    pub skipped_unsupported: usize,
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub scanned_files: Vec<SongRecord>,
    pub errors: Vec<ErrorRecord>,
    pub summary: ScanSummary,
    pub supported_format_counts: BTreeMap<String, usize>,
    pub skipped_format_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub modified_within_days: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub scan_root: PathBuf,
    pub output_dir: PathBuf,
    pub output_path: PathBuf,
    pub error_path: PathBuf,
    pub output_format: OutputFormat,
    pub modified_within_days: Option<u64>,
    pub config_path: Option<PathBuf>,
    pub plain: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlbumReportRow {
    pub artist: String,
    pub album: String,
    pub folder: PathBuf,
    pub external_cover_hint: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportMeta {
    pub scan_root: PathBuf,
    pub output_format: String,
    pub modified_filter: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SummarySnapshot {
    pub processed_audio: usize,
    pub supported_files_scanned: usize,
    pub supported_format_counts: BTreeMap<String, usize>,
    pub missing_front_cover_songs: usize,
    pub missing_front_cover_albums: usize,
    pub errors: usize,
    pub unsupported_audio_skipped: usize,
    pub unsupported_format_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    pub summary: ScanSummary,
    pub supported_format_counts: BTreeMap<String, usize>,
    pub skipped_format_counts: BTreeMap<String, usize>,
}
