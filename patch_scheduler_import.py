import sys

with open("user/reactor/scheduler.salt", "r") as f:
    content = f.read()

content = content.replace("package user.reactor.scheduler", "package user.reactor.scheduler\n\nimport user.os.process")
content = content.replace("user.os.process.mmap_shared", "process.mmap_shared")

with open("user/reactor/scheduler.salt", "w") as f:
    f.write(content)
