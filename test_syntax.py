import subprocess
with open("kernel/sys/nvme_block.salt", "r") as f:
    lines = f.readlines()

for i in range(len(lines)):
    test_lines = lines[:i+1]
    with open("/tmp/test.salt", "w") as f:
        f.writelines(test_lines)
    res = subprocess.run(["/Users/kevin/projects/lattice/salt-front/target/release/salt-front", "/tmp/test.salt", "--lib"], capture_output=True, text=True)
    if "Error: expected `,`" in res.stderr or "Error: expected `,`" in res.stdout:
        print(f"Error on line {i+1}:\n{lines[i]}")
        break
