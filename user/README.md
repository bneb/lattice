# User Space

**The Mission:** The "Ring 3" applications that give KeuOS its personality.

## Overview
KeuOS user space is unique: it is built using the same language and primitives (`Region`, `Channel`) as the kernel, but runs with restricted privileges.

## Components

| Directory | Role | Status |
|-----------|------|--------|
| [`facet/`](./facet) | **The Compositor.** A resolution-independent vector UI server. | Planning |
| [`grit/`](./grit) | **The Shell.** A structural, object-oriented shell/REPL. | Planning |
