//! Fork (irchrome/lightcraft, infrared): recover camera & lens metadata from Lightroom `.lrcat`
//! tables and embedded XMP when original files are offline or moved at import time.
//!
//! Lightroom stores interned camera/lens names in `AgInternedExifCameraModel` /
//! `AgInternedExifLens` referenced by `AgHarvestedExifMetadata`, and may also embed `tiff:Make` /
//! `tiff:Model` in `Adobe_AdditionalMetadata.xmp`. Populating `Photo::meta` from these avoids
//! leaving offline RAW stubs with an empty `meta.camera`, so camera-bound `.dcp` profiles stay
//! matched right after catalog import.

use std::collections::{BTreeMap, HashMap};

use lightcraft_catalog::Photo;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Camera, lens, and exposure fields harvested from a Lightroom Classic catalog row.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct HarvestedExif {
    pub make: String,
    pub camera: String,
    pub lens: String,
    pub focal_mm: Option<f32>,
    pub aperture: Option<f32>,
    pub shutter: String,
    pub iso: Option<u32>,
}

type TableRow = BTreeMap<String, Value>;

fn text<'a>(r: &'a TableRow, k: &str) -> &'a str {
    r.get(k).and_then(Value::as_str).unwrap_or("")
}

fn id_num(r: &TableRow, k: &str) -> i64 {
    r.get(k).and_then(|v| v.as_i64().or_else(|| v.as_f64().filter(|n| n.is_finite()).map(|n| n as i64))).unwrap_or(0)
}

fn real(r: &TableRow, k: &str) -> Option<f64> {
    r.get(k).and_then(Value::as_f64).filter(|n| n.is_finite())
}

fn format_shutter_apex(tv: f64) -> Option<String> {
    let t = 2.0_f64.powf(-tv);
    if !t.is_finite() || t <= 0.0 {
        return None;
    }
    Some(if t >= 1.0 { format!("{t:.0}") } else { format!("1/{:.0}", 1.0 / t) })
}

fn format_aperture_apex(av: f64) -> Option<f32> {
    if av <= 0.0 {
        return None;
    }
    let f = 2.0_f64.powf(av / 2.0);
    if !f.is_finite() || f <= 0.0 {
        return None;
    }
    Some(((f * 10.0).round() / 10.0) as f32)
}

/// Read optional Lightroom EXIF tables (`AgInternedExifCameraModel`, `AgInternedExifLens`,
/// `AgHarvestedExifMetadata`) keyed by `Adobe_images.id_local`.
pub(crate) fn read_harvested(
    optional_table: &mut impl FnMut(&str) -> std::result::Result<Vec<TableRow>, String>,
) -> std::result::Result<HashMap<i64, HarvestedExif>, String> {
    let cameras: HashMap<i64, String> = optional_table("AgInternedExifCameraModel")?
        .into_iter()
        .filter_map(|r| {
            let id = id_num(&r, "id_local");
            let val = text(&r, "value").trim().to_string();
            (id != 0 && !val.is_empty()).then_some((id, val))
        })
        .collect();
    let lenses: HashMap<i64, String> = optional_table("AgInternedExifLens")?
        .into_iter()
        .filter_map(|r| {
            let id = id_num(&r, "id_local");
            let val = text(&r, "value").trim().to_string();
            (id != 0 && !val.is_empty()).then_some((id, val))
        })
        .collect();
    let mut out = HashMap::new();
    for r in optional_table("AgHarvestedExifMetadata")? {
        let image = id_num(&r, "image");
        if image == 0 {
            continue;
        }
        let camera = cameras.get(&id_num(&r, "cameraModelRef")).cloned().unwrap_or_default();
        let lens = lenses.get(&id_num(&r, "lensRef")).cloned().unwrap_or_default();
        let focal_mm = real(&r, "focalLength").filter(|v| *v > 0.0).map(|v| v as f32);
        let aperture = real(&r, "aperture").and_then(format_aperture_apex);
        let shutter = real(&r, "shutterSpeed").and_then(format_shutter_apex).unwrap_or_default();
        let iso = real(&r, "isoSpeedRating").filter(|v| *v > 0.0 && *v <= u32::MAX as f64).map(|v| v.round() as u32);
        out.insert(image, HarvestedExif { make: String::new(), camera, lens, focal_mm, aperture, shutter, iso });
    }
    Ok(out)
}

const EXIF_KEY: &str = "__ir_exif";

/// Attach harvested EXIF metadata to `image` row only when present (keeps upstream structs unchanged).
pub(crate) fn attach_to_image(image: &mut TableRow, exif: Option<HarvestedExif>) {
    if let Some(exif) = exif
        && let Ok(val) = serde_json::to_value(exif)
    {
        image.insert(EXIF_KEY.to_string(), val);
    }
}

/// Fill empty camera/lens/EXIF fields on `p` from its Lightroom XMP packet (`tiff:Make` /
/// `tiff:Model` / `aux:Lens`) and harvested `.lrcat` EXIF metadata stored on `image`. Never
/// overwrites non-empty fields already populated from the file on disk.
pub(crate) fn fill_photo_meta(p: &mut Photo, image: &TableRow, xmp: &str) {
    let exif: HarvestedExif = image.get(EXIF_KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    fill_photo_meta_exif(p, &exif, xmp);
}

fn fill_photo_meta_exif(p: &mut Photo, exif: &HarvestedExif, xmp: &str) {
    if !xmp.is_empty()
        && (p.meta.make.is_empty() || p.meta.camera.is_empty() || p.meta.lens.is_empty() || p.meta.lens_make.is_empty())
        && let Ok(parsed) = lightcraft_meta::parse_xmp(xmp)
    {
        let m = &parsed.metadata;
        if p.meta.make.is_empty()
            && let Some(make) = m.make.as_deref().map(str::trim).filter(|s| !s.is_empty())
        {
            p.meta.make = make.to_string();
        }
        if p.meta.camera.is_empty() {
            let model = m.model.as_deref().map(str::trim).unwrap_or("");
            if !model.is_empty() {
                if !p.meta.make.is_empty() && !model.to_ascii_lowercase().starts_with(&p.meta.make.to_ascii_lowercase()) {
                    p.meta.camera = format!("{} {model}", p.meta.make.trim());
                } else {
                    p.meta.camera = model.to_string();
                }
            }
        }
        if p.meta.lens_make.is_empty()
            && let Some(lm) = m.lens_make.as_deref().map(str::trim).filter(|s| !s.is_empty())
        {
            p.meta.lens_make = lm.to_string();
        }
        if p.meta.lens.is_empty()
            && let Some(lens) = m.lens_model.as_deref().map(str::trim).filter(|s| !s.is_empty())
        {
            p.meta.lens = lens.to_string();
        }
    }

    if p.meta.make.is_empty() && !exif.make.trim().is_empty() {
        p.meta.make = exif.make.trim().to_string();
    }
    if p.meta.camera.is_empty() && !exif.camera.trim().is_empty() {
        let cam = exif.camera.trim();
        if !p.meta.make.is_empty() && !cam.to_ascii_lowercase().starts_with(&p.meta.make.to_ascii_lowercase()) {
            p.meta.camera = format!("{} {cam}", p.meta.make.trim());
        } else {
            p.meta.camera = cam.to_string();
        }
    }
    if p.meta.lens.is_empty() && !exif.lens.trim().is_empty() {
        p.meta.lens = exif.lens.trim().to_string();
    }
    p.meta.focal_mm = p.meta.focal_mm.or(exif.focal_mm);
    p.meta.aperture = p.meta.aperture.or(exif.aperture);
    if p.meta.shutter.is_empty() && !exif.shutter.is_empty() {
        p.meta.shutter = exif.shutter.clone();
    }
    p.meta.iso = p.meta.iso.or(exif.iso);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use lightcraft_catalog::{MediaKind, Photo, PhotoId, Source};
    use lightcraft_raw::dcp::{DcpProfile, write_dcp};
    use lightcraft_raw::{ColorData, Mat3};
    use serde_json::json;

    use super::*;
    use crate::Session;
    use crate::lightroom_sqlite::Value as SqlValue;

    fn varint(mut n: u64) -> Vec<u8> {
        let mut out = vec![(n & 127) as u8];
        n >>= 7;
        while n > 0 {
            out.push((n & 127) as u8 | 128);
            n >>= 7;
        }
        out.reverse();
        out
    }

    fn record(values: &[SqlValue]) -> Vec<u8> {
        let mut header = Vec::new();
        let mut body = Vec::new();
        for value in values {
            let serial = match value {
                SqlValue::Null => 0,
                SqlValue::Integer(n) => {
                    body.extend(n.to_be_bytes());
                    6
                }
                SqlValue::Real(x) => {
                    body.extend(x.to_be_bytes());
                    7
                }
                SqlValue::Text(s) => {
                    body.extend(s.as_bytes());
                    13 + 2 * s.len() as u64
                }
                _ => panic!("unexpected SqlValue in fixture: {value:?}"),
            };
            header.extend(varint(serial));
        }
        let mut out = varint(1 + header.len() as u64);
        out.extend(header);
        out.extend(body);
        out
    }

    fn leaf(page: &mut [u8], offset: usize, rows: &[Vec<SqlValue>]) {
        page[offset] = 13;
        page[offset + 3..offset + 5].copy_from_slice(&(rows.len() as u16).to_be_bytes());
        let mut end = page.len();
        for (i, values) in rows.iter().enumerate() {
            let payload = record(values);
            let mut cell = varint(payload.len() as u64);
            cell.extend(varint(i as u64 + 1));
            cell.extend(payload);
            end -= cell.len();
            page[end..end + cell.len()].copy_from_slice(&cell);
            page[offset + 8 + i * 2..offset + 10 + i * 2].copy_from_slice(&(end as u16).to_be_bytes());
        }
        page[offset + 5..offset + 7].copy_from_slice(&(end as u16).to_be_bytes());
    }

    fn sqlite_catalog(tables: &[(&str, &str, Vec<Vec<SqlValue>>)]) -> Vec<u8> {
        use SqlValue::{Integer as I, Text as T};
        let pages = (tables.len() + 1).max(5);
        let mut bytes = vec![0u8; pages * 2048];
        bytes[..16].copy_from_slice(b"SQLite format 3\0");
        bytes[16..18].copy_from_slice(&2048u16.to_be_bytes());
        bytes[18] = 1;
        bytes[19] = 1;
        bytes[21] = 64;
        bytes[22] = 32;
        bytes[23] = 32;
        bytes[28..32].copy_from_slice(&(pages as u32).to_be_bytes());
        bytes[44..48].copy_from_slice(&4u32.to_be_bytes());
        bytes[56..60].copy_from_slice(&1u32.to_be_bytes());
        let mut schema = Vec::new();
        for (i, (name, columns, rows)) in tables.iter().enumerate() {
            schema.push(vec![
                T("table".into()),
                T((*name).into()),
                T((*name).into()),
                I(i as i64 + 2),
                T(format!("CREATE TABLE {name} ({columns})")),
            ]);
            leaf(&mut bytes[(i + 1) * 2048..(i + 2) * 2048], 0, rows);
        }
        leaf(&mut bytes[..2048], 100, &schema);
        bytes
    }

    fn write_test_dcp(dir: &std::path::Path, name: &str, camera: &str) -> PathBuf {
        let m = Mat3([[3.2406, -1.5372, -0.4986], [-0.9689, 1.8758, 0.0415], [0.0557, -0.2040, 1.0570]]);
        let p = DcpProfile {
            name: name.into(),
            camera_model: camera.into(),
            color: ColorData { illuminant: [21, 0], color_matrix: [Some(m), None], ..Default::default() },
        };
        let f = dir.join(format!("{name}.dcp"));
        std::fs::write(&f, write_dcp(&p).unwrap()).unwrap();
        f
    }

    #[test]
    fn fill_photo_meta_uses_xmp_and_harvested_exif_without_overwriting() {
        let mut p = Photo::new(PhotoId(1), Source::File { path: "/missing/DSC_0001.NEF".into() }, "DSC_0001.NEF", "RAW", 6000, 4000, "2026-11-25");
        p.kind = MediaKind::Raw;
        let xmp = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
          <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
            <rdf:Description rdf:about=""
              xmlns:tiff="http://ns.adobe.com/tiff/1.0/"
              xmlns:aux="http://ns.adobe.com/exif/1.0/aux/"
              tiff:Make="NIKON CORPORATION"
              tiff:Model="NIKON Z f"
              aux:Lens="NIKKOR Z 24-70mm f/4 S"/>
          </rdf:RDF>
        </x:xmpmeta>"#;
        let harvested = HarvestedExif {
            make: String::new(),
            camera: "OTHER MODEL".into(),
            lens: "Other Lens".into(),
            focal_mm: Some(35.0),
            aperture: Some(4.0),
            shutter: "1/250".into(),
            iso: Some(200),
        };
        fill_photo_meta_exif(&mut p, &harvested, xmp);
        assert_eq!(p.meta.make, "NIKON CORPORATION");
        assert_eq!(p.meta.camera, "NIKON CORPORATION NIKON Z f");
        assert_eq!(p.meta.lens, "NIKKOR Z 24-70mm f/4 S");
        assert_eq!(p.meta.focal_mm, Some(35.0));
        assert_eq!(p.meta.aperture, Some(4.0));
        assert_eq!(p.meta.shutter, "1/250");
        assert_eq!(p.meta.iso, Some(200));

        // Existing camera name is preserved and never overwritten.
        fill_photo_meta_exif(
            &mut p,
            &HarvestedExif { camera: "REPLACED".into(), ..Default::default() },
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description tiff:Model="REPLACED"/></rdf:RDF></x:xmpmeta>"#,
        );
        assert_eq!(p.meta.camera, "NIKON CORPORATION NIKON Z f");
    }

    #[test]
    fn importing_lrcat_with_offline_raws_recovers_camera_from_harvested_tables_and_xmp() {
        use SqlValue::{Integer as I, Null as N, Real as R, Text as T};
        let dir = std::env::temp_dir().join(format!("lc-ir-lrcat-meta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let offline_root = dir.join("offline-missing-disk");
        let xmp = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:tiff="http://ns.adobe.com/tiff/1.0/" tiff:Make="FUJIFILM" tiff:Model="X-S20"/></rdf:RDF></x:xmpmeta>"#;
        let tables = vec![
            (
                "AgLibraryRootFolder",
                "id_local INTEGER PRIMARY KEY, absolutePath, name, relativePathFromCatalog",
                vec![vec![N, T(offline_root.to_string_lossy().into()), T("Offline".into()), T("".into())]],
            ),
            ("AgLibraryFolder", "id_local INTEGER PRIMARY KEY, rootFolder, pathFromRoot", vec![vec![N, I(1), T("2026/".into())]]),
            (
                "AgLibraryFile",
                "id_local INTEGER PRIMARY KEY, folder, idx_filename",
                vec![vec![N, I(1), T("DSC_1001.NEF".into())], vec![N, I(1), T("DSCF0002.RAF".into())]],
            ),
            (
                "Adobe_images",
                "id_local INTEGER PRIMARY KEY, id_global, rootFile, fileFormat, fileWidth, fileHeight, rating, pick",
                vec![
                    vec![N, T("uuid-1".into()), I(1), T("RAW".into()), I(6000), I(4000), I(0), I(0)],
                    vec![N, T("uuid-2".into()), I(2), T("RAW".into()), I(6240), I(4160), I(0), I(0)],
                ],
            ),
            ("AgInternedExifCameraModel", "id_local INTEGER PRIMARY KEY, value", vec![vec![N, T("NIKON Z f".into())]]),
            ("AgInternedExifLens", "id_local INTEGER PRIMARY KEY, value", vec![vec![N, T("NIKKOR Z 28mm f/2.8".into())]]),
            (
                "AgHarvestedExifMetadata",
                "id_local INTEGER PRIMARY KEY, image, cameraModelRef, lensRef, focalLength, aperture, isoSpeedRating, shutterSpeed",
                vec![vec![N, I(1), I(1), I(1), R(28.0), R(4.0), I(400), R(8.0)]],
            ),
            ("Adobe_AdditionalMetadata", "id_local INTEGER PRIMARY KEY, image, xmp", vec![vec![N, I(2), T(xmp.into())]]),
        ];
        let lrcat = dir.join("offline.lrcat");
        std::fs::write(&lrcat, sqlite_catalog(&tables)).unwrap();

        let zf_dcp = write_test_dcp(&dir, "Zf IR 590nm", "Nikon Z f");
        let xs20_dcp = write_test_dcp(&dir, "XS20 IR 720nm", "Fujifilm X-S20");

        let mut s = Session::new().with_fs();
        s.open_library(dir.join("lib"), false).unwrap();
        let zf_id =
            s.execute("profile.import", &json!({"paths": [zf_dcp.to_string_lossy()]})).unwrap()["imported"][0]["id"].as_str().unwrap().to_string();
        let xs20_id =
            s.execute("profile.import", &json!({"paths": [xs20_dcp.to_string_lossy()]})).unwrap()["imported"][0]["id"].as_str().unwrap().to_string();

        let r = s.execute("library.importLightroom", &json!({"path": lrcat.to_string_lossy()})).unwrap();
        let p1 = PhotoId(r["mapping"]["1"].as_u64().unwrap());
        let p2 = PhotoId(r["mapping"]["2"].as_u64().unwrap());

        let ph1 = s.catalog.photo(p1).unwrap();
        assert_eq!(ph1.meta.camera, "NIKON Z f");
        assert_eq!(ph1.meta.lens, "NIKKOR Z 28mm f/2.8");
        assert_eq!(ph1.meta.focal_mm, Some(28.0));
        assert_eq!(ph1.meta.aperture, Some(4.0));
        assert_eq!(ph1.meta.iso, Some(400));
        assert_eq!(ph1.meta.shutter, "1/256");

        let ph2 = s.catalog.photo(p2).unwrap();
        assert_eq!(ph2.meta.make, "FUJIFILM");
        assert_eq!(ph2.meta.camera, "FUJIFILM X-S20");

        // Camera-bound DCP profiles match each offline RAW immediately without needing Reload.
        let offered_1: Vec<_> = s.imported_profiles_for(Some(p1)).into_iter().map(|p| p.0).collect();
        let offered_2: Vec<_> = s.imported_profiles_for(Some(p2)).into_iter().map(|p| p.0).collect();
        assert_eq!(offered_1, vec![zf_id]);
        assert_eq!(offered_2, vec![xs20_id]);

        s.close_library().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
