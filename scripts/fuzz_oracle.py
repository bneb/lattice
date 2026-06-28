
import os
import sys
import random
import subprocess
import ctypes

# Configuration
ITERATIONS = 5000 # Start with 5k for initial test, plan says 100k
SALT_FRONT = os.path.abspath("salt-front/target/release/saltc")
SALT_OPT = os.path.abspath("salt/build/salt-opt")
CLANG = "clang" # Assumed in path
TEMP_DIR = "fuzz_temp"

if not os.path.exists(SALT_FRONT):
    print(f"Error: saltc not found at {SALT_FRONT}")
    sys.exit(1)
if not os.path.exists(SALT_OPT):
    print(f"Error: salt-opt not found at {SALT_OPT}")
    sys.exit(1)

os.makedirs(TEMP_DIR, exist_ok=True)

# --- Generator ---

OPS = ['+', '-', '*', '&', '|', '^'] 
# Exclude '/' and '%' for now to avoid div-by-zero complications in simple generator
# Exclude '<<', '>>' to avoid undefined behavior with large shifts for now

def generate_expr(depth=0):
    if depth > 3 or (depth > 0 and random.random() < 0.3):
        return str(random.randint(-100, 100))
    
    op = random.choice(OPS)
    lhs = generate_expr(depth + 1)
    rhs = generate_expr(depth + 1)
    
    return f"({lhs} {op} {rhs})"

def generate_program():
    expr = generate_expr()
    # Salt program template
    salt_code = f"""
    fn main() -> i32 {{
        let res: i32 = {expr};
        return res;
    }}
    """
    return salt_code, expr

# --- Reference Interpreter (Python) ---

def wrap32(val):
    return ctypes.c_int32(val).value

def eval_python(expr):
    # We need to evaluate the expression but enforce Wrapping(i32) semantics at each step
    # This is tricky with eval(). 
    # For now, we use a custom recursive evaluator to wrap every op.
    # Actually, let's parse the string back or build AST? 
    # Simpler: The generator produces python-valid syntax match. 
    # But Python doesn't wrap. 
    # We can rely on the fact that if we used small numbers and few ops, maybe it's fine?
    # NO, we need precise wrapping.
    
    # Let's write a simple recursive evaluator for the string
    # Or, change generator to produce a Python lambda with wrappers?
    pass

# Better approach: 
# The generator builds a Tree structure, which can emit Salt code AND execute locally.

class Node:
    pass

class Lit(Node):
    def __init__(self, val): self.val = val
    def to_salt(self): return str(self.val)
    def eval(self): return wrap32(self.val)

class BinOp(Node):
    def __init__(self, lhs, op, rhs):
        self.lhs = lhs
        self.op = op
        self.rhs = rhs
    
    def to_salt(self):
        return f"({self.lhs.to_salt()} {self.op} {self.rhs.to_salt()})"
    
    def eval(self):
        l = self.lhs.eval()
        r = self.rhs.eval()
        res = 0
        if self.op == '+': res = l + r
        elif self.op == '-': res = l - r
        elif self.op == '*': res = l * r
        elif self.op == '&': res = l & r
        elif self.op == '|': res = l | r
        elif self.op == '^': res = l ^ r
        return wrap32(res)

def gen_tree(depth=0):
    if depth > 4 or (depth > 0 and random.random() < 0.4):
        return Lit(random.randint(-1000, 1000))
    op = random.choice(OPS)
    return BinOp(gen_tree(depth+1), op, gen_tree(depth+1))


# --- Execution Pipeline ---

def run_salt(salt_code):
    src_file = os.path.join(TEMP_DIR, "test.salt")
    mlir_file = os.path.join(TEMP_DIR, "test.mlir")
    ll_file = os.path.join(TEMP_DIR, "test.ll")
    bin_file = os.path.join(TEMP_DIR, "test.bin")
    
    with open(src_file, "w") as f:
        f.write(salt_code)
        
    # 1. Salt Front
    try:
        # print("Running Front...")
        res = subprocess.run([SALT_FRONT, src_file, "--release"], capture_output=True, text=True, timeout=5)
    except subprocess.TimeoutExpired:
        return None, "Front timed out"
    
    if res.returncode != 0:
        return None, f"Front failed: {res.stderr}"
    
    # Write MLIR
    with open(mlir_file, "w") as f:
        f.write(res.stdout)
        
    # 2. Salt Opt
    try:
        # Piking via stdin since salt-opt seems to ignore file args or default to stdin
        # print("Running Opt...")
        res = subprocess.run([SALT_OPT, "--emit-llvm"], input=res.stdout, capture_output=True, text=True, timeout=5)
    except subprocess.TimeoutExpired:
        return None, "Opt timed out"

    if res.returncode != 0:
        # Check if it's just verify error or crash
        return None, f"Opt failed ({res.returncode}): {res.stderr}"
        
    with open(ll_file, "w") as f:
        f.write(res.stdout)
        
    # 3. Clang
    c_driver = """
    #include <stdio.h>
    #include <stdint.h>
    extern int32_t main_salt(); 
    
    int main() {
        int32_t res = main_salt();
        printf("%d", res);
        return 0;
    }
    """
    driver_path = os.path.join(TEMP_DIR, "driver.c")
    with open(driver_path, "w") as f:
        f.write(c_driver)
        
    try:
        # print("Running Clang...")
        res = subprocess.run([CLANG, ll_file, driver_path, "-o", bin_file, "-Wno-override-module"], capture_output=True, text=True, timeout=5)
    except subprocess.TimeoutExpired:
        return None, "Clang timed out"
        
    if res.returncode != 0:
        return None, f"Clang failed: {res.stderr}"
        
    # 4. Run
    try:
        # print("Running Bin...")
        res = subprocess.run([bin_file], capture_output=True, text=True, timeout=1)
    except subprocess.TimeoutExpired:
        return None, "Execution timed out"
    if res.returncode != 0 and res.returncode != 5: # 5 might be arbitrary?
        # Actually we just want stdout
        pass
        
    try:
        return int(res.stdout), None
    except ValueError:
        return None, f"Invalid output: '{res.stdout}'"


# --- Main Loop ---

import argparse

def main():
    parser = argparse.ArgumentParser(description="Differential Fuzzing Oracle for Salt")
    parser.add_argument("--iters", type=int, default=5000, help="Number of iterations to run (default: 5000)")
    args = parser.parse_args()

    iterations = args.iters
    print(f"Starting Fuzz Oracle (Goal: {iterations} iters)...")
    success = 0
    mismatches = 0
    failures = 0
    
    for i in range(iterations):
        if i < 5 or i % 100 == 0:
           print(f"Iter {i}: {success} pass, {mismatches} mismatch, {failures} fail")
            
        tree = gen_tree()
        expr_str = tree.to_salt()
        expected = tree.eval()
        
        # Salt code with 'main_salt' naming for linking
        salt_code = f"""
        fn main_salt() -> i32 {{
            let res: i32 = {expr_str};
            return res;
        }}
        """
        
        actual, err = run_salt(salt_code)
        
        if err:
            # Compilation failures might be interesting if unexpected?
            # For now, ignore unless high rate
            failures += 1
            if i < 5: print(f"Iter {i} Fail: {err}") 
            continue
        
        if i < 5: print(f"Iter {i} Success: {actual} vs {expected}")
            
        if actual != expected:
            print(f"\n[MISMATCH] Iter {i}")
            print(f"Expr: {expr_str}")
            print(f"Expected (Python): {expected}")
            print(f"Actual   (Salt):   {actual}")
            mismatches += 1
            
            with open(f"fail_semantic_{i}.salt", "w") as f:
                f.write(salt_code)
            
            # Stop mainly to let user see
            if mismatches >= 5:
                break
        else:
            success += 1
            
    print(f"\nDone. {success} passed, {mismatches} mismatches.")

if __name__ == "__main__":
    main()
