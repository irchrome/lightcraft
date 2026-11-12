//! Fork (irchrome/lightcraft, infrared): imported camera profiles (`.dcp`) belong to one camera.
//!
//! A DNG camera profile is made for the camera its `UniqueCameraModel` names; on any other camera's
//! raw its matrices are wrong, and JPEG, TIFF or PNG photos have no camera colour for it to replace.
//! As Lightroom does, the profile lists offer a photo only the camera profiles made for its camera,
//! `develop.profile` refuses one made for another camera, and an edit that carries settings to other
//! photos (paste, sync, Auto Sync, presets) leaves each photo's own profile in place where the
//! carried camera profile doesn't fit it. LUT (`.cube`) profiles and the built-in looks are not
//! camera-bound and stay offered for every photo.
//!
//! Kept out of upstream's files: `presets.rs` and the two UI lists call the photo-aware variants
//! here, `develop.profile` calls [`refuse_unfit`], and `Session::develop_op` calls
//! [`keep_own_camera_profile`].

use lightcraft_catalog::{MediaKind, Photo, PhotoId};
use lightcraft_develop::DevelopSettings;

use crate::cmd::bad;
use crate::cmd::dcp_profiles::{DcpEntry, is_dcp_id};
use crate::{Result, Session};

/// Lowercase ASCII letters and digits only: "NIKON Z5_2" and "Nikon Z 5 2" both become `nikonz52`.
fn norm(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// Whether a camera profile made for `dcp_model` (its `UniqueCameraModel`) fits a raw photo from the
/// camera with Exif `make` and `model` (and, for a DNG, its own `unique_model` when known).
///
/// Names are compared after [`norm`], and only whole names: `dcp_model` equals the photo's unique
/// model, its model, or its make followed by its model (the whole make, or its first word, which
/// covers makes like "NIKON CORPORATION", "OLYMPUS IMAGING CORP." or "OM Digital Solutions"). With
/// the make unknown, the profile's first word may stand for it. There is no prefix matching: a
/// profile for the "E-M1 Mark II" never fits an "E-M1 Mark III". An empty name fits nothing.
pub fn dcp_fits(dcp_model: &str, make: Option<&str>, model: Option<&str>, unique_model: Option<&str>) -> bool {
    let dcp = norm(dcp_model);
    if dcp.is_empty() {
        return false;
    }
    if unique_model.map(norm).is_some_and(|u| u == dcp) {
        return true;
    }
    let model = model.map(norm).unwrap_or_default();
    if model.is_empty() {
        return false;
    }
    if model == dcp {
        return true;
    }
    let make = make.unwrap_or_default();
    let (make_full, make_first) = (norm(make), make.split_whitespace().next().map(norm).unwrap_or_default());
    if make_full.is_empty() {
        // the profile's leading word is the make the photo doesn't name
        return dcp_model.trim().split_once(char::is_whitespace).is_some_and(|(_, rest)| norm(rest) == model);
    }
    [make_full, make_first].iter().any(|m| !m.is_empty() && format!("{m}{model}") == dcp)
}

/// The photo's camera as (make, model), from the catalog's `make` and `camera` ("make model");
/// `None` for a photo that is not a raw (camera profiles don't apply to it).
fn raw_camera(p: &Photo) -> Option<(Option<&str>, Option<&str>)> {
    if p.kind != MediaKind::Raw {
        return None;
    }
    let make = p.meta.make.trim();
    let camera = p.meta.camera.trim();
    let model = if make.is_empty() { camera } else { camera.strip_prefix(make).map_or(camera, str::trim) };
    fn some(s: &str) -> Option<&str> {
        (!s.is_empty()).then_some(s)
    }
    Some((some(make), some(model)))
}

/// Whether camera profile `entry` fits photo `p` (see [`dcp_fits`]).
fn entry_fits(entry: &DcpEntry, p: &Photo) -> bool {
    raw_camera(p).is_some_and(|(make, model)| dcp_fits(&entry.camera_model, make, model, None))
}

impl Session {
    /// Whether imported camera profile `dcp_id` fits photo `photo`: an unknown profile or photo doesn't.
    pub fn camera_profile_fits(&self, dcp_id: &str, photo: PhotoId) -> bool {
        let (Some(entry), Some(p)) = (self.dcp_profiles.iter().find(|e| e.id == dcp_id), self.catalog.photo(photo)) else {
            return false;
        };
        entry_fits(entry, p)
    }

    /// [`Session::imported_profiles`] for photo `photo`: every LUT profile, and only the camera profiles made for the
    /// photo's camera (none for a photo that is not a raw, or with no photo).
    pub fn imported_profiles_for(&self, photo: Option<PhotoId>) -> Vec<(String, String, String)> {
        let mut out = self.imported_profiles();
        out.retain(|(id, _, _)| self.profile_offered(id, photo));
        out
    }

    /// [`Session::profile_info`], or `None` for a camera profile that does not fit photo `photo` (favourites and the
    /// recent list leave it out).
    pub fn profile_info_for(&self, id: &str, photo: Option<PhotoId>) -> Option<(&str, &str)> {
        self.profile_offered(id, photo).then(|| self.profile_info(id)).flatten()
    }

    /// Whether profile `id` may be offered for photo `photo`: anything but an imported camera profile is; a camera
    /// profile only for a photo it fits.
    fn profile_offered(&self, id: &str, photo: Option<PhotoId>) -> bool {
        if !is_dcp_id(id) || !self.dcp_profiles.iter().any(|e| e.id == id) {
            return true;
        }
        photo.is_some_and(|p| self.camera_profile_fits(id, p))
    }
}

/// `develop.profile`: refuse camera profile `profile_id` for photo `photo` when it is made for another camera, or
/// the photo is not a raw. Other profiles pass.
pub(crate) fn refuse_unfit(s: &Session, c: &str, profile_id: &str, photo: PhotoId) -> Result<()> {
    if !is_dcp_id(profile_id) {
        return Ok(());
    }
    let (Some(entry), Some(p)) = (s.dcp_profiles.iter().find(|e| e.id == profile_id), s.catalog.photo(photo)) else {
        return Ok(());
    };
    if entry_fits(entry, p) {
        return Ok(());
    }
    let made_for = match entry.camera_model.trim() {
        "" => "no named camera".to_string(),
        m => format!("the {m}"),
    };
    let why = match (p.kind, p.meta.camera.trim()) {
        (MediaKind::Raw, "") => "this photo's camera is unknown".to_string(),
        (MediaKind::Raw, camera) => format!("this photo is from the {camera}"),
        _ if p.format.is_empty() => "this photo is not a raw".to_string(),
        _ => format!("this photo is a {} file, not a raw", p.format),
    };
    Err(bad(c, format!("camera profile “{}” is made for {made_for}; {why}", entry.name)))
}

/// `Session::develop_op`: an edit that would put a camera profile made for another camera on photo `id` (a paste,
/// sync, Auto Sync or preset carrying the profile from another photo) keeps the photo's own profile instead; the rest
/// of the edit goes on. A photo that already has that profile keeps it.
pub(crate) fn keep_own_camera_profile(s: &Session, id: PhotoId, settings: &mut DevelopSettings) {
    let new = &settings.profile.id;
    if !is_dcp_id(new) || !s.dcp_profiles.iter().any(|e| &e.id == new) || s.camera_profile_fits(new, id) {
        return;
    }
    let Some(own) = s.develop_of(id) else { return };
    if own.profile.id != *new {
        settings.profile = own.profile.clone();
    }
}

#[cfg(test)]
mod session_tests {
    use std::path::{Path, PathBuf};

    use lightcraft_catalog::PhotoId;
    use lightcraft_raw::dcp::{DcpProfile, write_dcp};
    use lightcraft_raw::{ColorData, Mat3};
    use serde_json::json;

    use crate::Session;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lc-ir-profiles-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A tiny camera profile for `camera` (our own numbers: an sRGB-like matrix).
    fn write_profile(dir: &Path, name: &str, camera: &str) -> PathBuf {
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

    /// A synthetic raw from the camera with Exif `make` and `model`.
    fn write_raw(dir: &Path, name: &str, make: &str, model: &str) -> PathBuf {
        let meta = lightcraft_meta::Metadata { make: Some(make.into()), model: Some(model.into()), ..Default::default() };
        let f = dir.join(name);
        std::fs::write(&f, crate::tests_xmp::synthetic_dng_with(None, meta)).unwrap();
        f
    }

    struct Fixture {
        s: Session,
        olympus: PhotoId,
        nikon: PhotoId,
        olympus_dcp: String,
        nikon_dcp: String,
        dir: PathBuf,
    }

    /// A library with an E-M1 II raw and a Z f raw, and one camera profile for each camera.
    fn fixture(tag: &str) -> Fixture {
        let dir = temp_dir(tag);
        let files = dir.join("files");
        std::fs::create_dir_all(&files).unwrap();
        let raws = [
            write_raw(&files, "P9030054.dng", "OLYMPUS CORPORATION", "E-M1MarkII"),
            write_raw(&files, "DSC_5987.dng", "NIKON CORPORATION", "NIKON Z f"),
        ];
        let dcps = [write_profile(&files, "EM1 IR", "Olympus E-M1 Mark II"), write_profile(&files, "Zf IR", "Nikon Z f")];
        let mut s = Session::new().with_fs();
        s.open_library(dir.join("lib"), false).unwrap();
        let mut ids = Vec::new();
        for raw in &raws {
            let r = s.execute("library.import", &json!({"paths": [raw.to_string_lossy()]})).unwrap();
            ids.push(PhotoId(r["imported"][0].as_u64().unwrap()));
        }
        let mut dcp_ids = Vec::new();
        for f in &dcps {
            let r = s.execute("profile.import", &json!({"paths": [f.to_string_lossy()]})).unwrap();
            dcp_ids.push(r["imported"][0]["id"].as_str().unwrap().to_string());
        }
        s.execute("library.select", &json!({"ids": [ids[0].0]})).unwrap();
        Fixture { s, olympus: ids[0], nikon: ids[1], olympus_dcp: dcp_ids[0].clone(), nikon_dcp: dcp_ids[1].clone(), dir }
    }

    fn menu_ids(s: &Session) -> String {
        s.profile_menu().to_string()
    }

    /// The profile lists offer a photo only its own camera's profiles; LUT profiles stay; no photo, no camera profiles.
    #[test]
    fn the_lists_offer_only_the_photos_camera_profiles() {
        let mut f = fixture("lists");
        f.s.lut_profiles.push(crate::cmd::lut_profiles::LutProfile {
            id: "lut:look".into(),
            name: "Look".into(),
            group: "Looks".into(),
            file: String::new(),
        });
        let ids = |s: &Session, p: Option<PhotoId>| s.imported_profiles_for(p).into_iter().map(|p| p.0).collect::<Vec<_>>();
        assert_eq!(ids(&f.s, Some(f.olympus)), ["lut:look".to_string(), f.olympus_dcp.clone()]);
        assert_eq!(ids(&f.s, Some(f.nikon)), ["lut:look".to_string(), f.nikon_dcp.clone()]);
        assert_eq!(ids(&f.s, None), ["lut:look".to_string()]);
        let menu = menu_ids(&f.s);
        assert!(menu.contains(&f.olympus_dcp) && !menu.contains(&f.nikon_dcp), "{menu}");
        // a favourite made for another camera is left out too
        f.s.execute("profile.favorite", &json!({"id": f.nikon_dcp})).unwrap();
        assert!(!menu_ids(&f.s).contains(&f.nikon_dcp));
        assert!(f.s.profile_info_for(&f.nikon_dcp, Some(f.olympus)).is_none());
        assert!(f.s.profile_info_for(&f.nikon_dcp, Some(f.nikon)).is_some());
        assert!(f.s.profile_info_for("lut:look", Some(f.olympus)).is_some() && f.s.profile_info_for("lc.vivid", None).is_some());
        f.s.execute("library.select", &json!({"ids": [f.nikon.0]})).unwrap();
        let menu = menu_ids(&f.s);
        assert!(menu.contains(&f.nikon_dcp) && !menu.contains(&f.olympus_dcp), "{menu}");
        f.s.close_library().unwrap();
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    /// `develop.profile` refuses another camera's profile, says which cameras, and changes nothing.
    #[test]
    fn another_cameras_profile_is_refused_and_nothing_changes() {
        let mut f = fixture("refuse");
        let before = f.s.develop_of(f.olympus).unwrap();
        let history = f.s.catalog.photo(f.olympus).unwrap().history.len();
        let e = f.s.execute("develop.profile", &json!({"id": f.nikon_dcp})).unwrap_err().to_string();
        assert!(e.contains("Nikon Z f") && e.contains("E-M1MarkII"), "{e}");
        assert_eq!(f.s.develop_of(f.olympus).unwrap(), before);
        assert_eq!(f.s.catalog.photo(f.olympus).unwrap().history.len(), history);
        assert!(!f.s.profile_recent.contains(&f.nikon_dcp));
        f.s.execute("develop.profile", &json!({"id": f.olympus_dcp})).unwrap();
        assert_eq!(f.s.develop_of(f.olympus).unwrap().profile.id, f.olympus_dcp);
        f.s.close_library().unwrap();
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    /// A photo that is not a raw takes no camera profile and is offered none.
    #[test]
    fn a_non_raw_photo_takes_no_camera_profile() {
        let mut s = Session::with_demo();
        let dir = temp_dir("jpeg");
        let dcp = write_profile(&dir, "Zf IR", "Nikon Z f");
        let id = s.execute("profile.import", &json!({"paths": [dcp.to_string_lossy()]})).unwrap()["imported"][0]["id"].as_str().unwrap().to_string();
        let photo = s.active().unwrap();
        assert!(!s.imported_profiles_for(Some(photo)).iter().any(|p| p.0 == id));
        let e = s.execute("develop.profile", &json!({"id": id})).unwrap_err().to_string();
        assert!(e.contains("not a raw"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sync, paste and Auto Sync carry the rest of an edit to a photo of another camera, but leave its own profile.
    #[test]
    fn sync_paste_and_auto_sync_skip_a_profile_that_does_not_fit() {
        let mut f = fixture("sync");
        let (o, n) = (f.olympus, f.nikon);
        f.s.execute("develop.profile", &json!({"id": f.olympus_dcp})).unwrap();
        f.s.execute("develop.set", &json!({"control": "light.exposure", "value": 0.7})).unwrap();
        f.s.execute("library.select", &json!({"ids": [o.0, n.0], "active": o.0})).unwrap();
        f.s.execute("develop.sync", &json!({})).unwrap();
        let synced = f.s.develop_of(n).unwrap();
        assert_eq!((synced.light.exposure, synced.profile.id.as_str()), (0.7, "lc.color"), "the exposure goes, the profile stays");
        // paste
        f.s.execute("develop.copy", &json!({})).unwrap();
        f.s.execute("develop.set", &json!({"control": "light.exposure", "value": 1.2})).unwrap();
        f.s.execute("develop.copy", &json!({})).unwrap();
        f.s.execute("develop.paste", &json!({"ids": [n.0]})).unwrap();
        let pasted = f.s.develop_of(n).unwrap();
        assert_eq!((pasted.light.exposure, pasted.profile.id.as_str()), (1.2, "lc.color"));
        // Auto Sync: the Olympus photo takes the change, the Nikon one keeps its own profile
        f.s.auto_sync = true;
        f.s.execute("develop.profile", &json!({"id": "lc.vivid"})).unwrap();
        assert_eq!(f.s.develop_of(n).unwrap().profile.id, "lc.vivid", "a built-in look syncs to every photo");
        f.s.execute("develop.profile", &json!({"id": f.olympus_dcp})).unwrap();
        assert_eq!(f.s.develop_of(o).unwrap().profile.id, f.olympus_dcp);
        assert_eq!(f.s.develop_of(n).unwrap().profile.id, "lc.vivid", "Auto Sync left the Nikon photo's own profile");
        f.s.close_library().unwrap();
        let _ = std::fs::remove_dir_all(&f.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::dcp_fits;

    /// The owner's cameras as the decoder reads them (make, model) and their profiles' `UniqueCameraModel`, observed
    /// on 2026-11-12 (Adobe CameraRaw profile folder; IR Swap Studio and imported CLiR profiles).
    const CAMERAS: [(&str, &str, &str); 6] = [
        ("Olympus E-M1 Mark II", "OLYMPUS CORPORATION", "E-M1MarkII"),
        ("Nikon Z f", "NIKON CORPORATION", "NIKON Z f"),
        ("Nikon Z 5 2", "NIKON CORPORATION", "NIKON Z5_2"),
        ("Fujifilm X-S20", "FUJIFILM", "X-S20"),
        ("Fujifilm GFX 50S II", "FUJIFILM", "GFX50S II"),
        ("Olympus E-PM2", "OLYMPUS IMAGING CORP.", "E-PM2"),
    ];

    /// Every camera fits its own profiles and none of the others'.
    #[test]
    fn each_camera_fits_only_its_own_profiles() {
        for (dcp, ..) in CAMERAS {
            for (own, make, model) in CAMERAS {
                assert_eq!(dcp_fits(dcp, Some(make), Some(model), None), dcp == own, "{dcp} on {make} {model}");
            }
        }
    }

    /// Close names never match: no prefix matching, and unknown cameras and empty names fit nothing.
    #[test]
    fn near_names_do_not_match() {
        let em1ii = ("OLYMPUS CORPORATION", "E-M1MarkII");
        let em1iii = ("OM Digital Solutions", "E-M1MarkIII");
        assert!(!dcp_fits("Olympus E-M1 Mark III", Some(em1ii.0), Some(em1ii.1), None));
        assert!(!dcp_fits("Olympus E-M1 Mark II", Some(em1iii.0), Some(em1iii.1), None));
        assert!(dcp_fits("OM Digital Solutions E-M1 Mark III", Some(em1iii.0), Some(em1iii.1), None), "the whole make");
        assert!(!dcp_fits("Nikon Z 5", Some("NIKON CORPORATION"), Some("NIKON Z5_2"), None));
        assert!(!dcp_fits("Nikon Z fc", Some("NIKON CORPORATION"), Some("NIKON Z f"), None));
        assert!(!dcp_fits("Nikon Z f", None, None, None), "unknown camera");
        assert!(!dcp_fits("", Some("NIKON CORPORATION"), Some("NIKON Z f"), None), "a profile for no camera");
        assert!(!dcp_fits("Nikon", Some("NIKON CORPORATION"), Some(""), None));
    }

    /// A DNG's own `UniqueCameraModel` counts, and with the make unknown the profile's first word stands for it.
    #[test]
    fn unique_model_and_unknown_make() {
        assert!(dcp_fits("Nikon Z f", None, Some("weird"), Some("Nikon Z f")));
        assert!(dcp_fits("Nikon Z f", None, Some("Z f"), None));
        assert!(!dcp_fits("Nikon Z f", None, Some("Z fc"), None));
        assert!(!dcp_fits("Nikon Z f", Some("FUJIFILM"), Some("Z f"), None), "a named make must be the profile's");
    }
}
