
#include <stdbool.h>
#include <stdio.h>

#define N 9

bool can_place(int board[81], int row, int col, int num) {
  for (int x = 0; x < 9; x++) {
    if (board[row * 9 + x] == num || board[x * 9 + col] == num) {
      return false;
    }
  }

  int startRow = row - row % 3;
  int startCol = col - col % 3;

  for (int i = 0; i < 3; i++) {
    for (int j = 0; j < 3; j++) {
      if (board[(i + startRow) * 9 + (j + startCol)] == num) {
        return false;
      }
    }
  }

  return true;
}

bool solve_sudoku(int board[81]) {
  for (int i = 0; i < 81; i++) {
    if (board[i] == 0) {
      int row = i / 9;
      int col = i % 9;
      for (int num = 1; num <= 9; num++) {
        if (can_place(board, row, col, num)) {
          board[i] = num;
          if (solve_sudoku(board)) {
            return true;
          }
          board[i] = 0;
        }
      }
      return false;
    }
  }
  return true;
}

int main() {
  int total_solved = 0;

  for (int k = 0; k < 600; k++) {
    int board[81] = {3, 0, 6, 5, 0, 8, 4, 0, 0, 5, 2, 0, 0, 0, 0, 0, 0, 0,
                       0, 8, 7, 0, 0, 0, 0, 3, 1, 0, 0, 3, 0, 1, 0, 0, 8, 0,
                       9, 0, 0, 8, 6, 3, 0, 0, 5, 0, 5, 0, 0, 9, 0, 6, 0, 0,
                       1, 3, 0, 0, 0, 0, 2, 5, 0, 0, 0, 0, 0, 0, 0, 0, 7, 4,
                       0, 0, 5, 2, 0, 6, 3, 0, 0};

    if (solve_sudoku(board)) {
      total_solved++;
    }
  }

  return total_solved != 600;
}
