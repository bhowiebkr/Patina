/* The C API of a Patina package (spec 14.2). Link patina_capi, and open the package folder.
 * The texts are UTF-8. A package is for one thread at a time. */
#ifndef PATINA_CAPI_H
#define PATINA_CAPI_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PatinaPackage PatinaPackage;

/* The samples in one audio block, and the samples in one second. */
#define PATINA_BLOCK 256
#define PATINA_RATE 48000
#define PATINA_CHANNELS 2

/* Opens the package folder `dir`. Gives NULL on an error. */
PatinaPackage *patina_open(const char *dir);

/* Sets the parameter "<node ID>.<input key>" to `value` for the next run. Gives 0, or -1 on an error. */
int patina_set(PatinaPackage *package, const char *name, const char *value);

/* Runs the batch domain one time. Gives 0, or -1 when a node failed. */
int patina_run(PatinaPackage *package);

/* Runs one frame of the frame domain. Gives 0, or -1 when a node failed or the graph has no frame nodes. */
int patina_frame(PatinaPackage *package);

/* Runs one block of the audio domain. Gives 0, or -1 when a node failed or the graph has no audio nodes. */
int patina_block(PatinaPackage *package);

/* The PATINA_BLOCK mono samples, downmixed from stereo, or NULL before a node played.
 * The samples stay until the next call. */
const float *patina_sound(PatinaPackage *package);

/* PATINA_BLOCK stereo frames, interleaved left then right, or NULL before a node played.
 * The samples stay until the next call. */
const float *patina_stereo_sound(PatinaPackage *package);

/* Runs the event domain until `count` event runs ended, or until no trigger can fire again. Each
 * `trigger` node fires one time at the start. Gives 0, or -1 when a run failed or the graph has no
 * event nodes. */
int patina_events(PatinaPackage *package, unsigned long long count);

/* The Debug text of the output "<node ID>.<output key>" after the last run, frame, block or event run. Only an output
 * with a wire has a value. Gives NULL when there is no value. The text stays until the next call. */
const char *patina_value(PatinaPackage *package, const char *name);

/* The text of the last error on this thread, or an empty text. */
const char *patina_error(void);

/* Closes the package. NULL does nothing. */
void patina_close(PatinaPackage *package);

#ifdef __cplusplus
}
#endif

#endif
