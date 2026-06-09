import os

path = '/Users/kevin/projects/lattice/user/test_memory.salt'
with open(path, 'r') as f:
    content = f.read()

content = content.replace('import user.lib.syscall\n', 'import user.lib.syscall\nimport user.std.stdio\n')
content = content.replace('syscall.write(', 'stdio.print_str(')

with open(path, 'w') as f:
    f.write(content)

runner_path = '/Users/kevin/projects/lattice/tools/runner_qemu.py'
with open(runner_path, 'r') as f:
    runner_content = f.read()

runner_content = runner_content.replace(
    'os.path.join(user_dir, "lib", "syscall.salt"),\n            ],\n        },\n        {\n            "name": "ring3_test_b",',
    'os.path.join(user_dir, "lib", "syscall.salt"),\n                os.path.join(user_dir, "std", "stdio.salt"),\n            ],\n        },\n        {\n            "name": "ring3_test_b",'
)

with open(runner_path, 'w') as f:
    f.write(runner_content)

print("Done")
