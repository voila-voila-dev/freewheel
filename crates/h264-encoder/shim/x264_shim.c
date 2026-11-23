/* The few x264 calls the lab encoder needs, behind a small C interface.
 *
 * x264's parameter struct is large and changes between builds: it stays on this side, where
 * x264.h describes it, and Rust only sees an opaque handle. Lab only (libx264 is GPL-2.0);
 * the box encodes with Rockchip MPP. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <x264.h>

struct fw_x264 {
	x264_t *encoder;
	x264_picture_t picture;
	int width;
	int height;
};

/* Quality asked of x264, high enough that the bitrate cap always binds (the lab's 720p test
 * pattern needs more than 3 Mbit/s at this quality). */
#define QUALITY_CRF 12

/* As the lab's ffmpeg (ultrafast, zero latency, a one second buffer at the bitrate, a keyframe
 * every `keyint` frames, headers before every keyframe) but with the bitrate held by the
 * buffer alone (capped CRF) rather than by x264's average bitrate (ABR): the same bitrate at a
 * constant setting, and a new one followed within a frame or two. ABR remembers the stream's
 * past: from 0.3 back to 3 Mbit/s, it took ~10 s. A hardware encoder in constant bitrate (the
 * box's Rockchip MPP) follows within a frame or two too. */
struct fw_x264 *fw_x264_open(int width, int height, int fps, int bitrate_kbps, int keyint) {
	x264_param_t param;
	if (x264_param_default_preset(&param, "ultrafast", "zerolatency") < 0) return NULL;
	param.i_width = width;
	param.i_height = height;
	param.i_csp = X264_CSP_I420;
	param.i_fps_num = (uint32_t)fps;
	param.i_fps_den = 1;
	param.i_timebase_num = 1;
	param.i_timebase_den = (uint32_t)fps;
	param.i_keyint_max = keyint;
	param.i_keyint_min = keyint;
	param.b_repeat_headers = 1;
	param.b_annexb = 1;
	param.i_log_level = X264_LOG_ERROR;
	param.rc.i_rc_method = X264_RC_CRF;
	param.rc.f_rf_constant = QUALITY_CRF;
	param.rc.i_bitrate = bitrate_kbps;
	param.rc.i_vbv_max_bitrate = bitrate_kbps;
	param.rc.i_vbv_buffer_size = bitrate_kbps;
	if (x264_param_apply_profile(&param, "high") < 0) return NULL;
	struct fw_x264 *handle = calloc(1, sizeof *handle);
	if (handle == NULL) return NULL;
	handle->encoder = x264_encoder_open(&param);
	if (handle->encoder == NULL) {
		free(handle);
		return NULL;
	}
	x264_picture_init(&handle->picture);
	handle->picture.img.i_csp = X264_CSP_I420;
	handle->picture.img.i_plane = 3;
	handle->picture.img.i_stride[0] = width;
	handle->picture.img.i_stride[1] = width / 2;
	handle->picture.img.i_stride[2] = width / 2;
	handle->width = width;
	handle->height = height;
	return handle;
}

/* The new bitrate applies from the next frame. 0 on success. */
int fw_x264_set_bitrate(struct fw_x264 *handle, int bitrate_kbps) {
	x264_param_t param;
	x264_encoder_parameters(handle->encoder, &param);
	param.rc.i_bitrate = bitrate_kbps;
	param.rc.i_vbv_max_bitrate = bitrate_kbps;
	param.rc.i_vbv_buffer_size = bitrate_kbps;
	return x264_encoder_reconfig(handle->encoder, &param) < 0 ? -1 : 0;
}

/* Encodes one I420 frame (width × height × 3 / 2 bytes), writes its NAL units into `out`.
 * Returns the bytes written, -1 if x264 failed, -2 if `out` is too small. */
int fw_x264_encode(struct fw_x264 *handle, const uint8_t *frame, int64_t pts, uint8_t *out, int capacity) {
	int luma = handle->width * handle->height;
	handle->picture.img.plane[0] = (uint8_t *)frame;
	handle->picture.img.plane[1] = (uint8_t *)frame + luma;
	handle->picture.img.plane[2] = (uint8_t *)frame + luma + luma / 4;
	handle->picture.i_pts = pts;
	x264_nal_t *nals;
	int nal_count;
	x264_picture_t encoded;
	int size = x264_encoder_encode(handle->encoder, &nals, &nal_count, &handle->picture, &encoded);
	if (size < 0) return -1;
	if (size > capacity) return -2;
	/* x264 lays the payloads of one call out contiguously. */
	if (size > 0) memcpy(out, nals[0].p_payload, (size_t)size);
	return size;
}

void fw_x264_close(struct fw_x264 *handle) {
	if (handle == NULL) return;
	x264_encoder_close(handle->encoder);
	free(handle);
}
