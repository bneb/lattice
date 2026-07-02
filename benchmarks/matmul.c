#include <stdio.h>
#include <stdlib.h>

void matmul(int n, double *a, double *b, double *c) {
  for (int i = 0; i < n; i++) {
    for (int j = 0; j < n; j++) {
      double sum = 0.0;
      for (int k = 0; k < n; k++) {
        sum += a[i * n + k] * b[k * n + j];
      }
      c[i * n + j] = sum;
    }
  }
}

int main(int argc, char **argv) {
  int n = 1024;
  double *a = (double *)malloc(n * n * sizeof(double));
  double *b = (double *)malloc(n * n * sizeof(double));
  double *c = (double *)malloc(n * n * sizeof(double));

  for (int i = 0; i < n * n; i++) {
    a[i] = 1.0 * argc;
    b[i] = 2.0;
  }

  matmul(n, a, b, c);

  // Checksum to prevent DCE
  double checksum = 0.0;
  for (int i = 0; i < n * n; i++) {
    checksum += c[i];
  }

  if (checksum != (2147483648.0 * argc)) {
    return 1;
  }

  free(a);
  free(b);
  free(c);
  return 0;
}
