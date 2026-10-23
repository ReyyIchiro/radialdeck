//! Native Rust image conversion using the `image` crate.

use std::fs::File;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageFormat, ImageReader};

use super::output::unique_path;
use super::ConvertError;

pub const NATIVE: [&str; 11] = [
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "ico", "qoi", "tga", "pnm",
];

pub fn is_native(extension: &str) -> bool {
    NATIVE.contains(&extension)
}

pub fn convert(source: &Path, target_extension: &str) -> Result<PathBuf, ConvertError> {
    let image = ImageReader::open(source)
        .map_err(|error| ConvertError(format!("open {}: {error}", source.display())))?
        .with_guessed_format()
        .map_err(|error| ConvertError(format!("read {}: {error}", source.display())))?
        .decode()
        .map_err(|error| ConvertError(format!("decode {}: {error}", source.display())))?;

    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());
    let destination = unique_path(parent, &stem, target_extension);
    write(&image, &destination, target_extension)?;
    Ok(destination)
}

fn write(image: &DynamicImage, destination: &Path, extension: &str) -> Result<(), ConvertError> {
    let file = File::create(destination)
        .map_err(|error| ConvertError(format!("create {}: {error}", destination.display())))?;
    let mut writer = std::io::BufWriter::new(file);

    let format = match extension {
        "jpg" | "jpeg" => {
            let rgb = image.to_rgb8();
            JpegEncoder::new_with_quality(&mut writer, 92)
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|error| ConvertError(format!("encode JPEG: {error}")))?;
            return finish(writer);
        }
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        "gif" => ImageFormat::Gif,
        "bmp" => ImageFormat::Bmp,
        "tiff" => ImageFormat::Tiff,
        "ico" => ImageFormat::Ico,
        "qoi" => ImageFormat::Qoi,
        "tga" => ImageFormat::Tga,
        "pnm" => ImageFormat::Pnm,
        other => return Err(ConvertError(format!("unsupported image target: {other}"))),
    };

    image
        .write_to(&mut writer, format)
        .map_err(|error| ConvertError(format!("encode {extension}: {error}")))?;
    finish(writer)
}

fn finish(writer: std::io::BufWriter<File>) -> Result<(), ConvertError> {
    writer
        .into_inner()
        .map_err(|error| ConvertError(format!("flush output: {error}")))?
        .sync_all()
        .map_err(|error| ConvertError(format!("flush output: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn sample(dir: &Path) -> PathBuf {
        let path = dir.join("sample.png");
        let image = RgbImage::from_pixel(8, 6, Rgb([255, 90, 0]));
        image.save(&path).unwrap();
        path
    }

    #[test]
    fn converts_png_to_webp_and_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = sample(dir.path());

        let webp = convert(&source, "webp").unwrap();
        assert_eq!(webp.extension().unwrap(), "webp");
        let decoded = image::open(&webp).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (8, 6));

        let jpg = convert(&source, "jpg").unwrap();
        assert_eq!(jpg.extension().unwrap(), "jpg");
        assert!(image::open(&jpg).is_ok());
    }

    #[test]
    fn refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let source = sample(dir.path());
        std::fs::write(dir.path().join("sample.webp"), b"existing").unwrap();
        let output = convert(&source, "webp").unwrap();
        assert_eq!(output.file_name().unwrap(), "sample-1.webp");
    }
}
