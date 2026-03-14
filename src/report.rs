use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use serde::Serialize;

use crate::config::OutputFormat;
use crate::model::{
    AlbumReportRow, ErrorRecord, ReportMeta, ScanSummary, SongRecord, SummarySnapshot,
};

const UTF8_BOM: &[u8; 3] = b"\xEF\xBB\xBF";

pub fn write_reports(
    output_path: &Path,
    error_path: &Path,
    output_format: OutputFormat,
    scan_root: &Path,
    songs: &[SongRecord],
    errors: &[ErrorRecord],
    scan_summary: &ScanSummary,
    supported_format_counts: &BTreeMap<String, usize>,
    skipped_format_counts: &BTreeMap<String, usize>,
    modified_within_days: Option<u64>,
) -> Result<()> {
    let meta = ReportMeta {
        scan_root: scan_root.to_path_buf(),
        output_format: output_format.as_str().to_owned(),
        modified_filter: format_modified_filter(modified_within_days),
        generated_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    let missing_songs: Vec<&SongRecord> =
        songs.iter().filter(|song| !song.has_front_cover).collect();
    let missing_albums = dedupe_missing_albums(&missing_songs);
    let deduped_errors = dedupe_errors(errors);

    let summary = SummarySnapshot {
        processed_audio: scan_summary.processed_audio,
        supported_files_scanned: scan_summary.scanned_supported,
        supported_format_counts: supported_format_counts.clone(),
        missing_front_cover_songs: missing_songs.len(),
        missing_front_cover_albums: missing_albums.len(),
        errors: scan_summary.errors,
        unsupported_audio_skipped: scan_summary.skipped_unsupported,
        unsupported_format_counts: skipped_format_counts.clone(),
    };

    match output_format {
        OutputFormat::Text => {
            write_text_report(output_path, &meta, &summary, &missing_albums)?;
            write_text_error_log(error_path, &meta, &deduped_errors)?;
        }
        OutputFormat::Csv => {
            write_csv_report(output_path, &meta, &summary, &missing_albums)?;
            write_csv_errors(error_path, &deduped_errors)?;
        }
        OutputFormat::Json => {
            write_json_report(output_path, &meta, &summary, &missing_albums)?;
            write_json_errors(error_path, &meta, &deduped_errors)?;
        }
    }

    Ok(())
}

fn write_text_report(
    output_path: &Path,
    meta: &ReportMeta,
    summary: &SummarySnapshot,
    missing_albums: &[AlbumReportRow],
) -> Result<()> {
    let mut writer = create_utf8_writer(output_path)?;
    writeln!(writer, "Album Cover Check 扫描报告")?;
    writeln!(writer, "生成时间：{}", meta.generated_at)?;
    writeln!(writer, "扫描目录：{}", meta.scan_root.display())?;
    writeln!(writer, "输出格式：{}", format_output_format_label(&meta.output_format))?;
    writeln!(writer, "修改时间过滤：{}", format_modified_filter_label(&meta.modified_filter))?;
    writeln!(writer)?;
    writeln!(writer, "=== 摘要 ===")?;
    writeln!(writer, "已处理音频：{}", summary.processed_audio)?;
    writeln!(writer, "已扫描支持格式：{}", summary.supported_files_scanned)?;
    writeln!(
        writer,
        "支持格式分布：{}",
        format_extension_counts(&summary.supported_format_counts)
    )?;
    writeln!(writer, "缺少嵌入式封面的歌曲：{}", summary.missing_front_cover_songs)?;
    writeln!(writer, "涉及专辑数：{}", summary.missing_front_cover_albums)?;
    writeln!(writer, "错误数：{}", summary.errors)?;
    writeln!(writer, "跳过的未支持音频：{}", summary.unsupported_audio_skipped)?;
    writeln!(
        writer,
        "未支持格式分布：{}",
        format_extension_counts(&summary.unsupported_format_counts)
    )?;
    writeln!(writer)?;
    writeln!(writer, "=== 缺少嵌入式封面的专辑 ===")?;
    writeln!(writer, "说明：下方列表按专辑去重显示，不按歌曲逐条展开。")?;
    writeln!(writer)?;
    for row in missing_albums {
        writeln!(
            writer,
            "艺术家：{}\t专辑：{}\t文件夹：{}\t检测到外部封面文件：{}",
            row.artist,
            row.album,
            row.folder.display(),
            yes_no_cn(row.external_cover_hint)
        )?;
    }
    writer.flush()?;
    Ok(())
}

fn write_text_error_log(
    error_path: &Path,
    meta: &ReportMeta,
    errors: &[ErrorRecord],
) -> Result<()> {
    let mut writer = create_utf8_writer(error_path)?;
    writeln!(writer, "Album Cover Check 错误日志")?;
    writeln!(writer, "生成时间：{}", meta.generated_at)?;
    writeln!(writer, "扫描目录：{}", meta.scan_root.display())?;
    writeln!(writer, "修改时间过滤：{}", format_modified_filter_label(&meta.modified_filter))?;
    writeln!(writer, "错误数量：{}", errors.len())?;
    writeln!(writer)?;
    for error in errors {
        writeln!(writer, "路径：{}", error.path.display())?;
        writeln!(writer, "专辑：{}", error.album)?;
        writeln!(writer, "错误：{}", error.message)?;
        writeln!(writer)?;
    }
    writer.flush()?;
    Ok(())
}

fn write_csv_report(
    output_path: &Path,
    meta: &ReportMeta,
    summary: &SummarySnapshot,
    missing_albums: &[AlbumReportRow],
) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .flexible(true)
        .from_path(output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;

    writer.write_record(["分区", "字段", "值"])?;
    write_summary_row(&mut writer, "生成时间", meta.generated_at.as_str())?;
    write_summary_row(
        &mut writer,
        "扫描目录",
        &meta.scan_root.display().to_string(),
    )?;
    write_summary_row(
        &mut writer,
        "输出格式",
        &format_output_format_label(&meta.output_format),
    )?;
    write_summary_row(
        &mut writer,
        "修改时间过滤",
        &format_modified_filter_label(&meta.modified_filter),
    )?;
    write_summary_row(
        &mut writer,
        "已处理音频",
        &summary.processed_audio.to_string(),
    )?;
    write_summary_row(
        &mut writer,
        "已扫描支持格式",
        &summary.supported_files_scanned.to_string(),
    )?;
    write_summary_row(
        &mut writer,
        "支持格式分布",
        &format_extension_counts(&summary.supported_format_counts),
    )?;
    write_summary_row(
        &mut writer,
        "缺少嵌入式封面的歌曲",
        &summary.missing_front_cover_songs.to_string(),
    )?;
    write_summary_row(
        &mut writer,
        "涉及专辑数",
        &summary.missing_front_cover_albums.to_string(),
    )?;
    write_summary_row(&mut writer, "错误数", &summary.errors.to_string())?;
    write_summary_row(
        &mut writer,
        "跳过的未支持音频",
        &summary.unsupported_audio_skipped.to_string(),
    )?;
    write_summary_row(
        &mut writer,
        "未支持格式分布",
        &format_extension_counts(&summary.unsupported_format_counts),
    )?;
    write_summary_row(&mut writer, "说明", "下方列表按专辑去重显示")?;

    writer.write_record([
        "缺封面专辑",
        "艺术家",
        "专辑",
        "文件夹",
        "检测到外部封面文件",
    ])?;
    for row in missing_albums {
        writer.write_record([
            "缺封面专辑",
            row.artist.as_str(),
            row.album.as_str(),
            &row.folder.display().to_string(),
            yes_no_cn(row.external_cover_hint),
        ])?;
    }

    writer.flush()?;
    Ok(())
}

fn write_csv_errors(error_path: &Path, errors: &[ErrorRecord]) -> Result<()> {
    let mut writer = csv::Writer::from_path(error_path)
        .with_context(|| format!("failed to create {}", error_path.display()))?;
    writer.write_record(["路径", "专辑", "错误"])?;
    for error in errors {
        writer.serialize(CsvErrorRow::from(error))?;
    }
    writer.flush()?;
    Ok(())
}

fn write_json_report(
    output_path: &Path,
    meta: &ReportMeta,
    summary: &SummarySnapshot,
    missing_albums: &[AlbumReportRow],
) -> Result<()> {
    let payload = JsonMainReport {
        meta: meta.clone(),
        summary: summary.clone(),
        missing_front_cover_albums: missing_albums.to_vec(),
    };
    write_json_file(output_path, &payload)
}

fn write_json_errors(error_path: &Path, meta: &ReportMeta, errors: &[ErrorRecord]) -> Result<()> {
    let payload = JsonErrorReport {
        meta: meta.clone(),
        errors: errors.to_vec(),
    };
    write_json_file(error_path, &payload)
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, value)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn create_utf8_writer(output_path: &Path) -> Result<BufWriter<File>> {
    let file = File::create(output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    let mut writer = BufWriter::new(file);
    writer.write_all(UTF8_BOM)?;
    Ok(writer)
}

fn dedupe_missing_albums(songs: &[&SongRecord]) -> Vec<AlbumReportRow> {
    let mut unique = BTreeMap::<(PathBuf, String), AlbumReportRow>::new();

    for song in songs {
        let key = (song.parent_directory.clone(), song.album.clone());
        unique.entry(key).or_insert_with(|| AlbumReportRow {
            artist: song.artist.clone(),
            album: song.album.clone(),
            folder: song.parent_directory.clone(),
            external_cover_hint: song.has_external_cover_hint,
        });
    }

    unique.into_values().collect()
}

fn dedupe_errors(errors: &[ErrorRecord]) -> Vec<ErrorRecord> {
    let mut unique = BTreeMap::<(PathBuf, String), ErrorRecord>::new();

    for error in errors {
        let folder = error
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| error.path.clone());
        let key = (folder, error.album.clone());
        unique.entry(key).or_insert_with(|| error.clone());
    }

    unique.into_values().collect()
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

fn yes_no_cn(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

fn format_output_format_label(value: &str) -> String {
    match value {
        "text" => String::from("文本"),
        "csv" => String::from("CSV"),
        "json" => String::from("JSON"),
        other => other.to_owned(),
    }
}

fn format_modified_filter_label(value: &str) -> String {
    if value == "none" {
        return String::from("无");
    }

    value.replace("last ", "最近 ").replace(" days", " 天")
}

#[derive(Serialize)]
struct JsonMainReport {
    meta: ReportMeta,
    summary: SummarySnapshot,
    missing_front_cover_albums: Vec<AlbumReportRow>,
}

#[derive(Serialize)]
struct JsonErrorReport {
    meta: ReportMeta,
    errors: Vec<ErrorRecord>,
}

#[derive(Serialize)]
struct CsvErrorRow<'a> {
    path: String,
    album: &'a str,
    error: &'a str,
}

impl<'a> From<&'a ErrorRecord> for CsvErrorRow<'a> {
    fn from(value: &'a ErrorRecord) -> Self {
        Self {
            path: value.path.display().to_string(),
            album: value.album.as_str(),
            error: value.message.as_str(),
        }
    }
}

fn write_summary_row(writer: &mut csv::Writer<File>, field: &str, value: &str) -> Result<()> {
    writer.write_record(["摘要", field, value])?;
    Ok(())
}
