// keuos_train.c - C Baseline for Training Comparison
//
// This is the "opponent" for Salt's KeuOS Training.
// Uses standard malloc/free and naive loops.

#include <fcntl.h>
#include <mach/mach_time.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>

#define INPUT_SIZE 784
#define HIDDEN_SIZE 128
#define OUTPUT_SIZE 10
#define TRAIN_COUNT 60000
#define EPOCHS 8 // Match Salt
#define LEARNING_RATE 0.001

// Timing
static mach_timebase_info_data_t timebase_info;
static int timebase_initialized = 0;

int64_t bench_clock(void) {
  if (!timebase_initialized) {
    mach_timebase_info(&timebase_info);
    timebase_initialized = 1;
  }
  uint64_t ticks = mach_absolute_time();
  return (int64_t)(ticks * timebase_info.numer / timebase_info.denom);
}

// ReLU
static inline float relu(float x) { return x > 0.0 ? x : 0.0; }

static inline float relu_grad(float x) { return x > 0.0 ? 1.0 : 0.0; }

// Xavier initialization with explicit scale
void init_weights(float *w, int count, float scale, unsigned int *seed) {
  for (int i = 0; i < count; i++) {
    *seed = (*seed * 1103515245 + 12345) % 2147483647;
    float r = ((int)(*seed % 2000) - 1000) / 1000.0f;
    w[i] = r * scale;
  }
}

int main() {
  printf("=== C Baseline Training ===\n");

  // Load MNIST via mmap
  printf("Loading MNIST via MMAP...\n");

  int fd_images = open("mnist_train_images.bin", O_RDONLY);
  int fd_labels = open("mnist_train_labels.bin", O_RDONLY);

  if (fd_images < 0 || fd_labels < 0) {
    fprintf(stderr, "Error: Could not open training data\n");
    fprintf(stderr, "Run: python3 prepare_data.py  first\n");
    return 1;
  }

  size_t images_size = TRAIN_COUNT * INPUT_SIZE * sizeof(float);
  float *train_images =
      mmap(NULL, images_size, PROT_READ, MAP_PRIVATE, fd_images, 0);
  uint8_t *train_labels =
      mmap(NULL, TRAIN_COUNT, PROT_READ, MAP_PRIVATE, fd_labels, 0);

  // Allocate weights with malloc
  printf("Initializing weights (Xavier)...\n");

  float *w1 = malloc(HIDDEN_SIZE * INPUT_SIZE * sizeof(float));
  float *b1 = calloc(HIDDEN_SIZE, sizeof(float));
  float *w2 = malloc(OUTPUT_SIZE * HIDDEN_SIZE * sizeof(float));
  // REMOVED: W2T - was causing numerical drift. Now use W2 directly with
  // transposed access.
  float *b2 = calloc(OUTPUT_SIZE, sizeof(float));

  // No gradient buffers needed - using fused SGD with direct updates

  // Activation buffers
  float *pre_relu = malloc(HIDDEN_SIZE * sizeof(float));
  float *hidden = malloc(HIDDEN_SIZE * sizeof(float));
  float *output = malloc(OUTPUT_SIZE * sizeof(float));
  float *delta_out = malloc(OUTPUT_SIZE * sizeof(float));
  float *delta_hidden = malloc(HIDDEN_SIZE * sizeof(float));

  // Proper Xavier initialization: sqrt(6 / (fan_in + fan_out))
  float xavier_w1 = sqrtf(6.0f / (INPUT_SIZE + HIDDEN_SIZE));  // ~0.081
  float xavier_w2 = sqrtf(6.0f / (HIDDEN_SIZE + OUTPUT_SIZE)); // ~0.209

  unsigned int seed = 42;
  init_weights(w1, HIDDEN_SIZE * INPUT_SIZE, xavier_w1, &seed);
  seed = 123;
  init_weights(w2, OUTPUT_SIZE * HIDDEN_SIZE, xavier_w2, &seed);

  int64_t total_start = bench_clock();

  // Convergence tracking
  int prev_accuracy = 0;
  int plateau_count = 0;
  int converged = 0;

  // Confusion Matrix (Inline to match Salt work)
  float confusion_matrix[100] = {0};

  // Training loop (max 100 epochs, will stop on convergence)
  for (int epoch = 0; epoch < EPOCHS && !converged; epoch++) {
    memset(confusion_matrix, 0, sizeof(confusion_matrix)); // Reset per epoch
    int64_t epoch_start = bench_clock();
    int correct = 0;

    for (int sample = 0; sample < TRAIN_COUNT; sample++) {
      float *input = &train_images[sample * INPUT_SIZE];
      uint8_t label = train_labels[sample];

      // Forward pass Layer 1
      for (int i = 0; i < HIDDEN_SIZE; i++) {
        float sum = b1[i];
        for (int j = 0; j < INPUT_SIZE; j++) {
          sum += w1[i * INPUT_SIZE + j] * input[j];
        }
        pre_relu[i] = sum;
        hidden[i] = relu(sum);
      }

      // Forward pass Layer 2
      for (int i = 0; i < OUTPUT_SIZE; i++) {
        float sum = b2[i];
        for (int j = 0; j < HIDDEN_SIZE; j++) {
          sum += w2[i * HIDDEN_SIZE + j] * hidden[j];
        }
        output[i] = sum;
      }

      // Argmax
      int pred = 0;
      float max_val = output[0];
      for (int i = 1; i < OUTPUT_SIZE; i++) {
        if (output[i] > max_val) {
          max_val = output[i];
          pred = i;
        }
      }

      if (pred == label)
        correct++;

      // Update Confusion Matrix
      if (label >= 0 && label < 10 && pred >= 0 && pred < 10) {
        confusion_matrix[label * 10 + pred] += 1.0f;
      }

      // Backward pass: softmax-CE gradient
      float max_out = output[0];
      for (int i = 1; i < OUTPUT_SIZE; i++) {
        if (output[i] > max_out)
          max_out = output[i];
      }

      float exp_sum = 0.0;
      for (int i = 0; i < OUTPUT_SIZE; i++) {
        delta_out[i] = expf(output[i] - max_out); // Use expf for performance
        exp_sum += delta_out[i];
      }

      for (int i = 0; i < OUTPUT_SIZE; i++) {
        float softmax = delta_out[i] / exp_sum;
        float target = (i == label) ? 1.0 : 0.0;
        delta_out[i] = softmax - target;
      }

      // FUSED SGD: Direct weight updates
      float lr = LEARNING_RATE;

      // Compute delta_hidden using W2 with transposed access (matching Salt)
      // W2[i, j] accessed as W2[i * HIDDEN_SIZE + j], we need W2.T[j, i] =
      // W2[i, j]
      for (int j = 0; j < HIDDEN_SIZE; j++) {
        float sum = 0.0;
        for (int i = 0; i < OUTPUT_SIZE; i++) {
          sum +=
              w2[i * HIDDEN_SIZE + j] * delta_out[i]; // Transposed access of W2
        }
        delta_hidden[j] = sum * relu_grad(pre_relu[j]);
      }

      // Update Layer 2 weights (W2)
      for (int i = 0; i < OUTPUT_SIZE; i++) {
        float d = delta_out[i];
        b2[i] -= lr * d;
        float update = lr * d;
        for (int j = 0; j < HIDDEN_SIZE; j++) {
          w2[i * HIDDEN_SIZE + j] -= update * hidden[j];
        }
      }

      // REMOVED: W2T update - was causing numerical drift

      // Update Layer 1 weights
      for (int i = 0; i < HIDDEN_SIZE; i++) {
        float d = delta_hidden[i];
        b1[i] -= lr * d;
        for (int j = 0; j < INPUT_SIZE; j++) {
          w1[i * INPUT_SIZE + j] -= lr * d * input[j];
        }
      }
    }

    int64_t epoch_end = bench_clock();
    int64_t epoch_ms = (epoch_end - epoch_start) / 1000000;
    int accuracy = (correct * 100) / TRAIN_COUNT;

    printf("Epoch %d: %d%% accuracy (%lld ms)\n", epoch + 1, accuracy,
           epoch_ms);

    // Convergence check: stop if <1% improvement for 5 epochs
    if (accuracy - prev_accuracy < 1) {
      plateau_count++;
      if (plateau_count >= 5) {
        printf("Converged (plateau detected)\n");
        converged = 1;
      }
    } else {
      plateau_count = 0;
    }
    prev_accuracy = accuracy;
  }

  int64_t total_end = bench_clock();
  int64_t total_ms = (total_end - total_start) / 1000000;

  printf("\n=== Training Complete ===\n");
  printf("Total time: %lld ms\n", total_ms);

  // ========== Test Set Evaluation ==========
  printf("\n=== Test Set Evaluation ===\n");

  int fd_test_images = open("mnist_test_images.bin", O_RDONLY);
  int fd_test_labels = open("mnist_test_labels.bin", O_RDONLY);

  if (fd_test_images < 0 || fd_test_labels < 0) {
    printf("Warning: Test data not found, skipping evaluation\n");
  } else {
    size_t test_images_size = 10000 * INPUT_SIZE * sizeof(float);
    float *test_images =
        mmap(NULL, test_images_size, PROT_READ, MAP_PRIVATE, fd_test_images, 0);
    uint8_t *test_labels =
        mmap(NULL, 10000, PROT_READ, MAP_PRIVATE, fd_test_labels, 0);

    // Per-class counters: true positives, false positives, false negatives
    int tp[OUTPUT_SIZE] = {0}, fp[OUTPUT_SIZE] = {0}, fn[OUTPUT_SIZE] = {0};
    int test_correct = 0;

    for (int i = 0; i < 10000; i++) {
      float *input = &test_images[i * INPUT_SIZE];
      int label = test_labels[i];

      // Forward pass Layer 1
      for (int j = 0; j < HIDDEN_SIZE; j++) {
        float sum = b1[j];
        for (int k = 0; k < INPUT_SIZE; k++) {
          sum += w1[j * INPUT_SIZE + k] * input[k];
        }
        hidden[j] = relu(sum);
      }

      // Forward pass Layer 2
      int pred = 0;
      float max_val = -1e9;
      for (int j = 0; j < OUTPUT_SIZE; j++) {
        float sum = b2[j];
        for (int k = 0; k < HIDDEN_SIZE; k++) {
          sum += w2[j * HIDDEN_SIZE + k] * hidden[k];
        }
        output[j] = sum;
        if (sum > max_val) {
          max_val = sum;
          pred = j;
        }
      }

      if (pred == label) {
        test_correct++;
        tp[label]++;
      } else {
        fp[pred]++;
        fn[label]++;
      }
    }

    // Compute metrics
    float accuracy = (float)test_correct / 10000.0f * 100.0f;
    float total_precision = 0, total_recall = 0, total_f1 = 0;
    int valid_classes = 0;

    printf("\nClass  Precision  Recall     F1\n");
    printf("------------------------------------\n");

    for (int c = 0; c < OUTPUT_SIZE; c++) {
      float prec = (tp[c] + fp[c] > 0) ? (float)tp[c] / (tp[c] + fp[c]) : 0;
      float rec = (tp[c] + fn[c] > 0) ? (float)tp[c] / (tp[c] + fn[c]) : 0;
      float f1 = (prec + rec > 0) ? 2 * prec * rec / (prec + rec) : 0;

      printf("  %d    %.4f     %.4f     %.4f\n", c, prec, rec, f1);

      if (tp[c] + fp[c] + fn[c] > 0) {
        total_precision += prec;
        total_recall += rec;
        total_f1 += f1;
        valid_classes++;
      }
    }

    printf("------------------------------------\n");
    printf("Macro  %.4f     %.4f     %.4f\n", total_precision / valid_classes,
           total_recall / valid_classes, total_f1 / valid_classes);

    printf("\n=== Summary ===\n");
    printf("Test Accuracy:    %.2f%%\n", accuracy);
    printf("Macro Precision:  %.4f\n", total_precision / valid_classes);
    printf("Macro Recall:     %.4f\n", total_recall / valid_classes);
    printf("Macro F1:         %.4f\n", total_f1 / valid_classes);
    printf("Training Time:    %lld ms\n", total_ms);

    munmap(test_images, test_images_size);
    munmap(test_labels, 10000);
  }

  // Cleanup
  munmap(train_images, images_size);
  munmap(train_labels, TRAIN_COUNT);
  free(w1);
  free(b1);
  free(w2);
  // REMOVED: free(w2t) - no longer used
  free(b2);
  free(pre_relu);
  free(hidden);
  free(output);
  free(delta_out);
  free(delta_hidden);

  return 0;
}
