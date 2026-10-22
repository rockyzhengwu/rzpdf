/* Project-authored 4x4, two-bit indices; no external content.
 * cc generate_indexed.c $(pkg-config --cflags --libs libopenjp2) -o /tmp/rzpdf-jpx-indices
 * /tmp/rzpdf-jpx-indices gray-indices-2bit.jp2
 */
#include <openjpeg.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    opj_cparameters_t parameters;
    opj_set_default_encoder_parameters(&parameters);
    parameters.numresolution = 2;
    parameters.tcp_numlayers = 1;
    parameters.tcp_rates[0] = 0;
    parameters.cp_disto_alloc = 1;
    opj_image_cmptparm_t component;
    memset(&component, 0, sizeof(component));
    component.dx = component.dy = 1;
    component.w = component.h = 4;
    component.prec = 2;
    opj_image_t *image = opj_image_create(1, &component, OPJ_CLRSPC_GRAY);
    if (!image) return 3;
    image->x1 = image->y1 = 4;
    for (int i = 0; i < 16; ++i) image->comps[0].data[i] = i % 4;
    opj_codec_t *codec = opj_create_compress(OPJ_CODEC_JP2);
    opj_stream_t *stream = opj_stream_create_default_file_stream(argv[1], OPJ_FALSE);
    int success = codec && stream && opj_setup_encoder(codec, &parameters, image)
        && opj_start_compress(codec, image, stream) && opj_encode(codec, stream)
        && opj_end_compress(codec, stream);
    if (stream) opj_stream_destroy(stream);
    if (codec) opj_destroy_codec(codec);
    opj_image_destroy(image);
    return success ? 0 : 4;
}
