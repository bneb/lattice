#!/usr/bin/env python3
"""
Pointer Injection Matrix — layout.salt rewriter.
Transforms all dom.ARRAY[idx] references into raw pointer dereferences.
"""

import re
import sys

# Map of array name -> (element_type, stride_bytes, read_cast, write_cast)
ARRAYS = {
    # i32, stride 4
    'LAYOUT_X':           ('i32', 4, '&i32', '&mut i32'),
    'LAYOUT_Y':           ('i32', 4, '&i32', '&mut i32'),
    'LAYOUT_W':           ('i32', 4, '&i32', '&mut i32'),
    'LAYOUT_H':           ('i32', 4, '&i32', '&mut i32'),
    'STYLE_W':            ('i32', 4, '&i32', '&mut i32'),
    'STYLE_H':            ('i32', 4, '&i32', '&mut i32'),
    'STYLE_FLEX_GROW':    ('i32', 4, '&i32', '&mut i32'),
    'STYLE_FLEX_BASIS':   ('i32', 4, '&i32', '&mut i32'),
    'STYLE_PADDING_TOP':  ('i32', 4, '&i32', '&mut i32'),
    'STYLE_PADDING_RIGHT':('i32', 4, '&i32', '&mut i32'),
    'STYLE_PADDING_BOTTOM':('i32', 4, '&i32', '&mut i32'),
    'STYLE_PADDING_LEFT': ('i32', 4, '&i32', '&mut i32'),
    'STYLE_TOP':          ('i32', 4, '&i32', '&mut i32'),
    'STYLE_LEFT':         ('i32', 4, '&i32', '&mut i32'),
    'STYLE_Z_INDEX':      ('i32', 4, '&i32', '&mut i32'),
    
    # u32, stride 4
    'DOM_NODE_TAG':       ('u32', 4, '&u32', '&mut u32'),
    'DOM_TEXT_LEN':        ('u32', 4, '&u32', '&mut u32'),
    'STYLE_PARENT':       ('u32', 4, '&u32', '&mut u32'),
    
    # u8, stride 1
    'DIRTY_LAYOUT':       ('u8', 1, '&u8', '&mut u8'),
    'DIRTY_STYLE':        ('u8', 1, '&u8', '&mut u8'),
    'STYLE_DISPLAY':      ('u8', 1, '&u8', '&mut u8'),
    'STYLE_FLEX_DIR':     ('u8', 1, '&u8', '&mut u8'),
    'STYLE_JUSTIFY_CONTENT': ('u8', 1, '&u8', '&mut u8'),
    'STYLE_ALIGN_ITEMS':  ('u8', 1, '&u8', '&mut u8'),
    'STYLE_W_UNIT':       ('u8', 1, '&u8', '&mut u8'),
    'STYLE_POSITION':     ('u8', 1, '&u8', '&mut u8'),
    'STYLE_OVERFLOW':     ('u8', 1, '&u8', '&mut u8'),
    'STYLE_GRID_COL_COUNT': ('u8', 1, '&u8', '&mut u8'),
    'STYLE_GRID_COL_START': ('u8', 1, '&u8', '&mut u8'),
    'STYLE_GRID_ROW_START': ('u8', 1, '&u8', '&mut u8'),
    'STYLE_TEXT_ALIGN':   ('u8', 1, '&u8', '&mut u8'),
    
    # u64, stride 8
    'DOM_NODE_NEXT_SIBLING': ('u64', 8, '&u64', '&mut u64'),
    'DOM_TEXT_PTR':        ('u64', 8, '&u64', '&mut u64'),
    
    # f32, stride 4
    'STYLE_FONT_SIZE':    ('f32', 4, '&f32', '&mut f32'),
    'LAYOUT_SCROLL_X':    ('f32', 4, '&f32', '&mut f32'),
    'LAYOUT_SCROLL_Y':    ('f32', 4, '&f32', '&mut f32'),
    'STYLE_GRID_COL_VAL': ('f32', 4, '&f32', '&mut f32'),
    'STYLE_GRID_COL_TYPE': ('u8', 1, '&u8', '&mut u8'),
}

def transform_line(line):
    """Transform a single line, replacing dom.ARRAY[idx] patterns."""
    
    # Pattern: dom.ARRAY[expr as usize] = value (WRITE)
    # We need to handle this BEFORE reads
    write_pattern = r'dom\.(\w+)\[([^]]+) as usize\]\s*='
    match = re.search(write_pattern, line)
    if match:
        array_name = match.group(1)
        idx_expr = match.group(2)
        if array_name in ARRAYS:
            _, stride, _, write_cast = ARRAYS[array_name]
            ptr_name = f'P_{array_name}'
            if stride == 1:
                offset = f'({idx_expr} as u64)'
            else:
                offset = f'({idx_expr} as u64) * {stride}'
            replacement = f'*(({ptr_name} + {offset}) as {write_cast}) ='
            line = line[:match.start()] + replacement + line[match.end():]
            return line
    
    # Pattern: dom.ARRAY[expr as usize] (READ)
    read_pattern = r'dom\.(\w+)\[([^]]+) as usize\]'
    
    def replace_read(m):
        array_name = m.group(1)
        idx_expr = m.group(2)
        if array_name in ARRAYS:
            _, stride, read_cast, _ = ARRAYS[array_name]
            ptr_name = f'P_{array_name}'
            if stride == 1:
                offset = f'({idx_expr} as u64)'
            else:
                offset = f'({idx_expr} as u64) * {stride}'
            return f'*(({ptr_name} + {offset}) as {read_cast})'
        return m.group(0)  # Unknown array, leave unchanged
    
    line = re.sub(read_pattern, replace_read, line)
    
    # Replace dom.dom_get_render_first_child_idx -> ext_dom_get_render_first_child_idx
    line = line.replace('dom.dom_get_render_first_child_idx(', 'ext_dom_get_render_first_child_idx(')
    
    # Replace dom.dom_get_next_sibling_idx -> ext_dom_get_next_sibling_idx
    line = line.replace('dom.dom_get_next_sibling_idx(', 'ext_dom_get_next_sibling_idx(')
    
    # Replace dom.get_dom_node_count() -> ext_get_dom_node_count()
    line = line.replace('dom.get_dom_node_count()', 'ext_get_dom_node_count()')
    
    # Replace dom.TAG_TEXT with inline constant 0 (TAG_TEXT = tag 0 for text nodes)
    line = line.replace('dom.TAG_TEXT', '0')
    
    return line


def main():
    filepath = sys.argv[1]
    with open(filepath, 'r') as f:
        lines = f.readlines()
    
    new_lines = []
    changed = 0
    for i, line in enumerate(lines):
        new_line = transform_line(line)
        if new_line != line:
            changed += 1
        new_lines.append(new_line)
    
    with open(filepath, 'w') as f:
        f.writelines(new_lines)
    
    print(f"Transformed {changed} lines in {filepath}")


if __name__ == '__main__':
    main()
