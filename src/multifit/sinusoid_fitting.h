
#ifndef _SINUSOID_FITTING_H
#define _SINUSOID_FITTING_H

#include "gsl/gsl_blas.h"
#include "gsl/gsl_matrix.h"
#include "gsl/gsl_multifit_nlinear.h"
#include "gsl/gsl_randist.h"
#include "gsl/gsl_rng.h"
#include "gsl/gsl_vector.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define FIT_FLOAT_TYPE float

typedef struct {
  uint32_t num_points;
  uint32_t skip_rate;
  const float *y; // the oscilloscope data
  float guess[4];
} multifit_data_t;

typedef struct {
  int gsl_status;
  int niter;
  float params[4];
} multifit_result_raw_t;

typedef struct {
  gsl_multifit_nlinear_workspace *work;
  gsl_multifit_nlinear_fdf *fdf;
  gsl_multifit_nlinear_parameters *setup_params;
  gsl_vector *guess;
  uint32_t skip_rate;
  uint32_t num_points;
  uint32_t max_iterations;
  float xtol;
  float gtol;
  float ftol;
  float max_av_ratio;
} multifit_setup_t;

int sinusoid(const gsl_vector *x, void *params, gsl_vector *f);
int sinusoid_df(const gsl_vector *x, void *params, gsl_matrix *J);
int sinusoid_fvv(const gsl_vector *x, const gsl_vector *v, void *params,
                 gsl_vector *fvv);

uint32_t init_multifit_setup(multifit_setup_t *setup);
void release_multifit_resources(multifit_setup_t *setup);
multifit_result_raw_t do_fitting(multifit_setup_t *setup, multifit_data_t data);
#endif
