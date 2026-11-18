//! File type categories for Browse (Images, Videos, Audio, Documents, Archives, Installers,
//! Code, AI models, Other), by extension.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum FileKind {
    Images,
    Videos,
    Audio,
    Documents,
    Archives,
    Installers,
    Code,
    AiModels,
    Other,
}

impl FileKind {
    pub const ALL: [FileKind; 9] = [
        FileKind::Images,
        FileKind::Videos,
        FileKind::Audio,
        FileKind::Documents,
        FileKind::Archives,
        FileKind::Installers,
        FileKind::Code,
        FileKind::AiModels,
        FileKind::Other,
    ];

    pub fn index(self) -> usize {
        FileKind::ALL.iter().position(|k| *k == self).unwrap_or(8)
    }

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Images => "Images",
            FileKind::Videos => "Videos",
            FileKind::Audio => "Audio",
            FileKind::Documents => "Documents",
            FileKind::Archives => "Archives",
            FileKind::Installers => "Installers",
            FileKind::Code => "Code",
            FileKind::AiModels => "AI models",
            FileKind::Other => "Other",
        }
    }

    pub fn of(name: &std::ffi::OsStr) -> FileKind {
        let n = name.as_encoded_bytes();
        let Some(dot) = n.iter().rposition(|&b| b == b'.') else {
            return FileKind::Other;
        };
        let ext = &n[dot + 1..];
        if ext.len() > 12 {
            return FileKind::Other;
        }
        let mut buf = [0u8; 12];
        for (i, b) in ext.iter().enumerate() {
            buf[i] = b.to_ascii_lowercase();
        }
        match &buf[..ext.len()] {
            b"jpg" | b"jpeg" | b"png" | b"gif" | b"bmp" | b"webp" | b"heic" | b"heif" | b"tif"
            | b"tiff" | b"raw" | b"cr2" | b"nef" | b"arw" | b"dng" | b"psd" | b"svg" | b"ico"
            | b"avif" => FileKind::Images,
            b"mp4" | b"mkv" | b"mov" | b"avi" | b"wmv" | b"webm" | b"m4v" | b"flv" | b"mts"
            | b"m2ts" | b"3gp" => FileKind::Videos,
            b"mp3" | b"wav" | b"flac" | b"aac" | b"ogg" | b"m4a" | b"wma" | b"opus" | b"aiff" => {
                FileKind::Audio
            }
            b"pdf" | b"doc" | b"docx" | b"xls" | b"xlsx" | b"ppt" | b"pptx" | b"odt" | b"ods"
            | b"odp" | b"rtf" | b"txt" | b"md" | b"epub" | b"csv" | b"pages" | b"key"
            | b"numbers" => FileKind::Documents,
            b"zip" | b"7z" | b"rar" | b"tar" | b"gz" | b"tgz" | b"xz" | b"bz2" | b"zst"
            | b"iso" | b"img" | b"cab" => FileKind::Archives,
            b"exe" | b"msi" | b"msix" | b"msixbundle" | b"appx" | b"appxbundle" => {
                FileKind::Installers
            }
            b"rs" | b"js" | b"mjs" | b"cjs" | b"ts" | b"tsx" | b"jsx" | b"py" | b"java" | b"kt"
            | b"c" | b"h" | b"cc" | b"cpp" | b"hpp" | b"cs" | b"go" | b"rb" | b"php" | b"swift"
            | b"dart" | b"lua" | b"sh" | b"ps1" | b"sql" | b"html" | b"css" | b"scss" | b"vue"
            | b"svelte" => FileKind::Code,
            b"safetensors" | b"gguf" | b"ggml" | b"ckpt" | b"pt" | b"pth" | b"onnx" | b"h5"
            | b"tflite" | b"mlmodel" | b"pb" => FileKind::AiModels,
            _ => FileKind::Other,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KindTotal {
    pub files: u64,
    pub bytes: u64,
}

pub type KindTotals = [KindTotal; 9];

pub fn add(t: &mut KindTotals, kind: FileKind, bytes: u64) {
    let k = &mut t[kind.index()];
    k.files += 1;
    k.bytes += bytes;
}

pub fn merge(into: &mut KindTotals, from: &KindTotals) {
    for (a, b) in into.iter_mut().zip(from.iter()) {
        a.files += b.files;
        a.bytes += b.bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(FileKind::of("a.MP4".as_ref()), FileKind::Videos);
        assert_eq!(
            FileKind::of("model.safetensors".as_ref()),
            FileKind::AiModels
        );
        assert_eq!(FileKind::of("setup.exe".as_ref()), FileKind::Installers);
        assert_eq!(FileKind::of("README".as_ref()), FileKind::Other);
        assert_eq!(FileKind::of("photo.final.JPEG".as_ref()), FileKind::Images);
    }
}
