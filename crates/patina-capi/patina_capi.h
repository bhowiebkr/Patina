/* The C API of a Patina package (spec 14.2). Link patina_capi, and open the package folder.
 * The texts are UTF-8. A package is for one thread at a time. */
#ifndef PATINA_CAPI_H
#define PATINA_CAPI_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PatinaPackage PatinaPackage;

/* Opens the package folder `dir`. Gives NULL on an error. */
PatinaPackage *patina_open(const char *dir);

/* Sets the parameter "<node ID>.<input key>" to `value` for the next run. Gives 0, or -1 on an error. */
int patina_set(PatinaPackage *package, const char *name, const char *value);

/* Runs the batch domain one time. Gives 0, or -1 when a node failed. */
int patina_run(PatinaPackage *package);

/* Runs one frame of the frame domain. Gives 0, or -1 when a node failed or the graph has no frame nodes. */
int patina_frame(PatinaPackage *package);

/* The Debug text of the output "<node ID>.<output key>" after the last run or frame. Only an output
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
