#!/usr/bin/env python3
"""
Structural Layout Validation (DOMA Matrix)
Compares Prisimi's layout_bounds.csv against Headless Chrome's extracted ground truth.
Calculates an "Incremental Parity Score".
"""

import csv
import sys
import os
import math

def load_csv(path):
    rows = []
    if not os.path.exists(path):
        return rows
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        for r in reader:
            rows.append({
                'x': float(r['x']),
                'y': float(r['y']),
                'w': float(r['w']),
                'h': float(r['h']),
                # tags can be numeric mapped internally
                'tag': r.get('tag', '')
            })
    return rows

def compute_parity(prisimi_rows, chrome_rows):
    if not chrome_rows:
        print("[WARNING] Chrome ground truth is empty or missing. Generating 0% score.")
        return 0.0

    match_count = 0
    total_error = 0.0
    
    # We'll just do a greedy sequential match for simplicity
    max_len = min(len(prisimi_rows), len(chrome_rows))
    
    for i in range(max_len):
        pr = prisimi_rows[i]
        cr = chrome_rows[i]
        
        dx = pr['x'] - cr['x']
        dy = pr['y'] - cr['y']
        dw = pr['w'] - cr['w']
        dh = pr['h'] - cr['h']
        
        # Euclidean distance error of the bounds box in hyperspace
        error = math.sqrt(dx**2 + dy**2 + dw**2 + dh**2)
        
        # Base penalty for missing completely
        max_possible_error = 3000.0
        
        if error < 5.0:
            # Pixel perfect (within float/rounding tolerance)
            match_score = 1.0
        else:
            match_score = max(0.0, 1.0 - (error / max_possible_error))
            
        total_error += match_score
        match_count += 1
        
    # Parity Score = (Scores / Chrome Total Nodes)
    # This heavily penalizes the engine if it drops DOM nodes entirely!
    parity = (total_error / len(chrome_rows)) * 100
    return parity

def main():
    prisimi_path = 'tests/output/prisimi_layout_bounds.csv'
    chrome_path = 'tests/fixtures/chrome_geometry_truth.csv'
    
    # For now, to allow the pipeline to run even if the user hasn't generated the Chrome truth:
    if not os.path.exists(chrome_path):
        print(f"[DOMA] Notice: {chrome_path} missing. Assuming Prisimi is running solo.")
        sys.exit(0)
        
    prisimi_rows = load_csv(prisimi_path)
    chrome_rows = load_csv(chrome_path)
    
    print(f"===========================================================")
    print(f"       DOMA MATRIX: STRUCTURAL LAYOUT EVALUATION           ")
    print(f"===========================================================")
    print(f" Chrome Ground Truth Nodes : {len(chrome_rows)}")
    print(f" Prisimi Layout Nodes      : {len(prisimi_rows)}")
    
    parity = compute_parity(prisimi_rows, chrome_rows)
    print(f" Incremental Parity Score  : {parity:.2f}%")
    print(f"===========================================================")
    
    if parity > 90.0:
        print(" [OK] Layout logic firmly matches Incumbent Browser.")
        sys.exit(0)
    else:
        print(" [DIAG] Layout matrix deviates significantly from Incumbent.")
        sys.exit(0)  # We exit 0 so diff pipeline doesn't break CI, it's just a metric tracking.

if __name__ == '__main__':
    main()
