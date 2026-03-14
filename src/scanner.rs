use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use walkdir::WalkDir;

use crate::audio::{self, ExtensionKind};
use crate::model::{ErrorRecord, ProgressSnapshot, ScanOptions, ScanResult};

const PROGRESS_INTERVAL: usize = 250;

#[derive(Debug, Clone)]
pub enum ScanEvent {
    Started,
    Progress(ProgressSnapshot),
    Error(ErrorRecord),
    UnsupportedAudio { extension: String, path: PathBuf },
    Finished(ProgressSnapshot),
}

pub fn scan_library<F>(root: &Path, options: &ScanOptions, mut on_event: F) -> Result<ScanResult>
where
    F: FnMut(ScanEvent),
{
    let mut result = ScanResult::default();
    let modified_since = options
        .modified_within_days
        .map(|days| SystemTime::now() - Duration::from_secs(days.saturating_mul(24 * 60 * 60)));

    on_event(ScanEvent::Started);

    for entry in WalkDir::new(root).follow_links(false) {
        match entry {
            Ok(entry) => {
                let path = entry.path();

                if entry.file_type().is_symlink() || entry.file_type().is_dir() {
                    continue;
                }

                if !entry.file_type().is_file() || audio::is_silent_junk_file(path) {
                    continue;
                }

                if let Some(modified_since) = modified_since {
                    if !passes_modified_filter(path, modified_since) {
                        continue;
                    }
                }

                match audio::classify_extension(path) {
                    ExtensionKind::SupportedAudio => match audio::read_song_record(path) {
                        Ok(song) => {
                            result.summary.processed_audio += 1;
                            result.summary.scanned_supported += 1;
                            increment_extension_count(&mut result.supported_format_counts, path);
                            if !song.has_front_cover {
                                result.summary.missing_front_cover += 1;
                            }
                            result.scanned_files.push(song);
                            maybe_emit_progress(&result, &mut on_event);
                        }
                        Err(error) => {
                            result.summary.processed_audio += 1;
                            result.summary.errors += 1;
                            let record = ErrorRecord {
                                path: path.to_path_buf(),
                                album: fallback_album_name(path),
                                message: format!("{error:#}"),
                            };
                            result.errors.push(record.clone());
                            on_event(ScanEvent::Error(record));
                            maybe_emit_progress(&result, &mut on_event);
                        }
                    },
                    ExtensionKind::UnsupportedAudio => {
                        result.summary.processed_audio += 1;
                        result.summary.skipped_unsupported += 1;
                        let extension = audio::normalized_extension(path)
                            .unwrap_or_else(|| String::from("[no extension]"));
                        increment_extension_count(&mut result.skipped_format_counts, path);
                        on_event(ScanEvent::UnsupportedAudio {
                            extension,
                            path: path.to_path_buf(),
                        });
                        maybe_emit_progress(&result, &mut on_event);
                    }
                    ExtensionKind::Ignored => {}
                }
            }
            Err(error) => {
                result.summary.errors += 1;
                let path = error
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.to_path_buf());

                let record = ErrorRecord {
                    path,
                    album: String::from("[Unknown Album]"),
                    message: error.to_string(),
                };
                result.errors.push(record.clone());
                on_event(ScanEvent::Error(record));
            }
        }
    }

    on_event(ScanEvent::Finished(progress_snapshot(&result)));

    Ok(result)
}

fn passes_modified_filter(path: &Path, modified_since: SystemTime) -> bool {
    let Ok(metadata) = path.metadata() else {
        return true;
    };

    let Ok(modified_at) = metadata.modified() else {
        return true;
    };

    modified_at >= modified_since
}

fn increment_extension_count(counts: &mut std::collections::BTreeMap<String, usize>, path: &Path) {
    let key = audio::normalized_extension(path).unwrap_or_else(|| String::from("[no extension]"));
    *counts.entry(key).or_default() += 1;
}

fn fallback_album_name(path: &Path) -> String {
    path.parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("[Unknown Album]"))
}

fn maybe_emit_progress<F>(result: &ScanResult, on_event: &mut F)
where
    F: FnMut(ScanEvent),
{
    let processed = result.summary.processed_audio;

    if processed == 1 || processed.is_multiple_of(PROGRESS_INTERVAL) {
        on_event(ScanEvent::Progress(progress_snapshot(result)));
    }
}

fn progress_snapshot(result: &ScanResult) -> ProgressSnapshot {
    ProgressSnapshot {
        summary: result.summary,
        supported_format_counts: result.supported_format_counts.clone(),
        skipped_format_counts: result.skipped_format_counts.clone(),
    }
}
