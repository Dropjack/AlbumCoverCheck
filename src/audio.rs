use std::path::Path;

use anyhow::{Context, Result, anyhow};
use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use mp4ameta::Tag as Mp4Tag;

use crate::model::SongRecord;

const UNKNOWN_ALBUM: &str = "[Unknown Album]";
const UNKNOWN_ARTIST: &str = "[Unknown Artist]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupportedFormat {
    FrontCoverTagged,
    Mp4Like,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionKind {
    SupportedAudio,
    UnsupportedAudio,
    Ignored,
}

pub fn is_silent_junk_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if file_name.starts_with("._") {
        return true;
    }

    matches!(
        file_name.to_ascii_lowercase().as_str(),
        ".ds_store" | "desktop.ini" | "thumbs.db"
    )
}

pub fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

pub fn has_external_cover_hint(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };

    const EXTERNAL_COVER_FILE_NAMES: &[&str] = &[
        "cover.jpg",
        "cover.jpeg",
        "cover.png",
        "folder.jpg",
        "folder.jpeg",
        "folder.png",
        "front.jpg",
        "front.jpeg",
        "front.png",
    ];

    EXTERNAL_COVER_FILE_NAMES
        .iter()
        .any(|file_name| parent.join(file_name).is_file())
}

pub fn read_song_record(path: &Path) -> Result<SongRecord> {
    match supported_format(path) {
        Some(SupportedFormat::Mp4Like) => read_mp4_like_song_record(path),
        Some(SupportedFormat::FrontCoverTagged) => read_standard_song_record(path),
        None => Err(anyhow!(
            "unsupported format for metadata read: {}",
            path.display()
        )),
    }
}

fn read_standard_song_record(path: &Path) -> Result<SongRecord> {
    let tagged_file = Probe::open(path)
        .with_context(|| format!("failed to open file {}", path.display()))?
        .read()
        .with_context(|| format!("failed to read metadata from {}", path.display()))?;

    let best_tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let album = best_tag
        .and_then(|tag| tag.album().map(|value| value.into_owned()))
        .filter(|value: &String| !value.trim().is_empty())
        .unwrap_or_else(|| UNKNOWN_ALBUM.to_owned());

    let artist = best_tag
        .and_then(|tag| tag.artist().map(|value| value.into_owned()))
        .filter(|value: &String| !value.trim().is_empty())
        .unwrap_or_else(|| UNKNOWN_ARTIST.to_owned());

    let has_front_cover = tagged_file.tags().iter().any(|tag| {
        tag.pictures()
            .iter()
            .any(|picture| picture.pic_type() == PictureType::CoverFront)
    });

    build_song_record(path, album, artist, has_front_cover)
}

fn read_mp4_like_song_record(path: &Path) -> Result<SongRecord> {
    let tag = Mp4Tag::read_from_path(path)
        .with_context(|| format!("failed to read MP4 metadata from {}", path.display()))?;

    let album = tag
        .album()
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| UNKNOWN_ALBUM.to_owned());

    let artist = tag
        .artist()
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| UNKNOWN_ARTIST.to_owned());

    let has_front_cover = tag.artwork().is_some();

    build_song_record(path, album, artist, has_front_cover)
}

fn build_song_record(
    path: &Path,
    album: String,
    artist: String,
    has_front_cover: bool,
) -> Result<SongRecord> {
    let parent_directory = path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("file has no parent directory: {}", path.display()))?;

    Ok(SongRecord {
        album,
        artist,
        parent_directory,
        has_front_cover,
        has_external_cover_hint: has_external_cover_hint(path),
    })
}

fn supported_format(path: &Path) -> Option<SupportedFormat> {
    match normalized_extension(path).as_deref() {
        Some("mp3" | "flac") => Some(SupportedFormat::FrontCoverTagged),
        Some("m4a" | "mp4") => Some(SupportedFormat::Mp4Like),
        _ => None,
    }
}

pub fn classify_extension(path: &Path) -> ExtensionKind {
    match normalized_extension(path).as_deref() {
        Some("mp3" | "m4a" | "mp4" | "flac") => ExtensionKind::SupportedAudio,
        Some("aac" | "aiff" | "alac" | "ape" | "dsd" | "dsf" | "ogg" | "opus" | "wav" | "wma") => {
            ExtensionKind::UnsupportedAudio
        }
        _ => ExtensionKind::Ignored,
    }
}
