import esprima
import sys
import json

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 reference_js_lexer.py <js_file>")
        sys.exit(1)

    with open(sys.argv[1], 'r') as f:
        source = f.read()

    print("== JS LEXER IR ==")
    
    try:
        tokens = esprima.tokenize(source)
        for token in tokens:
            kind = token.type
            value = token.value
            
            # Map esprima types to Salt IR names
            # Keyword, Identifier, Numeric, Punctuator, String, RegularExpression, Template
            if kind == 'Keyword':
                if value in ['let', 'var', 'const']:
                    print(f"TOKEN Keyword({value.capitalize()})")
                else:
                    print(f"TOKEN Keyword({value})")
            elif kind == 'Identifier':
                print("TOKEN Identifier")
            elif kind == 'Numeric':
                print("TOKEN Number")
            elif kind == 'Punctuator':
                # Map common punctuators
                mapping = {
                    '+': 'Plus',
                    '-': 'Minus',
                    '*': 'Asterisk',
                    '/': 'Slash',
                    '=': 'Assign',
                    '(': 'OpenParen',
                    ')': 'CloseParen',
                    ';': 'SemiColon'
                }
                name = mapping.get(value, value)
                print(f"TOKEN Punctuation({name})")
            else:
                print(f"TOKEN {kind}")
    except Exception as e:
        print(f"ERROR: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
