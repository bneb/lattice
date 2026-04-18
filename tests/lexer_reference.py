#!/usr/bin/env python3
"""
Reference HTML parser for differential testing against Salt's lex_html.

Outputs a normalized DOM tree in the SAME numeric format as Salt's
test_lexer_diff.salt, so both outputs can be diff'd directly.

Output format:
  NODE N=<idx> T=<tag_id> P=<parent_idx>
  TEXT N=<idx> P=<parent_idx> LEN=<len>
  TOTAL_NODES= <count>

Usage:
  python3 tests/lexer_reference.py tests/fixtures/lexer_test.html
"""

import sys
from html.parser import HTMLParser

# ============================================================
# TAG_MAP — MUST match lexer.salt match_tag() EXACTLY.
# Authoritative source: lexer.salt lines 54-127
# ============================================================
# match_tag returns based on tag name length + byte values.
# Tags NOT in this map return 96 (TAG_CUSTOM_ELEMENT).
TAG_MAP = {
    # len == 1
    'p':        6,   # TAG_P
    'a':        7,   # TAG_A
    'b':        15,  # TAG_B
    'i':        16,  # TAG_I
    'u':        96,  # explicitly returns 96 in match_tag
    # len == 2
    'h1':       9,   # TAG_H1
    'tr':       13,  # TAG_TR
    'td':       14,  # TAG_TD
    # h2, h3 are NOT in match_tag — they fall through to 96
    # len == 3
    'div':      4,   # TAG_DIV
    'img':      8,   # TAG_IMG
    # len == 4
    'span':     5,   # TAG_SPAN
    'html':     1,   # TAG_HTML
    'head':     2,   # TAG_HEAD
    'body':     3,   # TAG_BODY
    'font':     17,  # TAG_FONT
    # len == 5
    'table':    12,  # TAG_TABLE
    'style':    98,  # TAG_STYLE
    'video':    25,  # TAG_VIDEO
    'input':    18,  # TAG_INPUT
    # len == 6
    'script':   99,  # TAG_SCRIPT
    'center':   11,  # TAG_CENTER
    'iframe':   26,  # TAG_IFRAME
    'canvas':   27,  # TAG_CANVAS
    'button':   20,  # TAG_BUTTON
    'strong':   17,  # TAG_STRONG (shares slot with font — Salt line 123)
}

# All tags not in TAG_MAP fall through to 96 (TAG_CUSTOM_ELEMENT).
# This includes: h2, h3, textarea, section, nav, article, header, footer,
# form, label, select, option, ul, ol, li, em, br, hr, meta, link, etc.

# W3C Void Elements — do NOT push onto parent stack.
# Salt checks via is_void_tag_name() and is_void_element().
VOID_ELEMENTS = {
    'br', 'hr', 'img', 'col', 'meta', 'link', 'input',
    'area', 'base', 'embed', 'param', 'source', 'track', 'wbr'
}

# RAW_TEXT elements — their inner content is treated as a single text node
RAW_TEXT_ELEMENTS = {'script', 'style'}


class DOMBuilder(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=False)
        self.next_idx = 2  # 1 is the root (created externally)
        self.parent_stack = [1]  # Root is node 1
        self.output_lines = []
        self.in_raw_text = None  # Track if inside <script> or <style>
        self.text_buffer = ""

    def _flush_text(self):
        if self.text_buffer:
            parent = self.parent_stack[-1]
            idx = self._alloc()
            text_len = len(self.text_buffer.encode('utf-8'))
            self.output_lines.append(f"TEXT N={idx} P={parent} LEN={text_len}")
            self.text_buffer = ""

    def _alloc(self):
        idx = self.next_idx
        self.next_idx += 1
        return idx

    def handle_starttag(self, tag, attrs):
        self._flush_text()
        tag_lower = tag.lower()
        tag_id = TAG_MAP.get(tag_lower, 96)  # 96 = TAG_CUSTOM_ELEMENT default
        parent = self.parent_stack[-1]
        idx = self._alloc()
        self.output_lines.append(f"NODE N={idx} T={tag_id} P={parent}")
        
        if attrs:
            for k, v in attrs:
                v_str = v if v is not None else ""
                self.output_lines.append(f"ATTR N={idx} K={k} V={v_str}")
                
        if tag_lower not in VOID_ELEMENTS:
            self.parent_stack.append(idx)
            if tag_lower in RAW_TEXT_ELEMENTS:
                self.in_raw_text = tag_lower

    def handle_endtag(self, tag):
        self._flush_text()
        tag_lower = tag.lower()
        if tag_lower in VOID_ELEMENTS:
            return
        if self.in_raw_text == tag_lower:
            self.in_raw_text = None
        if self.parent_stack and len(self.parent_stack) > 1:
            self.parent_stack.pop()

    def handle_data(self, data):
        if data:
            self.text_buffer += data

    def handle_entityref(self, name):
        self.text_buffer += f"&{name};"

    def handle_charref(self, name):
        self.text_buffer += f"&#{name};"

    def handle_decl(self, decl):
        self._flush_text()
        pass  # <!DOCTYPE> — skip (matches Salt)

    def handle_comment(self, data):
        self._flush_text()
        pass  # <!-- --> — skip (matches Salt)


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <html_file>", file=sys.stderr)
        sys.exit(1)

    filepath = sys.argv[1]
    if filepath == '-':
        html = sys.stdin.read()
    else:
        with open(filepath, 'rb') as f:
            html = f.read().decode('utf-8', errors='replace')

    # Root is node 1 (TAG_HTML=1), created externally just like Salt
    print("NODE N=1 T=1 P=0")

    parser = DOMBuilder()
    parser.feed(html)
    parser._flush_text() # Flush any remaining text at EOF

    for line in parser.output_lines:
        print(line)

    print(f"TOTAL_NODES= {parser.next_idx}")


if __name__ == '__main__':
    main()
