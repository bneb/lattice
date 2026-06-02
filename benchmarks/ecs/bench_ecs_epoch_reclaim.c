#include <stdio.h>
#include <stdlib.h>

typedef struct {
    int id;
    int gen;
    long long data[4];
} SimulatedEntity;

int main() {
    int iterations = 100000;
    SimulatedEntity **arr = malloc(iterations * sizeof(SimulatedEntity *));
    if (!arr) return 1;
    
    for (int i = 0; i < iterations; i++) {
        arr[i] = malloc(sizeof(SimulatedEntity));
        if (arr[i]) {
            arr[i]->id = i;
            arr[i]->gen = 1;
        }
    }
    
    for (int i = 0; i < iterations; i++) {
        if (arr[i]) {
            free(arr[i]);
        }
    }
    
    free(arr);
    return 0;
}
