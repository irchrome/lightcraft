//! IR fork: unprocessed sensor samples of Nikon High Efficiency NEFs (HE / HE★, compression 34713 carrying a
//! JPEG XS-style codestream) through a local LibRaw build. Only LibRaw's public C API is used (`libraw_init`,
//! `libraw_open_buffer`, `libraw_unpack`); no LibRaw source is read or copied. Everything else about the file
//! (black and white levels, white balance, colour, crop) stays with LightCraft's own NEF reader.

#![allow(unsafe_code)] // FFI into the local LibRaw build; every block below states its invariant

use std::ffi::{c_char, c_int, c_uint, c_ushort, c_void};

unsafe extern "C" {
    fn lc_nikon_he_decode(
        buf: *const c_void,
        len: usize,
        out: *mut *mut c_ushort,
        width: *mut c_uint,
        height: *mut c_uint,
        decoder: *mut c_char,
        decoder_len: usize,
    ) -> c_int;
    fn lc_nikon_he_free(p: *mut c_ushort);
}

/// Sensor samples as stored (row-major, `width × height`), and the LibRaw decoder that produced them.
#[derive(Debug)]
pub struct Mosaic {
    pub width: usize,
    pub height: usize,
    pub samples: Vec<u16>,
    pub decoder: String,
}

/// Decodes a whole NEF held in memory. LibRaw keeps no global state between calls, so this is safe to call from
/// several threads at once (each call has its own LibRaw instance).
pub fn decode(bytes: &[u8]) -> Result<Mosaic, String> {
    let mut out: *mut c_ushort = std::ptr::null_mut();
    let (mut w, mut h): (c_uint, c_uint) = (0, 0);
    let mut name = [0 as c_char; 64];
    // SAFETY: `bytes` outlives the call; the out-pointers are valid; the shim either leaves `out` null or hands
    // over a malloc'ed `w * h` buffer that is copied and freed below.
    let rc = unsafe { lc_nikon_he_decode(bytes.as_ptr().cast(), bytes.len(), &mut out, &mut w, &mut h, name.as_mut_ptr(), name.len()) };
    if rc != 0 || out.is_null() {
        return Err(format!("LibRaw error {rc}"));
    }
    let n = w as usize * h as usize;
    // SAFETY: the shim allocated exactly `n` samples at `out`.
    let samples = unsafe { std::slice::from_raw_parts(out, n) }.to_vec();
    // SAFETY: `out` came from the shim's malloc and is freed once.
    unsafe { lc_nikon_he_free(out) };
    let decoder = name.iter().take_while(|&&c| c != 0).map(|&c| c as u8 as char).collect();
    Ok(Mosaic { width: w as usize, height: h as usize, samples, decoder })
}
