with open("lettuce/src/server.salt", "r") as f:
    for line in f:
        if "pub fn main" in line:
            print("Found main")
