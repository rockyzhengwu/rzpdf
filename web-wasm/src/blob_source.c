#include <emscripten.h>
#include <stdint.h>

// Callbacks are per Emscripten module. Exceptions become ordinary I/O errors,
// never JavaScript exceptions crossing a Rust frame.
EM_JS(int, rz_blob_read_js, (uint32_t id, double offset, uint8_t *target, uint32_t length), {
    try { return Module['rzBlobRead'](id, offset, target, length); }
    catch (_) { return -1; }
});
EM_JS(void, rz_blob_release_js, (uint32_t id), {
    try { Module['rzBlobRelease'](id); } catch (_) {}
});

// Defined C wrappers cause this archive member (including its EM_JS metadata)
// to be pulled into the final link when Rust references these symbols.
int rz_blob_read(uint32_t id, double offset, uint8_t *target, uint32_t length) {
    return rz_blob_read_js(id, offset, target, length);
}
void rz_blob_release(uint32_t id) {
    rz_blob_release_js(id);
}
