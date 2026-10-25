//! Tangerine-style advanced file tools.
//!
//! The domain layer owns tool discovery, parameter validation and execution.
//! The GTK layer only renders [`Field`] values and sends them back in a
//! [`ToolRequest`]. Each tool is implemented with an external, well-known
//! desktop utility (ImageMagick, ffmpeg, Ghostscript, Poppler, 7-Zip) invoked
//! without a shell.

mod image;
mod media;
mod pdf;
mod shell;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Category;
use shell::{parse_seconds, parse_u32, ToolResult};

pub use shell::ToolResult as ToolExecutionResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolId {
    CreatePdf,
    MergePdf,
    JoinVideos,
    CreateCollage,
    Compress,
    RemoveMetadata,
    CropImage,
    NormalizeAudio,
    MuteVideo,
    SplitPdf,
    ExtractArchive,
    EditImage,
    AnnotateImage,
    FrameImage,
    TrimVideo,
    OrganizePdf,
    CropVideo,
    ChangeVideoSpeed,
    VideoSnapshots,
    SplitVideo,
    AudioToVideo,
    TrimAudio,
    AudioChannels,
    RedactImage,
    RedactVideo,
    RedactAudio,
}

pub const ALL_TOOLS: [ToolId; 26] = [
    ToolId::CreatePdf,
    ToolId::MergePdf,
    ToolId::JoinVideos,
    ToolId::CreateCollage,
    ToolId::Compress,
    ToolId::RemoveMetadata,
    ToolId::CropImage,
    ToolId::NormalizeAudio,
    ToolId::MuteVideo,
    ToolId::SplitPdf,
    ToolId::ExtractArchive,
    ToolId::EditImage,
    ToolId::AnnotateImage,
    ToolId::FrameImage,
    ToolId::TrimVideo,
    ToolId::OrganizePdf,
    ToolId::CropVideo,
    ToolId::ChangeVideoSpeed,
    ToolId::VideoSnapshots,
    ToolId::SplitVideo,
    ToolId::AudioToVideo,
    ToolId::TrimAudio,
    ToolId::AudioChannels,
    ToolId::RedactImage,
    ToolId::RedactVideo,
    ToolId::RedactAudio,
];

impl ToolId {
    pub fn id(self) -> &'static str {
        match self {
            Self::CreatePdf => "create_pdf",
            Self::MergePdf => "merge_pdf",
            Self::JoinVideos => "join_videos",
            Self::CreateCollage => "create_collage",
            Self::Compress => "compress",
            Self::RemoveMetadata => "remove_metadata",
            Self::CropImage => "crop_image",
            Self::NormalizeAudio => "normalize_audio",
            Self::MuteVideo => "mute_video",
            Self::SplitPdf => "split_pdf",
            Self::ExtractArchive => "extract_archive",
            Self::EditImage => "edit_image",
            Self::AnnotateImage => "annotate_image",
            Self::FrameImage => "frame_image",
            Self::TrimVideo => "trim_video",
            Self::OrganizePdf => "organize_pdf",
            Self::CropVideo => "crop_video",
            Self::ChangeVideoSpeed => "change_video_speed",
            Self::VideoSnapshots => "video_snapshots",
            Self::SplitVideo => "split_video",
            Self::AudioToVideo => "audio_to_video",
            Self::TrimAudio => "trim_audio",
            Self::AudioChannels => "audio_channels",
            Self::RedactImage => "redact_image",
            Self::RedactVideo => "redact_video",
            Self::RedactAudio => "redact_audio",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        ALL_TOOLS.into_iter().find(|tool| tool.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CreatePdf => "Create PDF",
            Self::MergePdf => "Merge PDFs",
            Self::JoinVideos => "Join Videos",
            Self::CreateCollage => "Create Collage",
            Self::Compress => "Compress",
            Self::RemoveMetadata => "Remove Metadata",
            Self::CropImage => "Crop Image",
            Self::NormalizeAudio => "Normalize Volume",
            Self::MuteVideo => "Remove Audio",
            Self::SplitPdf => "Split PDF",
            Self::ExtractArchive => "Extract Archive",
            Self::EditImage => "Edit Photo",
            Self::AnnotateImage => "Annotate Photo",
            Self::FrameImage => "Add Background",
            Self::TrimVideo => "Trim Video",
            Self::OrganizePdf => "Organize Pages",
            Self::CropVideo => "Crop Video",
            Self::ChangeVideoSpeed => "Change Speed",
            Self::VideoSnapshots => "Save Frames",
            Self::SplitVideo => "Split Video",
            Self::AudioToVideo => "Audio Visualizer",
            Self::TrimAudio => "Trim Audio",
            Self::AudioChannels => "Audio Channels",
            Self::RedactImage => "Redact Photo",
            Self::RedactVideo => "Redact Video",
            Self::RedactAudio => "Bleep Audio",
        }
    }

    /// Labels that fit inside a narrow radial wedge.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::CreatePdf => "Create PDF",
            Self::MergePdf => "Merge PDFs",
            Self::JoinVideos => "Join Videos",
            Self::CreateCollage => "Collage",
            Self::Compress => "Compress",
            Self::RemoveMetadata => "Clean Tags",
            Self::CropImage => "Crop Photo",
            Self::NormalizeAudio => "Normalize",
            Self::MuteVideo => "Mute Video",
            Self::SplitPdf => "Split PDF",
            Self::ExtractArchive => "Extract",
            Self::EditImage => "Edit Photo",
            Self::AnnotateImage => "Annotate",
            Self::FrameImage => "Add BG",
            Self::TrimVideo => "Trim Video",
            Self::OrganizePdf => "Organize",
            Self::CropVideo => "Crop Video",
            Self::ChangeVideoSpeed => "Speed",
            Self::VideoSnapshots => "Frames",
            Self::SplitVideo => "Split Video",
            Self::AudioToVideo => "Visualize",
            Self::TrimAudio => "Trim Audio",
            Self::AudioChannels => "Channels",
            Self::RedactImage => "Redact Photo",
            Self::RedactVideo => "Redact Video",
            Self::RedactAudio => "Bleep",
        }
    }

    pub fn applies_to(self, category: Category) -> bool {
        self.for_category(category)
    }

    fn for_category(self, category: Category) -> bool {
        category_tools(category).contains(&self)
    }

    /// Tools that are meaningful for a category, in the same order as the
    /// reference app's advanced wheel.
    pub fn available_for(category: Category) -> &'static [ToolId] {
        category_tools(category)
    }

    pub fn requirements(self) -> &'static [&'static str] {
        match self {
            Self::CreatePdf
            | Self::CreateCollage
            | Self::CropImage
            | Self::EditImage
            | Self::AnnotateImage
            | Self::FrameImage
            | Self::RedactImage => &["magick"],
            Self::MergePdf | Self::SplitPdf | Self::OrganizePdf => &["pdfunite", "pdfseparate"],
            Self::Compress
            | Self::RemoveMetadata
            | Self::TrimVideo
            | Self::SplitVideo
            | Self::CropVideo
            | Self::ChangeVideoSpeed
            | Self::MuteVideo
            | Self::VideoSnapshots
            | Self::RedactVideo
            | Self::NormalizeAudio
            | Self::TrimAudio
            | Self::AudioChannels
            | Self::AudioToVideo
            | Self::RedactAudio => &["ffmpeg"],
            Self::JoinVideos => &["ffmpeg"],
            Self::ExtractArchive => &["7z"],
        }
    }
}

fn category_tools(category: Category) -> &'static [ToolId] {
    const IMAGE: &[ToolId] = &[
        ToolId::CreatePdf,
        ToolId::Compress,
        ToolId::RemoveMetadata,
        ToolId::CropImage,
        ToolId::EditImage,
        ToolId::AnnotateImage,
        ToolId::FrameImage,
        ToolId::RedactImage,
        ToolId::CreateCollage,
    ];
    const VIDEO: &[ToolId] = &[
        ToolId::JoinVideos,
        ToolId::Compress,
        ToolId::RemoveMetadata,
        ToolId::TrimVideo,
        ToolId::SplitVideo,
        ToolId::CropVideo,
        ToolId::ChangeVideoSpeed,
        ToolId::MuteVideo,
        ToolId::VideoSnapshots,
        ToolId::RedactVideo,
    ];
    const AUDIO: &[ToolId] = &[
        ToolId::Compress,
        ToolId::RemoveMetadata,
        ToolId::NormalizeAudio,
        ToolId::TrimAudio,
        ToolId::AudioChannels,
        ToolId::AudioToVideo,
        ToolId::RedactAudio,
    ];
    const PDF: &[ToolId] = &[
        ToolId::MergePdf,
        ToolId::Compress,
        ToolId::RemoveMetadata,
        ToolId::SplitPdf,
        ToolId::OrganizePdf,
    ];
    const ARCHIVES: &[ToolId] = &[ToolId::ExtractArchive];

    match category {
        Category::Images => IMAGE,
        Category::Video => VIDEO,
        Category::Audio => AUDIO,
        Category::Pdf => PDF,
        Category::Archives => ARCHIVES,
        Category::Documents
        | Category::Slides
        | Category::Spreadsheets
        | Category::Epub
        | Category::Subtitles => &[],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub value: String,
    pub hint: String,
    pub choices: Vec<String>,
}

impl Field {
    fn text(key: &str, label: &str, value: &str, hint: &str) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: value.into(),
            hint: hint.into(),
            choices: Vec::new(),
        }
    }

    fn choice(key: &str, label: &str, value: &str, choices: &[&str]) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: value.into(),
            hint: String::new(),
            choices: choices.iter().map(|choice| (*choice).to_owned()).collect(),
        }
    }
}

/// Parameters shown before a tool runs. Tools whose defaults are safe and
/// reversible (compress, merge, extract) have no fields and run immediately.
pub fn fields(id: ToolId, category: Category) -> Vec<Field> {
    match id {
        ToolId::Compress => match category {
            Category::Images => vec![
                Field::text("quality", "Quality", "82", "1–100"),
                Field::text(
                    "max_width",
                    "Max width (px)",
                    "2560",
                    "0 keeps the original width",
                ),
            ],
            Category::Video => vec![
                Field::text("crf", "Quality (CRF)", "28", "18 is large, 32 is small"),
                Field::choice(
                    "preset",
                    "Encoder preset",
                    "medium",
                    &["fast", "medium", "slow"],
                ),
            ],
            Category::Audio => vec![Field::text("bitrate", "Bitrate (kbps)", "128", "64–320")],
            Category::Pdf => vec![Field::choice(
                "preset",
                "Preset",
                "ebook",
                &["screen", "ebook", "printer", "prepress"],
            )],
            _ => Vec::new(),
        },
        ToolId::CropImage => vec![
            Field::text("width", "Width", "80%", "Pixels or percent"),
            Field::text("height", "Height", "80%", "Pixels or percent"),
            Field::text("x", "X offset", "center", "Pixels, percent, or center"),
            Field::text("y", "Y offset", "center", "Pixels, percent, or center"),
        ],
        ToolId::EditImage => vec![
            Field::text("brightness", "Brightness", "0", "-100 to 100"),
            Field::text("contrast", "Contrast", "0", "-100 to 100"),
            Field::text("saturation", "Saturation", "0", "-100 to 100"),
            Field::choice("rotate", "Rotate", "0", &["0", "90", "180", "270"]),
            Field::choice("flip", "Flip", "none", &["none", "horizontal", "vertical"]),
        ],
        ToolId::AnnotateImage => vec![
            Field::text("text", "Text", "Confidential", "Annotation text"),
            Field::choice(
                "gravity",
                "Position",
                "South",
                &["North", "Center", "South"],
            ),
            Field::text("size", "Text size", "32", "Points"),
            Field::text("color", "Color", "#ff3b30", "#rrggbb"),
        ],
        ToolId::FrameImage => vec![
            Field::text("color", "Background", "#ffffff", "#rrggbb"),
            Field::text("padding", "Padding (px)", "32", "0 for background only"),
        ],
        ToolId::RedactImage => vec![
            Field::text("x", "X", "30%", "Pixels or percent"),
            Field::text("y", "Y", "35%", "Pixels or percent"),
            Field::text("width", "Width", "40%", "Pixels or percent"),
            Field::text("height", "Height", "30%", "Pixels or percent"),
        ],
        ToolId::CreateCollage => vec![
            Field::text("columns", "Columns", "3", "2–8"),
            Field::text("cell", "Cell size", "480", "Pixels"),
        ],
        ToolId::TrimVideo | ToolId::TrimAudio => vec![
            Field::text("start", "Start", "0", "Seconds or HH:MM:SS"),
            Field::text("end", "End", "", "Blank keeps the rest"),
        ],
        ToolId::SplitVideo => vec![Field::text(
            "at",
            "Split at",
            "00:00:30",
            "Comma-separated timestamps",
        )],
        ToolId::CropVideo => vec![
            Field::text("width", "Width", "80%", "Pixels or percent"),
            Field::text("height", "Height", "80%", "Pixels or percent"),
            Field::text("x", "X", "10%", "Pixels or percent"),
            Field::text("y", "Y", "10%", "Pixels or percent"),
        ],
        ToolId::ChangeVideoSpeed => vec![Field::text("factor", "Speed", "2", "0.25–4×")],
        ToolId::VideoSnapshots => vec![
            Field::text("interval", "Every (seconds)", "5", "0.1–3600"),
            Field::choice("format", "Format", "jpg", &["jpg", "png"]),
        ],
        ToolId::RedactVideo => vec![
            Field::text("x", "X", "30%", "Pixels or percent"),
            Field::text("y", "Y", "35%", "Pixels or percent"),
            Field::text("width", "Width", "40%", "Pixels or percent"),
            Field::text("height", "Height", "30%", "Pixels or percent"),
            Field::text("from", "From", "0", "Seconds"),
            Field::text("to", "To", "", "Blank means the whole clip"),
        ],
        ToolId::SplitPdf => vec![Field::text(
            "ranges",
            "Page ranges",
            "1-3,4-6",
            "Use every for one PDF per page",
        )],
        ToolId::OrganizePdf => vec![Field::text(
            "order",
            "Page order",
            "3,1,2",
            "Comma-separated page numbers",
        )],
        ToolId::NormalizeAudio => {
            vec![Field::text("loudness", "Target (LUFS)", "-16", "-24 to -9")]
        }
        ToolId::AudioChannels => vec![Field::choice(
            "channels",
            "Output",
            "mono",
            &["mono", "stereo"],
        )],
        ToolId::AudioToVideo => vec![
            Field::choice("mode", "Visualizer", "waveform", &["waveform", "spectrum"]),
            Field::choice("size", "Resolution", "1280x720", &["1280x720", "1920x1080"]),
        ],
        ToolId::RedactAudio => vec![
            Field::text("from", "From", "0", "Seconds"),
            Field::text("to", "To", "5", "Seconds"),
        ],
        _ => Vec::new(),
    }
}

/// Returns why a tool cannot run for this selection, or `None` when it can.
pub fn availability(id: ToolId, category: Category, sources: &[PathBuf]) -> Option<String> {
    if !id.for_category(category) {
        return Some("This tool does not apply to the selected files".into());
    }
    if sources.is_empty() {
        return Some("Drop a file first".into());
    }
    if id != ToolId::ExtractArchive && sources.iter().any(|source| source.is_dir()) {
        return Some("Choose a file, not a folder".into());
    }
    let minimum = match id {
        ToolId::MergePdf | ToolId::JoinVideos | ToolId::CreateCollage => 2,
        _ => 1,
    };
    if sources.len() < minimum {
        return Some(match id {
            ToolId::MergePdf => "Select at least two PDFs".into(),
            ToolId::JoinVideos => "Select at least two videos".into(),
            ToolId::CreateCollage => "Select at least two images".into(),
            _ => "Drop a file first".into(),
        });
    }
    let requirements: &[&str] = match (id, category) {
        (ToolId::Compress | ToolId::RemoveMetadata, Category::Images) => &["magick"],
        (ToolId::Compress | ToolId::RemoveMetadata, Category::Pdf) => &["gs"],
        (ToolId::ExtractArchive, _) => &[],
        _ => id.requirements(),
    };
    for tool in requirements {
        if crate::convert::registry::find_tool(tool).is_none() {
            return Some(format!("{tool} is not installed"));
        }
    }
    if id == ToolId::ExtractArchive {
        let archive = sources[0]
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !matches!(
            archive.as_str(),
            "zip" | "tar" | "gz" | "tgz" | "rar" | "7z"
        ) {
            return Some("Choose a ZIP, TAR, GZ, RAR, or 7Z archive".into());
        }
        if crate::convert::registry::find_tool("7z").is_none()
            && crate::convert::registry::find_tool("unrar").is_none()
        {
            return Some("7-Zip or unrar is not installed".into());
        }
    }
    None
}

#[derive(Debug, Clone, Default)]
pub struct ToolRequest {
    pub id: Option<ToolId>,
    pub sources: Vec<PathBuf>,
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ToolFailure {
    pub source: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolOutcome {
    pub created: Vec<PathBuf>,
    pub failures: Vec<ToolFailure>,
    pub message: Option<String>,
}

impl ToolOutcome {
    pub fn success(created: Vec<PathBuf>) -> Self {
        Self {
            created,
            failures: Vec::new(),
            message: None,
        }
    }

    pub fn failure(source: &std::path::Path, message: impl Into<String>) -> Self {
        Self {
            created: Vec::new(),
            failures: vec![ToolFailure {
                source: source.to_path_buf(),
                message: message.into(),
            }],
            message: None,
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.created.extend(other.created);
        self.failures.extend(other.failures);
        if other.message.is_some() {
            self.message = other.message;
        }
    }
}

/// Executes a tool. The caller is expected to have checked `availability` and
/// rendered the fields; this function validates parameters again.
pub fn execute(request: &ToolRequest) -> ToolOutcome {
    let Some(id) = request.id else {
        return ToolOutcome::failure(std::path::Path::new(""), "No tool was selected".to_owned());
    };
    if request.sources.is_empty() {
        return ToolOutcome::failure(
            std::path::Path::new(""),
            "No files were selected".to_owned(),
        );
    }
    let Some(category) = crate::Category::of(&request.sources[0]) else {
        return ToolOutcome::failure(
            &request.sources[0],
            "This file type is not supported by advanced tools".to_owned(),
        );
    };
    execute_for(id, category, &request.sources, &request.values)
}

fn execute_for(
    id: ToolId,
    category: Category,
    sources: &[PathBuf],
    values: &BTreeMap<String, String>,
) -> ToolOutcome {
    match id {
        ToolId::CreatePdf
        | ToolId::CreateCollage
        | ToolId::CropImage
        | ToolId::EditImage
        | ToolId::AnnotateImage
        | ToolId::FrameImage
        | ToolId::RedactImage => image::execute(id, sources, values),
        ToolId::Compress | ToolId::RemoveMetadata if category == Category::Images => {
            image::execute(id, sources, values)
        }
        ToolId::MergePdf
        | ToolId::SplitPdf
        | ToolId::OrganizePdf
        | ToolId::Compress
        | ToolId::RemoveMetadata
            if category == Category::Pdf =>
        {
            pdf::execute(id, sources, values)
        }
        ToolId::ExtractArchive => pdf::extract_archive(sources),
        _ => media::execute(id, category, sources, values),
    }
}

// Parameter readers deliberately give per-field messages so a form can show
// exactly which value needs attention.
fn value<'a>(values: &'a BTreeMap<String, String>, key: &str, fallback: &'a str) -> &'a str {
    values
        .get(key)
        .map(String::as_str)
        .unwrap_or(fallback)
        .trim()
}

fn quality(values: &BTreeMap<String, String>, key: &str, fallback: u32) -> ToolResult<u32> {
    let raw = value(values, key, "");
    if raw.is_empty() {
        return Ok(fallback);
    }
    parse_u32(raw, key)
}

fn seconds(values: &BTreeMap<String, String>, key: &str, fallback: f64) -> ToolResult<f64> {
    let raw = value(values, key, "");
    if raw.is_empty() {
        Ok(fallback)
    } else {
        parse_seconds(raw)
    }
}
