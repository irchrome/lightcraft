//! IR fork raw decoders invoked only when `craft-raw` returns [`RawError::Unsupported`]:
//! - Olympus compressed ORF (`ir_olympus12`, measured E01/V02 12-bit profile; fork-only).
//! - Optional Nikon High Efficiency NEF (`nikon-he` feature via local `lightcraft-nikon-he` LibRaw shim).

use craft_tiff::image::{ImageInfo, Layout, chunk_bytes};
use craft_tiff::makernote::{self, MakerNote};
use craft_tiff::{Ifd, ParseOptions, Tiff, Value, tags as t};

use crate::ir_olympus12;
use crate::{BlackLevel, Cfa, ColorData, LensMetadata, Mode, OpcodeLists, Orientation, RawData, RawError, RawFormat, RawImage, Rect, Result};

const EXIF_CFA_PATTERN: u16 = 0xa302;

pub(crate) fn fallback_decode(bytes: &[u8], format: Option<RawFormat>, mode: Mode) -> Option<Result<RawImage>> {
    match format {
        Some(RawFormat::Orf) => try_olympus12(bytes, mode),
        #[cfg(feature = "nikon-he")]
        Some(RawFormat::Nef) => try_nikon_he(bytes, mode),
        _ => None,
    }
}

fn cfa_from_exif(tiff: &Tiff) -> Option<Cfa> {
    let &[c0, c1, r0, r1, s0, s1, s2, s3] = tiff.exif()?.bytes(EXIF_CFA_PATTERN)? else { return None };
    let two = |a: u8, b: u8| matches!((a, b), (2, 0) | (0, 2));
    let pattern = match [s0, s1, s2, s3] {
        [0, 1, 1, 2] | [2, 1, 1, 0] | [1, 0, 2, 1] | [1, 2, 0, 1] => vec![s0, s1, s2, s3],
        _ => return None,
    };
    (two(c0, c1) && two(r0, r1)).then_some(Cfa { width: 2, height: 2, origin_x: 0, origin_y: 0, pattern })
}

fn white_from_data(data: &[u16], bits: u32) -> f32 {
    let full_level = (1u32 << bits.clamp(1, 16)) - 1;
    let full = full_level as f32;
    let step = (data.len() / 2_000_000).max(1);
    let mut hist = vec![0usize; full_level as usize + 1];
    for &v in data.iter().step_by(step) {
        if let Some(h) = hist.get_mut(v as usize) {
            *h += 1;
        }
    }
    let min_plateau = (data.len() / 20_000).max(16);
    let strays = min_plateau / 2;
    let mut above = 0usize;
    let Some(mx) = hist.iter().enumerate().rev().find_map(|(v, &n)| {
        above += n * step;
        (above > strays).then_some(v as u32)
    }) else {
        return full;
    };
    let band = (mx / 256).max(1);
    let near = mx.saturating_sub(band);
    let below = near.saturating_sub(band);
    let count = |from: u32, to: u32| hist.get(from as usize..to as usize).map_or(0, |h| h.iter().sum::<usize>());
    let top = count(near, full_level + 1);
    let under = count(below, near);
    if mx as f32 > full * 0.5 && top * step >= min_plateau && top >= 4 * under + 4 { near as f32 } else { full }
}

fn parse_makernote_ifd(bytes: &[u8], tiff: &Tiff, ifd0: &Ifd) -> Option<MakerNote> {
    let make = ifd0.string(t::MAKE).unwrap_or_default();
    let e = tiff.exif()?.get(t::MAKER_NOTE)?;
    makernote::parse_makernote(bytes, e.offset, e.count() as u64, tiff.order, &make)
}

fn sub_ifd(bytes: &[u8], mn: &MakerNote, tag: u16) -> Option<Ifd> {
    let e = mn.ifd.get(tag)?;
    let at = match &e.value {
        Value::Undefined(_) | Value::Byte(_) => e.offset,
        _ => mn.base.checked_add(mn.ifd.u64(tag)?)?,
    };
    let opts = ParseOptions { max_ifds: 4, max_depth: 1, follow_children: false, ..Default::default() };
    craft_tiff::parse_ifd_at(bytes, at, mn.order, mn.base, false, &opts).ok().map(|(i, _)| i)
}

fn validate_orf_container(tiff: &Tiff, ifd0: &Ifd, info: &ImageInfo, stated: Option<&Cfa>, chunks_len: usize, h: usize) -> bool {
    tiff.order == craft_tiff::ByteOrder::Little
        && info.bits_per_sample.as_slice() == [16]
        && info.samples_per_pixel == 1
        && ifd0.u64(t::SAMPLES_PER_PIXEL).is_none_or(|v| v == 1)
        && info.sample_format == 1
        && info.planar == 1
        && info.predictor == 1
        && stated.is_some()
        && chunks_len == 1
        && info.byte_counts.first().is_some_and(|&count| count > 0)
        && matches!(info.layout, Layout::Strips { rows_per_strip } if rows_per_strip as usize == h)
}

fn orf_metadata(ip: Option<&Ifd>, cfa: &Cfa, w: usize, h: usize) -> (Rect, BlackLevel, Option<[f32; 3]>) {
    const IMAGE_CROP: [u16; 4] = [0x0612, 0x0613, 0x0614, 0x0615];
    const BLACK: u16 = 0x0600;
    const WB_RB: u16 = 0x0100;
    const WB_G: u16 = 0x011f;
    let active = match ip.map(|i| IMAGE_CROP.map(|tag| i.u64(tag).and_then(|v| usize::try_from(v).ok()))) {
        Some([Some(x), Some(y), Some(cw), Some(ch)]) if cw > 0 && ch > 0 && x <= w && y <= h && cw <= w - x && ch <= h - y => Rect::new(x, y, cw, ch),
        _ => Rect::new(0, 0, w, h),
    };
    let black = match ip.and_then(|i| i.f64s(BLACK)).as_deref() {
        Some(v @ [_, _, _, _]) => {
            let a = cfa.shifted(active.x, active.y);
            let values = a.pattern.iter().map(|&c| [v[0], (v[1] + v[2]) / 2.0, v[3]][c as usize] as f32).collect();
            BlackLevel { repeat_rows: 2, repeat_cols: 2, values, ..Default::default() }
        }
        _ => BlackLevel::uniform(0.0),
    };
    let wb = ip
        .and_then(|i| Some((i.f64s(WB_RB)?, i.f64(WB_G).filter(|g| *g > 0.0 && g.is_finite()).unwrap_or(256.0))))
        .filter(|(v, _)| v.len() >= 2 && v[0] > 0.0 && v[1] > 0.0)
        .map(|(v, g)| [(v[0] / g) as f32, 1.0, (v[1] / g) as f32]);
    (active, black, wb)
}

fn try_olympus12(bytes: &[u8], mode: Mode) -> Option<Result<RawImage>> {
    const IMAGE_PROCESSING: u16 = 0x2040;
    let tiff = Tiff::parse(bytes).ok()?;
    let ifd0 = tiff.ifds.first()?;
    let info = ifd0.image().ok()?;
    if info.compression != 1 {
        return None;
    }
    let mn = parse_makernote_ifd(bytes, &tiff, ifd0)?;
    let ip = sub_ifd(bytes, &mn, IMAGE_PROCESSING)?;
    if !ir_olympus12::matches_profile(&ip) {
        return ir_olympus12::has_coding_fields(&ip).then(|| Err(RawError::Unsupported("unverified Olympus coding profile".into())));
    }
    Some(decode_olympus12(bytes, &tiff, ifd0, &info, &ip, mode))
}

fn decode_olympus12(bytes: &[u8], tiff: &Tiff, ifd0: &Ifd, info: &ImageInfo, ip: &Ifd, mode: Mode) -> Result<RawImage> {
    let (w, h) = (info.width as usize, info.height as usize);
    let _ = crate::checked_pixel_count(w as u64, h as u64).ok_or(RawError::Limit("image too large"))?;
    let chunks = info.chunks(bytes.len() as u64);
    let stated = cfa_from_exif(tiff);
    if !validate_orf_container(tiff, ifd0, info, stated.as_ref(), chunks.len(), h) {
        return Err(RawError::Unsupported("unverified compressed ORF container layout or Bayer metadata".into()));
    }
    let src = chunk_bytes(bytes, &chunks[0]).ok_or_else(|| RawError::Corrupt("ORF strip outside file".into()))?;
    let _ = ir_olympus12::validate(src, w, h)?;
    let samples = if mode == Mode::Full { ir_olympus12::decode(src, w, h)? } else { Vec::new() };
    let cfa = stated.unwrap_or_else(|| Cfa { width: 2, height: 2, origin_x: 0, origin_y: 0, pattern: vec![0, 1, 1, 2] });
    let (active_area, black, wb_multipliers) = orf_metadata(Some(ip), &cfa, w, h);
    let white = if mode == Mode::Full { white_from_data(&samples, 12) } else { 4095.0 };
    let crop = Rect::new(0, 0, active_area.width, active_area.height);
    let mut img = RawImage {
        format: RawFormat::Orf,
        width: w,
        height: h,
        cpp: 1,
        data: RawData::U16(samples),
        cfa: Some(cfa),
        bits: 12,
        black,
        white: vec![white],
        active_area,
        crop,
        orientation: Orientation::from_exif(ifd0.u16(t::ORIENTATION).unwrap_or(1)),
        color: ColorData::default(),
        wb_multipliers,
        linearized: false,
        pixel_aspect: 1.0,
        opcodes: OpcodeLists::default(),
        metadata: Default::default(),
        lens_metadata: LensMetadata::default(),
    };
    img.metadata = crate::convert::exif_metadata(bytes, img.info().developed_size());
    img.check(mode == Mode::Full)?;
    Ok(img)
}

#[cfg(feature = "nikon-he")]
fn try_nikon_he(bytes: &[u8], mode: Mode) -> Option<Result<RawImage>> {
    const JPEG_XS_START: [u8; 4] = [0xff, 0x10, 0xff, 0x50];
    let tiff = Tiff::parse(bytes).ok()?;
    let ifd0 = tiff.ifds.first()?;
    let raw = tiff
        .all_ifds()
        .into_iter()
        .filter(|i| i.u16(t::PHOTOMETRIC) == Some(t::photometric::CFA))
        .max_by_key(|i| i.u64(t::IMAGE_WIDTH).unwrap_or(0).saturating_mul(i.u64(t::IMAGE_LENGTH).unwrap_or(0)))?;
    let info = raw.image().ok()?;
    if info.compression != t::compression::NIKON {
        return None;
    }
    let chunks = info.chunks(bytes.len() as u64);
    let [c] = chunks.as_slice() else { return None };
    let head = usize::try_from(c.offset).ok().and_then(|o| bytes.get(o..o.checked_add(JPEG_XS_START.len())?))?;
    if head != JPEG_XS_START {
        return None;
    }
    Some(decode_nikon_he(bytes, &tiff, ifd0, raw, &info, mode))
}

#[cfg(feature = "nikon-he")]
fn trailing_masked_columns(d: &[u16], w: usize, bits: u32) -> usize {
    const MASK_ROWS: usize = 128;
    let rows = d.len().checked_div(w).unwrap_or(0).min(MASK_ROWS);
    if w < 128 || rows == 0 {
        return w;
    }
    let col_mean = |x: usize| (0..rows).map(|y| d.get(y * w + x).copied().unwrap_or(0) as f64).sum::<f64>() / rows as f64;
    let interior = (w / 4..w * 3 / 4).step_by(w / 64).map(col_mean).sum::<f64>() / 32.0;
    let white = ((1u32 << bits.clamp(1, 16)) - 1) as f64;
    let dark = (white * 0.01).min(interior * 0.1);
    let mut aw = w;
    while aw > w - 64 && col_mean(aw - 1) < dark {
        aw -= 1;
    }
    aw & !1
}

#[cfg(feature = "nikon-he")]
fn nikon_crop_and_cfa(raw: &Ifd, mn: Option<&MakerNote>, active_w: usize, h: usize) -> (Rect, Cfa) {
    const CROP_AREA: u16 = 0x0045;
    let fallback = Rect::new(0, 0, active_w, h);
    let crop = match mn.and_then(|m| m.ifd.u64s(CROP_AREA)).as_deref() {
        Some([x, y, cw, ch])
            if *cw > 0
                && *ch > 0
                && x.checked_add(*cw).is_some_and(|v| v <= active_w as u64)
                && y.checked_add(*ch).is_some_and(|v| v <= h as u64) =>
        {
            Rect::new(*x as usize, *y as usize, *cw as usize, *ch as usize)
        }
        _ => fallback,
    };
    let cfa = match (raw.u64s(t::CFA_REPEAT_PATTERN_DIM).as_deref(), raw.bytes(t::CFA_PATTERN_EP)) {
        (Some([2, 2]), Some(p)) if p.len() == 4 && p.iter().all(|&c| c <= 2) => {
            Cfa { width: 2, height: 2, pattern: p.to_vec(), origin_x: 0, origin_y: 0 }
        }
        _ => Cfa { width: 2, height: 2, pattern: vec![0, 1, 1, 2], origin_x: 0, origin_y: 0 },
    };
    (crop, cfa)
}

#[cfg(feature = "nikon-he")]
fn decode_nikon_he(bytes: &[u8], tiff: &Tiff, ifd0: &Ifd, raw: &Ifd, info: &ImageInfo, mode: Mode) -> Result<RawImage> {
    const HIGH_EFFICIENCY: &str = "Nikon High Efficiency NEF (HE / HE★, a JPEG XS-based codec with no public specification; shoot RAW Lossless compressed to edit the raw data)";
    const WB_RB_LEVELS: u16 = 0x000c;
    const BLACK_LEVEL: u16 = 0x003d;
    const MASK_ROWS: usize = 128;

    let (w, h) = (info.width as usize, info.height as usize);
    let bits = info.bits() as u32;
    let m = lightcraft_nikon_he::decode(bytes).map_err(|e| RawError::Unsupported(format!("{HIGH_EFFICIENCY}: {e}")))?;
    if (m.width, m.height) != (w, h) || m.samples.len() != w * h {
        return Err(RawError::Unsupported(format!("{HIGH_EFFICIENCY}: LibRaw gave {}x{}, the NEF states {w}x{h}", m.width, m.height)));
    }
    let rows = match mode {
        Mode::Full => h,
        Mode::Header => h.min(MASK_ROWS),
    };
    let mut samples = m.samples;
    samples.truncate(rows.min(h) * w);
    let active_w = trailing_masked_columns(&samples, w, bits);
    let mn = parse_makernote_ifd(bytes, tiff, ifd0);
    let black_scale = if bits == 12 { 0.25 } else { 1.0 };
    let black = match mn.as_ref().and_then(|m| m.ifd.f64s(BLACK_LEVEL)).as_deref() {
        Some([a, b, c, d]) if [a, b, c, d].iter().all(|v| **v < 16384.0) => BlackLevel {
            repeat_rows: 2,
            repeat_cols: 2,
            values: [a, b, c, d].iter().map(|v| (**v * black_scale) as f32).collect(),
            ..Default::default()
        },
        _ => BlackLevel::uniform(0.0),
    };
    let wb_multipliers = mn
        .as_ref()
        .and_then(|m| m.ifd.f64s(WB_RB_LEVELS))
        .filter(|v| v.len() >= 2 && v[0] > 0.1 && v[1] > 0.1 && v[0] < 10.0 && v[1] < 10.0)
        .map(|v| [v[0] as f32, 1.0, v[1] as f32]);
    let (crop, cfa) = nikon_crop_and_cfa(raw, mn.as_ref(), active_w, h);
    let white = match mode {
        Mode::Full => white_from_data(&samples, bits),
        Mode::Header => ((1u32 << bits.clamp(1, 16)) - 1) as f32,
    };
    let mut img = RawImage {
        format: RawFormat::Nef,
        width: w,
        height: h,
        cpp: 1,
        data: match mode {
            Mode::Full => RawData::U16(samples),
            Mode::Header => RawData::U16(Vec::new()),
        },
        cfa: Some(cfa),
        bits,
        black,
        white: vec![white],
        active_area: Rect::new(0, 0, active_w, h),
        crop,
        orientation: Orientation::from_exif(ifd0.u16(t::ORIENTATION).unwrap_or(1)),
        color: ColorData::default(),
        wb_multipliers,
        linearized: false,
        pixel_aspect: 1.0,
        opcodes: OpcodeLists::default(),
        metadata: Default::default(),
        lens_metadata: LensMetadata::default(),
    };
    img.metadata = crate::convert::exif_metadata(bytes, img.info().developed_size());
    img.check(mode == Mode::Full)?;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_tiff::tags as t;
    use craft_tiff::{ByteOrder, IfdBuilder, ImageData, TiffWriter, Value};

    const PROFILE: [(u16, u16); 14] = [
        (0x0640, 0),
        (0x0641, 5),
        (0x0642, 0),
        (0x0643, 2),
        (0x0644, 5),
        (0x0645, 4),
        (0x0646, 3),
        (0x0647, 2),
        (0x0648, 3),
        (0x0649, 4),
        (0x0650, 7),
        (0x0651, 15),
        (0x0652, 12),
        (0x0653, 5),
    ];

    fn profile_fields() -> Vec<(u16, Value)> {
        let mut f = vec![(0x0611, Value::Short(vec![12, 0]))];
        for (tag, val) in PROFILE {
            f.push((tag, Value::Short(vec![val])));
        }
        f
    }

    fn write_ifd_block(entries: &[(u16, Value)], base_offset: u32) -> Vec<u8> {
        let ifd_size = 2 + entries.len() * 12 + 4;
        let mut out = Vec::with_capacity(ifd_size);
        let mut extra = Vec::new();
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for (tag, val) in entries {
            out.extend_from_slice(&tag.to_le_bytes());
            let Value::Short(vals) = val else { continue };
            out.extend_from_slice(&3u16.to_le_bytes()); // SHORT
            out.extend_from_slice(&(vals.len() as u32).to_le_bytes());
            let raw: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
            if raw.len() <= 4 {
                let mut padded = [0u8; 4];
                padded[..raw.len()].copy_from_slice(&raw);
                out.extend_from_slice(&padded);
            } else {
                out.extend_from_slice(&(base_offset + ifd_size as u32 + extra.len() as u32).to_le_bytes());
                extra.extend_from_slice(&raw);
            }
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&extra);
        out
    }

    fn compressed_exif(fields: &[(u16, Value)]) -> IfdBuilder {
        // OlympusNew maker note: `OLYMPUS\0II\x01\0` (12 bytes), main IFD at +12, ImageProcessing sub-IFD right after it.
        let mut note = b"OLYMPUS\0II\x01\0".to_vec();
        let main_size = 2 + 12 + 4; // 1 entry: 0x2040 -> main + main_size
        let ip_at = (note.len() + main_size) as u32;
        note.extend_from_slice(&1u16.to_le_bytes());
        note.extend_from_slice(&0x2040u16.to_le_bytes());
        note.extend_from_slice(&4u16.to_le_bytes()); // LONG pointer
        note.extend_from_slice(&1u32.to_le_bytes());
        note.extend_from_slice(&ip_at.to_le_bytes());
        note.extend_from_slice(&0u32.to_le_bytes());
        note.extend_from_slice(&write_ifd_block(fields, ip_at));
        let mut exif = IfdBuilder::new();
        exif.set(EXIF_CFA_PATTERN, Value::Undefined(vec![2, 0, 2, 0, 0, 1, 1, 2]));
        exif.set(t::MAKER_NOTE, Value::Undefined(note));
        exif
    }

    fn compressed_strip() -> Vec<u8> {
        let mut s = vec![0, 0, 0, 0, 1, 0, 0];
        s.extend_from_slice(&[0x18, 0x10, 0x18, 0x10, 0x10, 0x10, 0x10, 0x10, 0x96, 0x10, 0x11, 0x10, 0x10, 0x10, 0x10, 0x10]);
        s
    }

    fn compressed_fixture(exif: IfdBuilder, bits: u16, strip: Vec<u8>) -> Vec<u8> {
        let mut ifd0 = IfdBuilder::new();
        ifd0.set(t::MAKE, Value::Ascii("OLYMPUS CORPORATION".into()))
            .set(t::MODEL, Value::Ascii("E-M1MarkII".into()))
            .set(t::IMAGE_WIDTH, Value::Long(vec![4]))
            .set(t::IMAGE_LENGTH, Value::Long(vec![4]))
            .set(t::BITS_PER_SAMPLE, Value::Short(vec![bits]))
            .set(t::COMPRESSION, Value::Short(vec![1]))
            .set(t::SAMPLES_PER_PIXEL, Value::Short(vec![1]))
            .set(t::PLANAR_CONFIGURATION, Value::Short(vec![1]))
            .set(t::SAMPLE_FORMAT, Value::Short(vec![1]))
            .set(t::PREDICTOR, Value::Short(vec![1]))
            .set_image(ImageData::Strips { rows_per_strip: 4, strips: vec![strip] })
            .set_child(t::EXIF_IFD, exif);
        let mut bytes = TiffWriter::new(ByteOrder::Little, false).write(&[ifd0]).unwrap();
        bytes[2..4].copy_from_slice(&0x4f52u16.to_le_bytes());
        bytes
    }

    #[test]
    fn compressed_orf_decodes_measured_profile_and_rejects_unverified_fields() {
        let original = profile_fields();
        let bytes = compressed_fixture(compressed_exif(&original), 16, compressed_strip());
        let img = crate::decode(&bytes).unwrap();
        assert_eq!(img.bits, 12);
        let RawData::U16(s) = &img.data else { panic!("expected u16") };
        assert_eq!(s, &[32, 0, 64, 0, 0, 0, 0, 0, 4, 0, 34, 0, 0, 0, 0, 0]);
        assert!(crate::probe_info(&bytes).is_ok());

        let mut unverified = original.clone();
        unverified[1].1 = Value::Short(vec![1]);
        let bad = compressed_fixture(compressed_exif(&unverified), 16, compressed_strip());
        assert!(matches!(crate::decode(&bad), Err(RawError::Unsupported(_))));
        assert!(matches!(crate::probe_info(&bad), Err(RawError::Unsupported(_))));

        let bad_bits = compressed_fixture(compressed_exif(&original), 14, compressed_strip());
        assert!(matches!(crate::decode(&bad_bits), Err(RawError::Unsupported(_))));
    }

    #[test]
    fn real_compressed_orf_sample_decodes_if_present() {
        let path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Documents/_IR-raw-corpus/samples-from-pictures/P9020022.ORF");
        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };
        let info = crate::probe_info(&bytes).expect("compressed ORF probe");
        let img = crate::decode(&bytes).expect("compressed ORF decode");
        assert_eq!(info.bits, 12);
        assert_eq!((img.width, img.height), (info.width, info.height));
        assert!(!img.data.is_empty());
    }
}
