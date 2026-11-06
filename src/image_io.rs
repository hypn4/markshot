//! PNG decoding to straight-alpha RGBA8 and encoding back, keeping pHYs (DPI) and iCCP.

use std::borrow::Cow;
use std::io::Cursor;

use png::{BitDepth, ColorType, PixelDimensions, Transformations, Unit};

use crate::error::Error;

const METERS_PER_INCH: f64 = 0.0254;

/// How the source image's ICC profile identifies its color space.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorSpace {
    /// sRGB, or no ICC profile at all.
    Srgb,
    DisplayP3,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceImage {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) alpha, top-down rows.
    pub rgba: Vec<u8>,
    /// From a pHYs chunk in meters.
    pub dpi: Option<f64>,
    pub icc: Option<Vec<u8>>,
    pub color_space: ColorSpace,
}

/// Decodes any PNG color type, bit depth, or interlacing to RGBA8.
/// For animated PNGs only the default image is read.
///
/// # Errors
/// [`Error::Png`] when the bytes are not a valid PNG.
pub fn decode_png(bytes: &[u8]) -> Result<SourceImage, Error> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| Error::Png("image too large to decode".to_owned()))?;
    let mut buf = vec![0; size];
    let frame = reader.next_frame(&mut buf)?;
    buf.truncate(frame.buffer_size());
    if frame.bit_depth != BitDepth::Eight {
        return Err(Error::Unsupported(format!(
            "PNG decoded to {:?}-bit samples",
            frame.bit_depth
        )));
    }
    let rgba = match frame.color_type {
        ColorType::Rgba => buf,
        ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        ColorType::Grayscale => buf.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[v, a]| [v, v, v, a])
            .collect(),
        ColorType::Indexed => {
            return Err(Error::Unsupported(
                "PNG palette was not expanded".to_owned(),
            ));
        }
    };
    let info = reader.info();
    let icc = info.icc_profile.as_ref().map(|p| p.to_vec());
    Ok(SourceImage {
        width: frame.width,
        height: frame.height,
        rgba,
        dpi: info.pixel_dims.and_then(dpi_from_pixel_dims),
        color_space: color_space_of(icc.as_deref()),
        icc,
    })
}

/// Encodes straight-alpha RGBA8 rows, writing `dpi` as pHYs (pixels per meter) and `icc` as iCCP.
///
/// # Errors
/// [`Error::Png`] when `rgba` is not `width × height × 4` bytes or the size is invalid.
pub fn encode_png(
    width: u32,
    height: u32,
    rgba: &[u8],
    dpi: Option<f64>,
    icc: Option<&[u8]>,
) -> Result<Vec<u8>, Error> {
    let mut info = png::Info::with_size(width, height);
    info.color_type = ColorType::Rgba;
    info.bit_depth = BitDepth::Eight;
    info.pixel_dims = dpi.map(|dpi| {
        let ppm = ppm_from_dpi(dpi);
        PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: Unit::Meter,
        }
    });
    info.icc_profile = icc.map(Cow::Borrowed);
    let mut out = Vec::new();
    let mut writer = png::Encoder::with_info(&mut out, info)?.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(out)
}

/// the reference app's device pixel ratio for an image DPI: `clamp(round(dpi / 72), 1, 3)`, 1 without DPI.
#[must_use]
pub fn dpr_from_dpi(dpi: Option<f64>) -> f64 {
    dpi.map_or(1.0, |dpi| (dpi / 72.0).round().clamp(1.0, 3.0))
}

/// `ppm × 0.0254`, snapped to the whole DPI when that encodes back to the same ppm
/// (5669 px/m → 144, not 143.9926), so pHYs survives a decode/encode round trip either way.
fn dpi_from_pixel_dims(dims: PixelDimensions) -> Option<f64> {
    (dims.unit == Unit::Meter).then(|| {
        let exact = f64::from(dims.xppu) * METERS_PER_INCH;
        let whole = exact.round();
        if ppm_from_dpi(whole) == dims.xppu {
            whole
        } else {
            exact
        }
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "`as` saturates; DPI values come from pHYs or a positive spec value"
)]
fn ppm_from_dpi(dpi: f64) -> u32 {
    (dpi / METERS_PER_INCH).round() as u32
}

fn color_space_of(icc: Option<&[u8]>) -> ColorSpace {
    let Some(icc) = icc else {
        return ColorSpace::Srgb;
    };
    match icc_description(icc) {
        Some(d) if d.contains("Display P3") => ColorSpace::DisplayP3,
        Some(d) if d.contains("sRGB") => ColorSpace::Srgb,
        _ => ColorSpace::Other,
    }
}

/// The profile's `desc` tag text: ICC v2 `textDescriptionType` (ASCII) or ICC v4
/// `multiLocalizedUnicodeType` (UTF-16BE, all records joined by newlines).
/// `None` when the profile is malformed.
fn icc_description(icc: &[u8]) -> Option<String> {
    let tag_count = be_u32(icc, 128)?;
    let entry = icc
        .get(132..)?
        .as_chunks::<12>()
        .0
        .iter()
        .take(tag_count)
        .find(|e| e.starts_with(b"desc"))?;
    let (offset, len) = (be_u32(entry, 4)?, be_u32(entry, 8)?);
    let tag = icc.get(offset..offset.checked_add(len)?)?;
    match tag.get(..4)? {
        b"desc" => {
            let text = tag.get(12..)?;
            let text = &text[..be_u32(tag, 8)?.min(text.len())];
            let text = text.split(|&b| b == 0).next().unwrap_or_default();
            Some(String::from_utf8_lossy(text).into_owned())
        }
        b"mluc" => {
            let (count, record_size) = (be_u32(tag, 8)?, be_u32(tag, 12)?);
            if record_size < 12 {
                return None;
            }
            let records = tag.get(16..)?.chunks_exact(record_size).take(count);
            let texts = records
                .map(|r| {
                    let (len, offset) = (be_u32(r, 4)?, be_u32(r, 8)?);
                    let utf16 = tag.get(offset..offset.checked_add(len)?)?;
                    let units = utf16
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&u| u16::from_be_bytes(u));
                    Some(
                        char::decode_utf16(units)
                            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
                            .collect::<String>(),
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            Some(texts.join("\n"))
        }
        _ => None,
    }
}

fn be_u32(bytes: &[u8], at: usize) -> Option<usize> {
    let field = bytes.get(at..at.checked_add(4)?)?;
    usize::try_from(u32::from_be_bytes(field.try_into().ok()?)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_fixture(
        width: u32,
        height: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        configure: impl FnOnce(&mut png::Info<'static>),
        data: &[u8],
    ) -> Vec<u8> {
        let mut info = png::Info::with_size(width, height);
        info.color_type = color;
        info.bit_depth = depth;
        configure(&mut info);
        let mut out = Vec::new();
        let mut writer = png::Encoder::with_info(&mut out, info)
            .unwrap()
            .write_header()
            .unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        out
    }

    /// A zlib stream holding `data` in one stored (uncompressed) deflate block.
    fn zlib_stored(data: &[u8]) -> Vec<u8> {
        let len = u16::try_from(data.len()).unwrap();
        let mut out = vec![0x78, 0x01, 0x01];
        out.extend(len.to_le_bytes());
        out.extend((!len).to_le_bytes());
        out.extend(data);
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + u32::from(byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        out.extend(((b << 16) | a).to_be_bytes());
        out
    }

    #[test]
    fn dpr_rules() {
        assert_eq!(dpr_from_dpi(Some(144.0)), 2.0);
        assert_eq!(dpr_from_dpi(Some(72.0)), 1.0);
        assert_eq!(dpr_from_dpi(Some(400.0)), 3.0);
        assert_eq!(dpr_from_dpi(None), 1.0);
    }

    #[test]
    fn decodes_gray16_palette_trns_interlaced_to_rgba8() {
        let gray16 = encode_fixture(
            2,
            2,
            png::ColorType::Grayscale,
            png::BitDepth::Sixteen,
            |_| {},
            &[0x00, 0x00, 0xFF, 0xFF, 0x80, 0x00, 0x12, 0x34],
        );
        assert_eq!(
            decode_png(&gray16).unwrap().rgba,
            [
                0, 0, 0, 255, 255, 255, 255, 255, 128, 128, 128, 255, 18, 18, 18, 255
            ]
        );

        let gray_alpha = encode_fixture(
            2,
            2,
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            |_| {},
            &[10, 0, 20, 64, 30, 128, 40, 255],
        );
        assert_eq!(
            decode_png(&gray_alpha).unwrap().rgba,
            [
                10, 10, 10, 0, 20, 20, 20, 64, 30, 30, 30, 128, 40, 40, 40, 255
            ]
        );

        // 2-bit indices (0, 1 / 2, 0) into red, green, blue; tRNS makes green fully transparent
        // and leaves blue (beyond the tRNS entries) opaque.
        let palette = encode_fixture(
            2,
            2,
            png::ColorType::Indexed,
            png::BitDepth::Two,
            |info| {
                info.palette = Some(vec![255, 0, 0, 0, 255, 0, 0, 0, 255].into());
                info.trns = Some(vec![255, 0].into());
            },
            &[0b0001_0000, 0b1000_0000],
        );
        assert_eq!(
            decode_png(&palette).unwrap().rgba,
            [255, 0, 0, 255, 0, 255, 0, 0, 0, 0, 255, 255, 255, 0, 0, 255]
        );

        // Adam7 for 2×2: pass 1 holds (0,0), pass 6 holds (1,0), pass 7 holds row 1.
        // The png encoder does not interlace, so write the IDAT by hand (filter byte 0 per row).
        let mut interlaced = Vec::new();
        let mut info = png::Info::with_size(2, 2);
        info.color_type = png::ColorType::Rgb;
        info.bit_depth = png::BitDepth::Eight;
        info.interlaced = true;
        let mut writer = png::Encoder::with_info(&mut interlaced, info)
            .unwrap()
            .write_header()
            .unwrap();
        let passes = [0, 1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9, 10, 11, 12];
        writer
            .write_chunk(png::chunk::IDAT, &zlib_stored(&passes))
            .unwrap();
        writer.finish().unwrap();
        let decoded = decode_png(&interlaced).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(
            decoded.rgba,
            [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
    }

    #[test]
    fn roundtrip_keeps_phys_and_iccp() {
        let icc = icc_with_desc_tag(&mluc_tag("Display P3"));
        let rgba: Vec<u8> = (0..16).collect();
        let bytes = encode_png(2, 2, &rgba, Some(144.0), Some(&icc)).unwrap();
        let decoded = decode_png(&bytes).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(decoded.rgba, rgba);
        assert_eq!(decoded.dpi, Some(144.0));
        assert_eq!(decoded.icc.as_deref(), Some(icc.as_slice()));
        assert_eq!(decoded.color_space, ColorSpace::DisplayP3);
    }

    #[test]
    fn roundtrip_without_metadata() {
        let rgba = [9u8; 4];
        let decoded = decode_png(&encode_png(1, 1, &rgba, None, None).unwrap()).unwrap();
        assert_eq!(decoded.rgba, rgba);
        assert_eq!(decoded.dpi, None);
        assert_eq!(decoded.icc, None);
        assert_eq!(decoded.color_space, ColorSpace::Srgb);
    }

    #[test]
    fn phys_without_whole_dpi_survives_reencoding() {
        // 3000 px/m is 76.2 dpi, which is not a whole number: keep it exact.
        let rgba = [0u8; 4];
        let original = encode_fixture(
            1,
            1,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            |info| {
                info.pixel_dims = Some(png::PixelDimensions {
                    xppu: 3000,
                    yppu: 3000,
                    unit: png::Unit::Meter,
                });
            },
            &rgba,
        );
        let dpi = decode_png(&original).unwrap().dpi;
        assert!((dpi.unwrap() - 76.2).abs() < 1e-9, "{dpi:?}");
        let reencoded = encode_png(1, 1, &rgba, dpi, None).unwrap();
        assert_eq!(reencoded, original);
    }

    #[test]
    fn phys_with_unknown_unit_has_no_dpi() {
        let rgba = [0u8; 4];
        let bytes = encode_fixture(
            1,
            1,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            |info| {
                info.pixel_dims = Some(png::PixelDimensions {
                    xppu: 2,
                    yppu: 1,
                    unit: png::Unit::Unspecified,
                });
            },
            &rgba,
        );
        assert_eq!(decode_png(&bytes).unwrap().dpi, None);
    }

    #[test]
    fn rejects_non_png_and_wrong_buffer_size() {
        assert!(matches!(decode_png(b"not a png"), Err(Error::Png(_))));
        assert!(matches!(
            encode_png(2, 2, &[0; 4], None, None),
            Err(Error::Png(_))
        ));
    }

    /// A minimal ICC profile whose tag table holds a single `desc` tag with the given data.
    fn icc_with_desc_tag(tag: &[u8]) -> Vec<u8> {
        let mut icc = vec![0u8; 128];
        icc.extend(1u32.to_be_bytes());
        icc.extend(b"desc");
        icc.extend(144u32.to_be_bytes());
        icc.extend(u32::try_from(tag.len()).unwrap().to_be_bytes());
        icc.extend(tag);
        let size = u32::try_from(icc.len()).unwrap();
        icc[..4].copy_from_slice(&size.to_be_bytes());
        icc
    }

    /// ICC v2 `textDescriptionType`.
    fn text_description_tag(text: &str) -> Vec<u8> {
        let mut tag = b"desc\0\0\0\0".to_vec();
        tag.extend(u32::try_from(text.len() + 1).unwrap().to_be_bytes());
        tag.extend(text.as_bytes());
        tag.push(0);
        tag.extend([0u8; 79]); // empty Unicode and ScriptCode descriptions
        tag
    }

    /// ICC v4 `multiLocalizedUnicodeType` with a de-DE record then an en-US record.
    fn mluc_tag(text: &str) -> Vec<u8> {
        let german: Vec<u8> = "Farbprofil"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        let english: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let mut tag = b"mluc\0\0\0\0".to_vec();
        tag.extend(2u32.to_be_bytes());
        tag.extend(12u32.to_be_bytes());
        let mut offset: u32 = 16 + 2 * 12;
        for (locale, record) in [(b"deDE", &german), (b"enUS", &english)] {
            let len = u32::try_from(record.len()).unwrap();
            tag.extend(locale);
            tag.extend(len.to_be_bytes());
            tag.extend(offset.to_be_bytes());
            offset += len;
        }
        tag.extend(german);
        tag.extend(english);
        tag
    }

    #[test]
    fn detects_display_p3_from_icc_desc() {
        let space = |icc: &[u8]| color_space_of(Some(icc));
        assert_eq!(
            space(&icc_with_desc_tag(&mluc_tag("Display P3"))),
            ColorSpace::DisplayP3
        );
        assert_eq!(
            space(&icc_with_desc_tag(&text_description_tag("Display P3"))),
            ColorSpace::DisplayP3
        );
        assert_eq!(
            space(&icc_with_desc_tag(&text_description_tag(
                "sRGB IEC61966-2.1"
            ))),
            ColorSpace::Srgb
        );
        assert_eq!(
            space(&icc_with_desc_tag(&mluc_tag("sRGB IEC61966-2.1"))),
            ColorSpace::Srgb
        );
        assert_eq!(
            space(&icc_with_desc_tag(&mluc_tag("Adobe RGB (1998)"))),
            ColorSpace::Other
        );
        assert_eq!(color_space_of(None), ColorSpace::Srgb);
    }

    #[test]
    fn malformed_icc_is_other() {
        let mut lying_offsets = icc_with_desc_tag(&mluc_tag("Display P3"));
        lying_offsets[136..140].copy_from_slice(&u32::MAX.to_be_bytes());
        let mut zero_record_size = icc_with_desc_tag(&mluc_tag("Display P3"));
        zero_record_size[144 + 12..144 + 16].copy_from_slice(&0u32.to_be_bytes());
        let mut huge_tag_count = vec![0u8; 128];
        huge_tag_count.extend(u32::MAX.to_be_bytes());
        for icc in [
            &[][..],
            &[0u8; 131][..],
            &lying_offsets,
            &zero_record_size,
            &huge_tag_count,
        ] {
            assert_eq!(color_space_of(Some(icc)), ColorSpace::Other);
        }
    }
}
