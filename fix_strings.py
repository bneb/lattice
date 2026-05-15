import sys

with open('scripts/run_test.sh', 'r') as f:
    lines = f.readlines()

with open('scripts/run_test.sh', 'w') as f:
    for line in lines:
        if 'if grep -q \'sprint6_form_test\' "$SALT_FILE" 2>/dev/null; then' in line:
             f.write('if grep -q \'sprint8_search_e2e_test\' "$SALT_FILE" 2>/dev/null; then\n')
             f.write('    BRIDGES+=("$PROJECT_ROOT/tests/bridges/sprint8_search_e2e_bridge.c")\n')
             f.write('fi\n\n')
             f.write(line)
        else:
             f.write(line)
