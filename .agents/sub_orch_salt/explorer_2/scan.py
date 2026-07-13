import os
import re

SOURCE_DIR = "/Users/kevin/projects/salt/salt-front/src"
EXCLUDE_FILES = [".bak", "vendor", "test"] # Simple filters to classify test files or backup files

# AI slop/chatty patterns or words
AI_SLOP_PATTERNS = [
    r"\b(here, we|let's first|specifically, we|as you can see|basically|simply|note that|it's important to|we must|we can|we should|this function)\b"
]

HYPERBOLE_WORDS = [
    "disaster", "catastrophe", "nightmare", "insane", "crazy", "terrible", "horrible",
    "brilliant", "amazing", "magic", "magical", "miracle", "stupid", "weird", "bizarre",
    "insanely", "dangerous", "deadly"
]

MUTANTS = ["TODO", "FIXME", "HACK", "XXX", "temp_", "workaround"]

# Code structure heuristics to detect commented out code blocks
# e.g., consecutive comment lines that contain things like let, fn, if, match, loops, or semicolons/braces
CODE_INDICATORS = [
    r"^\s*//\s*(let\s+\w+|fn\s+\w+|if\s+|match\s+|for\s+|loop\s+|while\s+|impl\s+|struct\s+|enum\s+|pub\s+)",
    r"^\s*//\s*.*\;\s*$",
    r"^\s*//\s*.*\b(unwrap|expect|assert_eq|return|Ok|Err)\b",
    r"^\s*//\s*(\{|\})\s*$"
]

def is_test_file(filepath):
    filename = os.path.basename(filepath)
    if "test" in filename:
        return True
    return False

def scan_file(filepath):
    findings = {
        "ai_slop": [],
        "hyperbole": [],
        "mutants": [],
        "commented_out_code": []
    }
    
    with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
        lines = f.readlines()
        
    in_block_comment = False
    commented_out_block = []
    
    for idx, line in enumerate(lines):
        line_num = idx + 1
        stripped = line.strip()
        
        # Detect legacy files or temp files like backups
        if filepath.endswith(".bak"):
            # Whole file is a legacy mutant
            findings["mutants"].append((line_num, line, f"Backup file {filepath} should be removed."))
            continue
            
        # Match single-line comments
        comment_content = None
        is_comment = False
        if stripped.startswith("//"):
            is_comment = True
            comment_content = re.sub(r"^///?/?\s*", "", stripped)
        
        if is_comment and comment_content:
            # Check mutants in non-test files
            if not is_test_file(filepath):
                for mutant in MUTANTS:
                    # Avoid false positives for variable names like temp_map, but if the task asks for all mutants, we flag them
                    if mutant in comment_content or (mutant == "temp_" and "temp_" in line):
                        # Flag mutant
                        findings["mutants"].append((line_num, line.strip(), f"Legacy/illegal mutant word: '{mutant}'"))
            
            # Check hyperbole
            for word in HYPERBOLE_WORDS:
                if re.search(r"\b" + re.escape(word) + r"\b", comment_content, re.IGNORECASE):
                    findings["hyperbole"].append((line_num, line.strip(), f"Hyperbole word: '{word}'"))
            
            # Check AI slop
            for pattern in AI_SLOP_PATTERNS:
                if re.search(pattern, comment_content, re.IGNORECASE):
                    findings["ai_slop"].append((line_num, line.strip(), f"Chatty or AI-like phrasing matching pattern: '{pattern}'"))
            
            # Check if this line looks like commented-out code
            is_code_like = False
            for ind in CODE_INDICATORS:
                if re.match(ind, line):
                    is_code_like = True
                    break
            
            if is_code_like:
                commented_out_block.append((line_num, line.strip()))
            else:
                if len(commented_out_block) >= 1: # even single lines of code-like comments can be reported
                    # Record the block
                    findings["commented_out_code"].append(commented_out_block)
                    commented_out_block = []
        else:
            # Non-comment line or empty line
            if len(commented_out_block) >= 1:
                findings["commented_out_code"].append(commented_out_block)
                commented_out_block = []
            
            # If the code line contains temp_ (as variables) and it is non-test file, flag it as mutant (since task bans temp_ in non-test files)
            if not is_test_file(filepath):
                if "temp_" in stripped:
                    # Ensure it's not a false positive but let's check
                    findings["mutants"].append((line_num, line.strip(), "Contains 'temp_' in variable name or expression"))
                    
    # Handle end of file block
    if len(commented_out_block) >= 1:
        findings["commented_out_code"].append(commented_out_block)
        
    return findings

def main():
    all_findings = []
    
    for root, dirs, files in os.walk(SOURCE_DIR):
        # Exclude vendor dependencies if any
        if "vendor" in root:
            continue
        for file in files:
            filepath = os.path.join(root, file)
            # Scan files with .rs and also look for .bak files
            if file.endswith(".rs") or file.endswith(".bak"):
                file_findings = scan_file(filepath)
                
                # Format findings
                relative_path = os.path.relpath(filepath, "/Users/kevin/projects/salt")
                
                for key, items in file_findings.items():
                    for item in items:
                        if key == "commented_out_code":
                            # item is a list of (line_num, text)
                            start_line = item[0][0]
                            end_line = item[-1][0]
                            block_text = "\n".join([t[1] for t in item])
                            all_findings.append({
                                "file": relative_path,
                                "type": key,
                                "line": f"{start_line}-{end_line}" if start_line != end_line else str(start_line),
                                "text": block_text,
                                "details": "Commented-out or unused code block"
                            })
                        else:
                            line_num, text, details = item
                            all_findings.append({
                                "file": relative_path,
                                "type": key,
                                "line": str(line_num),
                                "text": text,
                                "details": details
                            })
                            
    # Print summary or save to file
    print(f"Total findings: {len(all_findings)}")
    import json
    with open("raw_findings.json", "w") as f:
        json.dump(all_findings, f, indent=2)

if __name__ == "__main__":
    main()
