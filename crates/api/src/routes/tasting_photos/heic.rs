//! Fail-closed HEIC preflight, reviewed against exactly heic-rs 0.1.4.
//! Its max_pixels limit covers the container/output, not HEVC SPS allocations
//! or all retained grid tiles. Check every selected bitstream before decoding.
use heic_rs::{context::Context, props::hvcc::HevcConfig};
use image::{DynamicImage, RgbImage};

use super::{MAX_DECODED_BYTES, MAX_PIXELS};

const INVALID: &str = "Invalid or unsupported HEIC photo; try exporting it as JPEG";
const TOO_LARGE: &str = "HEIC photo exceeds the 40 megapixel or 192 MiB decoded memory limit; some photos above 12 MP need exporting as JPEG";
const MAX_TILES: usize = 256;

pub(super) fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    preflight(bytes).map_err(str::to_owned)?;
    let options = heic_rs::DecodeOptions::default()
        .with_layout(heic_rs::PixelLayout::Rgb8)
        .with_max_pixels(Some(MAX_PIXELS))
        .with_alpha(false)
        .with_transforms(true)
        .with_strict(true)
        .with_threads(Some(1));
    let decoded = heic_rs::decode(bytes, &options).map_err(|_| INVALID)?;
    let rgb = RgbImage::from_raw(decoded.width, decoded.height, decoded.data).ok_or(INVALID)?;
    Ok(DynamicImage::ImageRgb8(rgb))
}

fn preflight(bytes: &[u8]) -> Result<(), &'static str> {
    let ctx = Context::open(bytes).map_err(|_| INVALID)?;
    // Bound parser bookkeeping and reject unusual container complexity.
    if ctx.meta.items.len() > 512 {
        return Err(INVALID);
    }
    let id = ctx.meta.primary;
    let primary = ctx.props(id).map_err(|_| INVALID)?;
    if primary.transforms.len() > 3 || ctx.alpha_item(id).map_err(|_| INVALID)?.is_some() {
        return Err(INVALID);
    }
    let (width, height) = ctx.coded_size(id, &primary).map_err(|_| INVALID)?;
    let output_pixels = pixels(width, height)?;
    let (tw, th) = heic_rs::transform::transformed_size(width, height, &primary.transforms)
        .map_err(|_| INVALID)?;
    pixels(tw, th)?;
    let items = match ctx.grid(id).map_err(|_| INVALID)? {
        Some((grid, tiles)) => {
            pixels(grid.output_width, grid.output_height)?;
            if (grid.output_width, grid.output_height) != (width, height) {
                return Err(INVALID);
            }
            if tiles.is_empty() || tiles.len() > MAX_TILES {
                return Err(INVALID);
            }
            tiles
        }
        None => vec![id],
    };
    let mut retained_pixels = 0u64;
    let mut largest_pixels = 0u64;
    let mut compressed_bytes = 0u64;
    let mut nal_count = 0usize;
    for item in items {
        let props = ctx.props(item).map_err(|_| INVALID)?;
        if ctx.meta.props.unknown_essential(item).is_some() {
            return Err(INVALID);
        }
        let config = props.hvcc.as_ref().ok_or(INVALID)?;
        let coded_pixels = check_config(config)?;
        // A contradictory ispe must not make the output budget smaller than
        // the actual frame. Parse the visible size only after the uncropped
        // SPS has passed our allocation guard.
        let frame_info = heic_rs::hevc::probe(&config.parameter_sets()).map_err(|_| INVALID)?;
        if let Some(ispe) = props.ispe {
            if (ispe.width, ispe.height) != (frame_info.width, frame_info.height) {
                return Err(INVALID);
            }
        }
        retained_pixels = add_tile(retained_pixels, coded_pixels)?;
        largest_pixels = largest_pixels.max(coded_pixels);
        // Repeated references count repeatedly; source extents are not trusted
        // as a proxy for the allocations of each independently decoded tile.
        let data = ctx.item_data(item).map_err(|_| INVALID)?;
        compressed_bytes = compressed_bytes
            .checked_add(data.len() as u64)
            .ok_or(TOO_LARGE)?;
        if compressed_bytes > super::MAX_UPLOAD_BYTES as u64 {
            return Err(TOO_LARGE);
        }
        nal_count = nal_count
            .checked_add(check_payload(&data, config.length_size)?)
            .ok_or(INVALID)?;
        if nal_count > 4096 {
            return Err(INVALID);
        }
    }
    // Conservative buffer budget: retained u16 Y/Cb/Cr frames (6 bytes/pixel,
    // even though accepted files use 4:2:0), one tile's reconstruction/filter/
    // geometry scratch, two RGB buffers during orientation, compressed/RBSP
    // bookkeeping, and 16 MiB headroom. This intentionally refuses some large
    // HEICs below 40 MP rather than relying on the output-only decoder limit.
    let budget = retained_pixels * 6
        + largest_pixels * 24
        + output_pixels * 6
        + compressed_bytes * 8
        + bytes.len() as u64 * 2
        + 16 * 1024 * 1024;
    if budget > MAX_DECODED_BYTES {
        return Err(TOO_LARGE);
    }
    Ok(())
}

fn check_payload(data: &[u8], length_size: u8) -> Result<usize, &'static str> {
    if !(1..=4).contains(&length_size) {
        return Err(INVALID);
    }
    let mut offset = 0usize;
    let mut count = 0usize;
    while offset < data.len() {
        let prefix = data
            .get(offset..offset + usize::from(length_size))
            .ok_or(INVALID)?;
        let length = prefix
            .iter()
            .fold(0usize, |value, byte| (value << 8) | usize::from(*byte));
        offset += usize::from(length_size);
        if length < 2 {
            return Err(INVALID);
        }
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= data.len())
            .ok_or(INVALID)?;
        let nal_type = (data[offset] >> 1) & 0x3f;
        // heic-rs 0.1.4 currently ignores non-VCL payload units and takes
        // parameter sets only from hvcC. Explicitly reject in-band replacements
        // as well, keeping preflight valid if that decoder behavior changes.
        if matches!(nal_type, 32..=34) {
            return Err(INVALID);
        }
        count += 1;
        if count > 256 {
            return Err(INVALID);
        }
        offset = end;
    }
    if count == 0 {
        return Err(INVALID);
    }
    Ok(count)
}

fn add_tile(retained: u64, next: u64) -> Result<u64, &'static str> {
    retained
        .checked_add(next)
        .filter(|sum| *sum <= MAX_PIXELS)
        .ok_or(TOO_LARGE)
}

fn pixels(width: u32, height: u32) -> Result<u64, &'static str> {
    let value = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || value > MAX_PIXELS {
        return Err(TOO_LARGE);
    }
    Ok(value)
}

fn check_config(config: &HevcConfig<'_>) -> Result<u64, &'static str> {
    // One SPS/PPS/VPS avoids alternate-set selection and keeps the reviewed
    // allocation bounds independent of which PPS a slice chooses.
    let sets = config.parameter_sets();
    if sets.len() != 3 {
        return Err(INVALID);
    }
    for (nal, expected) in sets.iter().zip([32, 33, 34]) {
        if nal.len() < 2 || (nal[0] >> 1) & 0x3f != expected || nal.len() > 4096 {
            return Err(INVALID);
        }
    }
    sps_pixels(sets[1])
}

// Parse only the fixed SPS prefix needed to bound the *uncropped* allocations.
// heic_rs::hevc::probe returns cropped dimensions and cannot enforce this.
fn sps_pixels(nal: &[u8]) -> Result<u64, &'static str> {
    let mut rbsp = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &byte in nal.get(2..).ok_or(INVALID)? {
        if zeros >= 2 && byte == 3 {
            zeros = 0;
            continue;
        }
        rbsp.push(byte);
        zeros = if byte == 0 { zeros + 1 } else { 0 };
    }
    let mut bits = Bits {
        bytes: &rbsp,
        offset: 0,
    };
    bits.read(4)?;
    let layers = bits.read(3)? as usize;
    if layers > 6 {
        return Err(INVALID);
    }
    bits.read(1)?;
    bits.skip(96)?; // general profile/tier/compatibility/constraints/level
    let mut present = [(false, false); 6];
    for entry in present.iter_mut().take(layers) {
        *entry = (bits.read(1)? != 0, bits.read(1)? != 0);
    }
    if layers > 0 {
        bits.skip((8 - layers) * 2)?;
    }
    for (profile, level) in present.into_iter().take(layers) {
        if profile {
            bits.skip(88)?;
        }
        if level {
            bits.skip(8)?;
        }
    }
    bits.ue()?; // SPS id
    if bits.ue()? != 1 {
        return Err(INVALID);
    } // reviewed subset: 4:2:0
    let width = bits.ue()?;
    let height = bits.ue()?;
    let size = pixels(width, height)?;
    if width > 65535 || height > 65535 {
        return Err(INVALID);
    }
    if bits.read(1)? != 0 {
        for _ in 0..4 {
            bits.ue()?;
        } // conformance crop cannot hide coded size
    }
    if bits.ue()? > 2 || bits.ue()? > 2 {
        return Err(INVALID);
    } // 8/9/10 bit
    Ok(size)
}

struct Bits<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Bits<'_> {
    fn read(&mut self, count: usize) -> Result<u32, &'static str> {
        if count > 32 || self.offset + count > self.bytes.len() * 8 {
            return Err(INVALID);
        }
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1)
                | u32::from((self.bytes[self.offset / 8] >> (7 - self.offset % 8)) & 1);
            self.offset += 1;
        }
        Ok(value)
    }
    fn skip(&mut self, count: usize) -> Result<(), &'static str> {
        if self.offset + count > self.bytes.len() * 8 {
            return Err(INVALID);
        }
        self.offset += count;
        Ok(())
    }
    fn ue(&mut self) -> Result<u32, &'static str> {
        let mut zeros = 0;
        while self.read(1)? == 0 {
            zeros += 1;
            if zeros > 30 {
                return Err(INVALID);
            }
        }
        Ok((1u32 << zeros) - 1 + self.read(zeros)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID: &[u8] = include_bytes!("../../../tests/fixtures/heic/checker-1024.heic");

    #[test]
    fn apple_encoded_grid_decodes_and_orientation_is_applied_once() {
        let original = decode(GRID).unwrap();
        assert_eq!((original.width(), original.height()), (1024, 1024));
        assert!(original.to_rgb8().pixels().any(|p| p[0] > 240));
        assert!(original.to_rgb8().pixels().any(|p| p[0] < 16));
        let mut rotated = GRID.to_vec();
        let irot = rotated
            .windows(4)
            .position(|value| value == b"irot")
            .unwrap();
        rotated[irot + 4] = 1; // HEIF counter-clockwise 90 degrees
        let decoded = decode(&rotated).unwrap();
        assert_eq!(decoded.to_rgb8(), original.rotate270().to_rgb8());
        let (thumb, large) = super::super::process_photo(&rotated).unwrap();
        assert_eq!(
            image::guess_format(&thumb).unwrap(),
            image::ImageFormat::Jpeg
        );
        assert_eq!(
            image::guess_format(&large).unwrap(),
            image::ImageFormat::Jpeg
        );
    }

    #[test]
    fn grid_tiles_are_counted_even_when_the_same_tile_is_referenced_again() {
        let retained = add_tile(0, 30_000_000).unwrap();
        assert!(add_tile(retained, 30_000_000).is_err());
        assert!(add_tile(u64::MAX, 1).is_err());
    }

    #[test]
    fn in_band_parameter_sets_and_unbounded_nal_lists_are_refused() {
        for nal_type in [32, 33, 34] {
            assert!(check_payload(&[2, nal_type << 1, 1], 1).is_err());
        }
        assert_eq!(check_payload(&[2, 19 << 1, 1], 1), Ok(1));
        assert!(check_payload(&[2, 19 << 1], 1).is_err());
        assert!(check_payload(&[2, 19 << 1, 1], 0).is_err());
        assert!(check_payload(&[2, 19 << 1, 1].repeat(257), 1).is_err());
    }

    #[test]
    fn container_dimensions_cannot_understate_grid_or_frame_size() {
        let properties: Vec<_> = GRID
            .windows(4)
            .enumerate()
            .filter_map(|(offset, value)| (value == b"ispe").then_some(offset))
            .collect();
        assert!(properties.len() >= 2);
        for offset in [properties[0], *properties.last().unwrap()] {
            let mut bytes = GRID.to_vec();
            bytes[offset + 8..offset + 12].copy_from_slice(&1u32.to_be_bytes());
            assert_eq!(preflight(&bytes), Err(INVALID));
        }
    }

    #[test]
    fn uncropped_sps_dimensions_cannot_hide_behind_a_small_crop() {
        // A malicious stream could declare a small container/crop around a
        // 100 MP decoded plane. Refuse the coded size before reading the crop.
        let nal = synthetic_sps(10000, 10000, true);
        assert_eq!(sps_pixels(&nal), Err(TOO_LARGE));
        assert_eq!(sps_pixels(&synthetic_sps(512, 512, false)), Ok(512 * 512));
    }

    fn synthetic_sps(width: u32, height: u32, crop: bool) -> Vec<u8> {
        fn ue(bits: &mut Vec<bool>, value: u32) {
            let encoded = value + 1;
            let len = 32 - encoded.leading_zeros();
            bits.extend(std::iter::repeat_n(false, (len - 1) as usize));
            for shift in (0..len).rev() {
                bits.push(encoded & (1 << shift) != 0);
            }
        }
        let mut bits = vec![false; 4 + 3]; // no sublayers
        bits.push(true);
        bits.extend(std::iter::repeat_n(false, 96));
        ue(&mut bits, 0);
        ue(&mut bits, 1);
        ue(&mut bits, width);
        ue(&mut bits, height);
        bits.push(crop);
        if crop {
            for _ in 0..4 {
                ue(&mut bits, 0);
            }
        }
        ue(&mut bits, 0);
        ue(&mut bits, 0);
        let mut rbsp = vec![0u8; bits.len().div_ceil(8)];
        for (offset, bit) in bits.into_iter().enumerate() {
            if bit {
                rbsp[offset / 8] |= 1 << (7 - offset % 8);
            }
        }
        let mut nal = vec![33 << 1, 1];
        let mut zeros = 0;
        for byte in rbsp {
            if zeros >= 2 && byte <= 3 {
                nal.push(3);
                zeros = 0;
            }
            nal.push(byte);
            zeros = if byte == 0 { zeros + 1 } else { 0 };
        }
        nal
    }

    #[test]
    #[cfg(feature = "external-heic-fixture-tests")]
    fn real_iphone_12_pro_portrait() {
        let path = std::env::var("HEIC_IPHONE_FIXTURE").expect("HEIC_IPHONE_FIXTURE required");
        let bytes = std::fs::read(path).unwrap();
        let decoded = decode(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (3024, 4032));
        let (thumb, large) = super::super::process_photo(&bytes).unwrap();
        let thumb = image::load_from_memory(&thumb).unwrap();
        let large = image::load_from_memory(&large).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (320, 426));
        assert_eq!((large.width(), large.height()), (960, 1280));
    }
}
