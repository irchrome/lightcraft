/* IR fork: Nikon High Efficiency NEF through LibRaw's public C API (libraw.h only). */
#include <stdlib.h>
#include <string.h>
#include "libraw/libraw.h"

/* Decodes the file in memory to its unprocessed sensor samples (raw_width x raw_height, row-major, no black
 * subtraction, no scaling). On success returns 0, sets *out to a malloc'ed buffer the caller frees with
 * lc_nikon_he_free, and the dimensions; on failure returns LibRaw's error code (non-zero). */
int lc_nikon_he_decode(const void *buf, size_t len, unsigned short **out, unsigned *width, unsigned *height,
                       char *decoder, size_t decoder_len) {
  *out = NULL;
  libraw_data_t *lr = libraw_init(0);
  if (!lr) return -1;
  int rc = libraw_open_buffer(lr, buf, len);
  if (rc == 0) rc = libraw_unpack(lr);
  if (rc == 0 && !lr->rawdata.raw_image) rc = -2;
  if (rc == 0) {
    const char *name = libraw_unpack_function_name(lr);
    if (decoder && decoder_len) {
      strncpy(decoder, name ? name : "", decoder_len - 1);
      decoder[decoder_len - 1] = 0;
    }
    unsigned w = lr->sizes.raw_width, h = lr->sizes.raw_height, pitch = lr->sizes.raw_pitch / 2;
    unsigned short *dst = malloc((size_t)w * h * sizeof(unsigned short));
    if (!dst) rc = -3;
    else {
      for (unsigned y = 0; y < h; y++) memcpy(dst + (size_t)y * w, lr->rawdata.raw_image + (size_t)y * pitch, w * 2);
      *out = dst;
      *width = w;
      *height = h;
    }
  }
  libraw_close(lr);
  return rc;
}

void lc_nikon_he_free(unsigned short *p) { free(p); }
