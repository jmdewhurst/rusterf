
#include "sinusoid_fitting.h"
#include <gsl/gsl_errno.h>

int sinusoid(const gsl_vector *x, void *params, gsl_vector *f) {
  multifit_data_t *data = (multifit_data_t *)params;
  uint32_t n = data->num_points;
  const float *y = data->y;

  FIT_FLOAT_TYPE A = gsl_vector_get(x, 0);
  FIT_FLOAT_TYPE freq = gsl_vector_get(x, 1);
  FIT_FLOAT_TYPE phi = gsl_vector_get(x, 2);
  FIT_FLOAT_TYPE offs = gsl_vector_get(x, 3);

  for (unsigned int i = 0; i < n; i++) {
    FIT_FLOAT_TYPE Yi = A * cos(freq * i - phi) + offs;
    gsl_vector_set(f, i, Yi - y[i]);
  }

  return GSL_SUCCESS;
}

int sinusoid_df(const gsl_vector *x, void *params, gsl_matrix *J) {
  multifit_data_t *data = (multifit_data_t *)params;
  uint32_t n = data->num_points;

  FIT_FLOAT_TYPE A = gsl_vector_get(x, 0);
  FIT_FLOAT_TYPE freq = gsl_vector_get(x, 1);
  FIT_FLOAT_TYPE phi = gsl_vector_get(x, 2);

  for (unsigned int i = 0; i < n; i++) {
    /* Jacobian matrix J(i,j) = dfi / dxj, */
    /* where fi = (Yi - yi),      */
    /*       Yi = A * cos(freq*ti + phi) + offs  */
    /* and the xj are the parameters (A, freq, phi, offs) */
    gsl_matrix_set(J, i, 0, cos(freq * i - phi));
    gsl_matrix_set(J, i, 1, -A * i * sin(freq * i - phi));
    gsl_matrix_set(J, i, 2, A * sin(freq * i - phi));
    gsl_matrix_set(J, i, 3, 1.0);
  }

  return GSL_SUCCESS;
}

int sinusoid_fvv(const gsl_vector *x, const gsl_vector *v, void *params,
                 gsl_vector *fvv) {
  multifit_data_t *data = (multifit_data_t *)params;
  uint32_t n = data->num_points;

  FIT_FLOAT_TYPE a = gsl_vector_get(x, 0);
  FIT_FLOAT_TYPE b = gsl_vector_get(x, 1);
  FIT_FLOAT_TYPE c = gsl_vector_get(x, 2);
  FIT_FLOAT_TYPE va = gsl_vector_get(v, 0);
  FIT_FLOAT_TYPE vb = gsl_vector_get(v, 1);
  FIT_FLOAT_TYPE vc = gsl_vector_get(v, 2);

  for (unsigned int i = 0; i < n; i++) {
    FIT_FLOAT_TYPE cos_part = cos(b * i - c);
    FIT_FLOAT_TYPE sin_part = sin(b * i - c);

    FIT_FLOAT_TYPE Dab = -sin_part * i;
    FIT_FLOAT_TYPE Dbb = -a * i * i * cos_part;
    FIT_FLOAT_TYPE Dac = sin_part;
    FIT_FLOAT_TYPE Dbc = a * i * cos_part;
    FIT_FLOAT_TYPE Dcc = -a * cos_part;

    FIT_FLOAT_TYPE sum;
    sum = vb * vb * Dbb + vc * vc * Dcc + 2.0 * va * vb * Dab +
          2.0 * va * vc * Dac + 2.0 * vb * vc * Dbc;
    gsl_vector_set(fvv, i, sum);
  }

  return GSL_SUCCESS;
}

int sinusoid_b(const gsl_vector *x, void *params, gsl_vector *f) {
  multifit_data_t *data = params;
  uint32_t n = data->num_points;
  const float *y = data->y;

  FIT_FLOAT_TYPE A_cos = gsl_vector_get(x, 0);
  FIT_FLOAT_TYPE A_sin = gsl_vector_get(x, 1);
  FIT_FLOAT_TYPE freq = gsl_vector_get(x, 2);
  FIT_FLOAT_TYPE offs = gsl_vector_get(x, 3);

  for (unsigned int i = 0; i < n; i++) {
    FIT_FLOAT_TYPE Yi = A_cos * cos(freq * i) + A_sin * sin(freq * i) + offs;
    gsl_vector_set(f, i, Yi - y[i]);
  }
  return GSL_SUCCESS;
}

int sinusoid_df_b(const gsl_vector *x, void *params, gsl_matrix *J) {
  multifit_data_t *data = (multifit_data_t *)params;
  uint32_t n = data->num_points;

  FIT_FLOAT_TYPE A_cos = gsl_vector_get(x, 0);
  FIT_FLOAT_TYPE A_sin = gsl_vector_get(x, 1);
  FIT_FLOAT_TYPE freq = gsl_vector_get(x, 2);

  for (unsigned int i = 0; i < n; i++) {
    /* Jacobian matrix J(i,j) = dfi / dxj, */
    /* where fi = (Yi - yi),      */
    /*       Yi = A * cos(freq*ti + phi) + offs  */
    /* and the xj are the parameters (A, freq, phi, offs) */
    gsl_matrix_set(J, i, 0, cos(freq * i));
    gsl_matrix_set(J, i, 1, sin(freq * i));
    gsl_matrix_set(J, i, 2,
                   -A_cos * i * sin(freq * i) + A_sin * i * cos(freq * i));
    gsl_matrix_set(J, i, 3, 1.0);
  }

  return GSL_SUCCESS;
}

int sinusoid_fvv_b(const gsl_vector *x, const gsl_vector *v, void *params,
                   gsl_vector *fvv) {
  multifit_data_t *data = (multifit_data_t *)params;
  uint32_t n = data->num_points;

  FIT_FLOAT_TYPE a = gsl_vector_get(x, 0);
  FIT_FLOAT_TYPE b = gsl_vector_get(x, 1);
  FIT_FLOAT_TYPE w = gsl_vector_get(x, 2);
  FIT_FLOAT_TYPE va = gsl_vector_get(v, 0);
  FIT_FLOAT_TYPE vb = gsl_vector_get(v, 1);
  FIT_FLOAT_TYPE vc = gsl_vector_get(v, 2);

  for (unsigned int i = 0; i < n; i++) {
    float i_f = i;
    FIT_FLOAT_TYPE cos_p = cos(w * i_f);
    FIT_FLOAT_TYPE sin_p = sin(w * i_f);

    FIT_FLOAT_TYPE Dac = -(i_f)*sin_p;
    FIT_FLOAT_TYPE Dbc = (i_f)*cos_p;
    FIT_FLOAT_TYPE Dcc = -a * (i_f * i_f) * cos_p - b * (i_f * i_f) * sin_p;
    FIT_FLOAT_TYPE sum =
        (vc * vc * Dcc) + (2.0 * va * vc * Dac) + (2.0 * vb * vc * Dbc);
    gsl_vector_set(fvv, i, sum);
  }
  return GSL_SUCCESS;
}

multifit_result_raw_t do_fitting(multifit_setup_t *setup,
                                 multifit_data_t data) {
  for (int i = 0; i < 4; i++) {
    gsl_vector_set(setup->guess, i, data.guess[i]);
  }
  int info;
  multifit_result_raw_t result;
  setup->fdf->params = &data;
  gsl_multifit_nlinear_init(setup->guess, setup->fdf, setup->work);
  int status = gsl_multifit_nlinear_driver(setup->max_iterations, setup->xtol,
                                           setup->gtol, setup->ftol, NULL, NULL,
                                           &info, setup->work);
  gsl_vector *coef = gsl_multifit_nlinear_position(setup->work);
  for (int i = 0; i < 4; i++) {
    result.params[i] = gsl_vector_get(coef, i);
  }
  result.gsl_status = status;
  result.niter = gsl_multifit_nlinear_niter(setup->work);

  // calculate chi_squared
  gsl_vector *f;
  double chisq;
  f = gsl_multifit_nlinear_residual(setup->work);
  gsl_blas_ddot(f, f, &chisq);
  result.chisq = (float)chisq;

  // fprintf(stderr, "reason for stopping: %s\n",
  //         (info == 1) ? "small step size" : "small gradient");
  return result;
}

uint32_t init_multifit_setup(multifit_setup_t *setup) {
  setup->fdf = malloc(sizeof(gsl_multifit_nlinear_fdf));
  setup->guess = gsl_vector_alloc(4);

  setup->setup_params = malloc(sizeof(gsl_multifit_nlinear_parameters));
  *(setup->setup_params) = gsl_multifit_nlinear_default_parameters();
  setup->setup_params->trs = gsl_multifit_nlinear_trs_lm;
  // setup->setup_params->trs = gsl_multifit_nlinear_trs_lmaccel;
  setup->setup_params->solver = gsl_multifit_nlinear_solver_cholesky;
  setup->setup_params->scale = gsl_multifit_nlinear_scale_more;
  setup->setup_params->factor_up = 3.;
  setup->setup_params->avmax = setup->max_av_ratio;

  setup->fdf->f = sinusoid_b;
  setup->fdf->df = sinusoid_df_b;
  setup->fdf->fvv = sinusoid_fvv_b;
  // setup->fdf->f = sinusoid;
  // setup->fdf->df = sinusoid_df;
  // setup->fdf->fvv = sinusoid_fvv;
  // setup->fdf->fvv = NULL;
  setup->fdf->n = setup->num_points;
  setup->fdf->p = 4;

  const gsl_multifit_nlinear_type *T = gsl_multifit_nlinear_trust;
  setup->work =
      gsl_multifit_nlinear_alloc(T, setup->setup_params, setup->num_points, 4);

  return 0;
}

void release_multifit_resources(multifit_setup_t *setup) {
  gsl_multifit_nlinear_free(setup->work);
  gsl_vector_free(setup->guess);
  free(setup->fdf);
}
