use std::io::Cursor;

use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, codecs::jpeg::JpegEncoder,
    imageops::FilterType,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::{auth::jwt::AuthUser, db, errors::ApiError, state::AppState};

mod heic;

pub const MAX_UPLOAD_BYTES: usize = 12 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;
const MAX_DECODED_BYTES: u64 = 192 * 1024 * 1024;
// Keep expensive decoding/resizing bounded, including when a client disconnects.
static PROCESSING: Semaphore = Semaphore::const_new(2);

pub async fn upload_photo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_owner(&state, id, auth.user_id).await?;
    let permit = PROCESSING.try_acquire().map_err(|_| {
        ApiError::ServiceUnavailable("Photo processing is busy; please retry shortly".into())
    })?;
    let (thumb, large) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        process_photo(&body)
    })
    .await
    .map_err(|_| ApiError::Internal("Photo processing worker failed".into()))?
    .map_err(ApiError::Validation)?;
    if !db::tasting_photos::save(&state.pool, id, auth.user_id, &thumb, &large).await? {
        return Err(ApiError::NotFound("Tasting not found".into()));
    }
    Ok(Json(serde_json::json!({"has_photo": true})))
}

pub async fn get_photo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, variant)): Path<(Uuid, String)>,
) -> Result<Response, ApiError> {
    let thumbnail = match variant.as_str() {
        "thumb" => true,
        "large" => false,
        _ => return Err(ApiError::NotFound("Photo variant not found".into())),
    };
    let bytes = db::tasting_photos::get(&state.pool, id, auth.user_id, thumbnail)
        .await?
        .ok_or_else(|| ApiError::NotFound("Photo not found".into()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "private, no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}

pub async fn delete_photo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_owner(&state, id, auth.user_id).await?;
    db::tasting_photos::delete(&state.pool, id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn require_owner(state: &AppState, id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    if !db::tasting_photos::is_owner(&state.pool, id, user_id).await? {
        return Err(ApiError::NotFound("Tasting not found".into()));
    }
    Ok(())
}

fn process_photo(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if bytes.is_empty() || bytes.len() > MAX_UPLOAD_BYTES {
        return Err("Photo must contain between 1 byte and 12 MiB".into());
    }
    let image = if bytes.get(4..8) == Some(b"ftyp") {
        heic::decode(bytes)?
    } else {
        decode_standard(bytes)?
    };
    let thumb = encode_variant(&image, 320, 640, 80)?;
    let large = encode_variant(&image, 960, 1920, 85)?;
    if thumb.len() > 2 * 1024 * 1024 || large.len() > 8 * 1024 * 1024 {
        return Err("Encoded photo exceeds storage limits".into());
    }
    Ok((thumb, large))
}

fn decode_standard(bytes: &[u8]) -> Result<DynamicImage, String> {
    let format =
        image::guess_format(bytes).map_err(|_| "Upload a JPEG, PNG, WebP or HEIC photo")?;
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
    ) {
        return Err("Upload a JPEG, PNG, WebP or HEIC photo".into());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_PIXELS as u32);
    limits.max_image_height = Some(MAX_PIXELS as u32);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| "Invalid or oversized photo")?;
    let (width, height) = decoder.dimensions();
    // Check the actual header before allocating or decoding the pixel buffer.
    if width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > MAX_PIXELS
        || decoder.total_bytes() > MAX_DECODED_BYTES
    {
        return Err("Photo must be at most 40 megapixels and fit the decoded memory limit".into());
    }
    let orientation = decoder
        .orientation()
        .map_err(|_| "Invalid photo orientation")?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| "Invalid photo data")?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn variant_dimensions(width: u32, height: u32, short: u32, long: u32) -> (u32, u32) {
    let scale = (f64::from(short) / f64::from(width.min(height)))
        .min(f64::from(long) / f64::from(width.max(height)))
        .min(1.0);
    (
        (f64::from(width) * scale).floor().max(1.0) as u32,
        (f64::from(height) * scale).floor().max(1.0) as u32,
    )
}

fn encode_variant(
    image: &DynamicImage,
    short: u32,
    long: u32,
    quality: u8,
) -> Result<Vec<u8>, String> {
    let (width, height) = variant_dimensions(image.width(), image.height(), short, long);
    let resized = image.resize_exact(width, height, FilterType::Lanczos3);
    // Flatten transparency onto white. A fresh encoder receives only pixels:
    // no EXIF, location, comments or other source metadata can be copied.
    let mut rgb = resized.to_rgb8();
    if resized.color().has_alpha() {
        for (dest, source) in rgb.pixels_mut().zip(resized.to_rgba8().pixels()) {
            let alpha = u16::from(source[3]);
            for channel in 0..3 {
                dest[channel] =
                    ((u16::from(source[channel]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
        }
    }
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, quality)
        .encode_image(&rgb)
        .map_err(|_| "Photo could not be encoded".to_owned())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_preserve_aspect_ratio_with_caps_and_no_upscale() {
        assert_eq!(variant_dimensions(4000, 3000, 320, 640), (426, 320));
        assert_eq!(variant_dimensions(3000, 4000, 960, 1920), (960, 1280));
        assert_eq!(variant_dimensions(8000, 1000, 320, 640), (640, 80));
        assert_eq!(variant_dimensions(100, 80, 960, 1920), (100, 80));
    }

    #[test]
    fn unsupported_and_empty_uploads_fail() {
        assert!(process_photo(&[]).is_err());
        assert!(process_photo(b"not a photo").is_err());
        assert!(process_photo(b"GIF89a").is_err());
        assert!(process_photo(&vec![0; MAX_UPLOAD_BYTES + 1]).is_err());
    }

    #[test]
    fn accepted_formats_produce_two_small_jpegs() {
        for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let original = DynamicImage::new_rgb8(32, 24);
            let mut input = Cursor::new(Vec::new());
            original.write_to(&mut input, format).unwrap();
            let (thumb, large) = process_photo(input.get_ref()).unwrap();
            for bytes in [thumb, large] {
                assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::Jpeg);
                let decoded = image::load_from_memory(&bytes).unwrap();
                assert_eq!((decoded.width(), decoded.height()), (32, 24));
            }
        }
    }

    #[test]
    fn exif_orientation_is_applied_and_metadata_removed() {
        let mut original = Cursor::new(Vec::new());
        DynamicImage::new_rgb8(32, 24)
            .write_to(&mut original, ImageFormat::Jpeg)
            .unwrap();
        // Minimal TIFF IFD in a JPEG APP1 segment: orientation 6 (rotate 90).
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut input = vec![0xff, 0xd8, 0xff, 0xe1];
        input.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        input.extend_from_slice(exif);
        input.extend_from_slice(&original.get_ref()[2..]);
        let (thumb, large) = process_photo(&input).unwrap();
        for bytes in [thumb, large] {
            let decoded = image::load_from_memory(&bytes).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (24, 32));
            let mut decoder = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg)
                .into_decoder()
                .unwrap();
            assert!(decoder.exif_metadata().unwrap().is_none());
        }
    }

    #[test]
    fn oversized_pixel_header_is_rejected_before_decode() {
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::new_rgb8(32, 24)
            .write_to(&mut encoded, ImageFormat::Jpeg)
            .unwrap();
        let mut bytes = encoded.into_inner();
        let sof = bytes
            .windows(2)
            .position(|marker| marker == [0xff, 0xc0])
            .unwrap();
        bytes[sof + 5..sof + 7].copy_from_slice(&10000_u16.to_be_bytes());
        bytes[sof + 7..sof + 9].copy_from_slice(&10000_u16.to_be_bytes());
        let error = process_photo(&bytes).unwrap_err();
        assert!(error.contains("40 megapixels"), "{error}");
    }
}
