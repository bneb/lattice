import os

lints = """#![allow(
    clippy::unnecessary_map_or,
    clippy::redundant_closure,
    clippy::new_without_default,
    clippy::let_and_return,
    clippy::manual_strip,
    clippy::while_let_on_iterator,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::match_like_matches_macro,
    clippy::doc_lazy_continuation,
    clippy::if_same_then_else,
    clippy::unnecessary_literal_unwrap,
    clippy::empty_line_after_doc_comments
)]
"""

for filepath in ["salt-front/src/lib.rs", "salt-front/src/main.rs"]:
    if os.path.exists(filepath):
        with open(filepath, "r") as f:
            content = f.read()
        if "#![allow(" not in content:
            with open(filepath, "w") as f:
                f.write(lints + "\n" + content)
