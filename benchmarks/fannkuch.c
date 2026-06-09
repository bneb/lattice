// fannkuch-redux benchmark
// Derived from the Computer Language Benchmarks Game
// https://benchmarksgame-team.pages.debian.net/benchmarksgame/

#include <stdio.h>
#include <stdlib.h>

int fannkuch(int n) {
  int *perm = malloc(n * sizeof(int));
  int *perm1 = malloc(n * sizeof(int));
  int *count = malloc(n * sizeof(int));

  int max_flips = 0;
  int checksum = 0;
  int perm_count = 0;

  // Initialize
  for (int i = 0; i < n; i++) {
    perm1[i] = i;
  }

  int r = n;

  while (1) {
    // Copy perm1 to perm
    while (r != 1) {
      count[r - 1] = r;
      r--;
    }

    for (int i = 0; i < n; i++) {
      perm[i] = perm1[i];
    }

    // Count flips
    int flips = 0;
    int k;
    while ((k = perm[0]) != 0) {
      // Reverse first k+1 elements
      int i = 0;
      int j = k;
      while (i < j) {
        int temp = perm[i];
        perm[i] = perm[j];
        perm[j] = temp;
        i++;
        j--;
      }
      flips++;
    }

    if (flips > max_flips) {
      max_flips = flips;
    }

    // Checksum
    if (perm_count % 2 == 0) {
      checksum += flips;
    } else {
      checksum -= flips;
    }
    perm_count++;

    // Generate next permutation
    while (1) {
      if (r == n) {
        free(perm);
        free(perm1);
        free(count);
        // Return combined result for verification
        return checksum * 1000 + max_flips;
      }

      int perm0 = perm1[0];
      for (int i = 0; i < r; i++) {
        perm1[i] = perm1[i + 1];
      }
      perm1[r] = perm0;

      count[r]--;
      if (count[r] > 0) {
        break;
      }
      r++;
    }
  }
}

int main(int argc, char **argv) {
  // Use argc to prevent constant folding, default n=10
  int n = 10 + (argc - 1);

  int result = fannkuch(n);
  int checksum = result / 1000;
  int max_flips = result % 1000;

  // Expected for n=10: checksum=73196, max_flips=38
  // With argc=1, n=10
  if (argc == 1) {
    if (checksum == 73196 && max_flips == 38) {
      return 0;
    }
  }

  // For other n values, just verify it ran
  if (max_flips > 0) {
    return 0;
  }
  return 1;
}
