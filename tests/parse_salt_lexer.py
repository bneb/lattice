#!/usr/bin/env python3
"""
Parse Salt's interleaved [INFO] lexer output and normalize it.
Salt outputs: [INFO] NODE N=\n1.00[INFO]  T=\n1.00[INFO]  P=\n0.00[INFO] \n
This script joins all text into a stream, removes [INFO] markers, then splits.
"""
import sys
import re

raw = sys.stdin.read()

# Strip all [INFO] markers and join everything
text = raw.replace('[INFO] ', '').replace('[INFO]', '')
# Remove stray newlines that don't start a NODE/TEXT/TOTAL line
text = re.sub(r'\n(?!NODE|TEXT|TOTAL)', '', text)
# Clean up any remaining whitespace issues
text = re.sub(r'\s+', ' ', text).strip()

# Split on NODE, TEXT, TOTAL markers  
parts = re.split(r'(?=NODE N=|TEXT N=|TOTAL_NODES=)', text)

for p in parts:
    p = p.strip()
    if not p:
        continue
    # Only process lines after the SALT LEXER IR header
    if 'SALT LEXER IR' in p:
        continue
    # Convert floats like "1.00" to ints "1"
    p = re.sub(r'(\d+)\.00', lambda m: m.group(1), p)
    # Clean up extra spaces
    p = re.sub(r'\s+', ' ', p).strip()
    print(p)
