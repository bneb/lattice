# Handoff Report: Facet UI Directory Audit (AI Slop, Hyperbole, and Legacy Artifacts)

This report presents the findings of a read-only quality and standards audit performed on the `ui/` directory of the `facet` repository.

---

## 1. Observation

A full scan was conducted on the `/Users/kevin/projects/facet/ui` directory. The directory contains exactly three files:
1. `text.salt` (90 lines) - Core UI text path generation.
2. `widget.salt` (259 lines) - Core UI retained-mode widget tree & layouts.
3. `test_ui.salt` (760 lines) - UI Counter App Demo / Test harness.

### Specific Findings

#### A. AI Slop (Conversational / LLM Monologue Comments)

Conversational "thinking out loud" commentary was identified in all three files:

*   **`text.salt` (Lines 6-7, 10, 14)**:
    ```salt
    // Each char is 5 bytes (columns). Bit 0 is top row? No, usually LSB=Top or Bottom.
    // Let's adopt LSB=Top (y=0). 7 bits used.
    ...
    // x..x  51 (01010001) - wait, this encoding is vertical stripes
    ...
    // Yes, standard 5-byte column vertical encoding.
    ```
*   **`widget.salt` (Lines 138-148, 159, 212-218)**:
    ```salt
    // Note: Node transforms are applied recursively. 
    // We can either set `group.transform` or just pass accum coords.
    // Let's use `group.transform` for positioning the widget itself?
    // Actually, `widget_layout` computed relative positions.
    // If we use translation node for each widget, it simplifies logic.
    // BUT we shouldn't nest too deeply. 
    // Let's translate the group to (my_x, my_y).
    // Wait, caller passes absolute offset? Or relative accumulates?
    // Let's say `widget_build` returns a Node representing the widget at (0,0) local?
    // No, layout computed positions.
    // `widget_build` should return a Node positioned correctly if it's a child.
    ...
    // We might center it? layout says top-left.
    ...
    // Simple AABB check against (0,0, w,h) in local space?
    // We need to transform coordinates as we descend!
    // But here we don't have the transform stack.
    // "Retained Mode" usually keeps absolute coords or re-traverses layout.
    // For this simple demo: We assume layout pass set `rect` (size).
    // But positions are relative.
    // We need to pass `rel_mx, rel_my`.
    ```
*   **`test_ui.salt` (Lines 735-748)**:
    ```salt
    // Manual cleanup using facet_free_node logic recursively?
    // Wait, I only have shallow facet_free_node.
    // I need recursive free for the scene graph.
    // I'll implement `node_free_tree(n)` helper here, duplicating logic from test_compositor?
    // No, I'll allow a leak per frame for this demo since it's short lived?
    // No, 3000 frames will explode memory.
    
    // Inline naive recursive free for now or add helper to Part 3.
    // I'll use a hack to free just the top level constructed nodes since widgets are reused?
    // No, widget_build Creates NEW Nodes every frame.
    // They must be freed.
    
    // I'll add `node_free_recursive` helper to Part 3 and call it.
    ```

#### B. Hyperbole
*   **No Hyperbole Found**: There are no promotional, marketing, or exaggerated claims in any file or comment. All descriptions are strictly technical.

#### C. Legacy Artifacts & Mutants (Commented-out code and forbidden words)

*   **`widget.salt` (Line 13)**: Commented-out unused code.
    ```salt
    // type ClickCallback = fn(state: &mut AppState); 
    ```
*   **`test_ui.salt` (Line 541)**: Forbidden mutant term `Workaround`.
    ```salt
    // Workaround: recursive pointer types fail method lookup for offset? Cast to Ptr<i64> (size 8)
    ```
*   **`test_ui.salt` (Line 743)**: Forbidden mutant term `hack`.
    ```salt
    // I'll use a hack to free just the top level constructed nodes since widgets are reused?
    ```

#### D. Repository Constraint Violations

*   **Nesting Indentation Limit (Max 3 levels)**:
    *   `text.salt` (Lines 76-80): The `if (bits & (1 << row)) != 0` check is nested at 4 levels deep (function -> while -> while -> while -> if).
*   **Function Length Limit (Max 32 non-blank lines)**:
    *   `widget.salt`:
        *   `widget_layout` (lines 94-128: 35 total lines, ~33 non-blank lines)
        *   `widget_build` (lines 131-206: 76 total lines, ~42 non-blank lines)
        *   `widget_event` (lines 210-258: 49 total lines, ~32-35 non-blank lines)
    *   `test_ui.salt`:
        *   `fill_path` (~33 non-blank lines)
        *   `widget_build` (~35 non-blank lines)
        *   `widget_event` (~33 non-blank lines)
        *   `main` (~78 non-blank lines)
*   **File Length Limit (Max 500 lines)**:
    *   `test_ui.salt` has 760 lines, violating the 500-line limit for individual files.

---

## 2. Logic Chain

1.  **AI Slop Identification**: Professional codebase comments should state facts, definitions, or algorithms. Conversational style, self-answering, and "thought monologue" comments (e.g. `Wait, I only have... No, I'll allow...`) indicate copy-pasting from LLM output.
2.  **Constraint Verifications**:
    *   The nesting depth in `text_build_path` (line 76) spans 4 levels of indentation (16 spaces indent), which is a hard violation of the max-3 nesting rule.
    *   Checking line counts of functions in `widget.salt` reveals multiple core routines (`widget_layout`, `widget_build`, `widget_event`) exceeding the 32 non-blank lines constraint.
    *   `test_ui.salt` length (760 lines) directly violates the 500-line file length limit.
3.  **Legacy and Mutant Term Flags**: The presence of `hack` and `workaround` directly violates the "No mutants" rule specified in Kevin's Agent Rules.

---

## 3. Caveats

*   `test_ui.salt` is a test/demo application. The task description specifies: *"Exclude vendor dependencies, test fixtures, and generated files."* As such, `test_ui.salt` may be considered a test fixture and excluded from strict structural refactoring (like splitting the file to bypass the 500-line limit), but its AI slop comments and forbidden mutant keywords (`hack`/`workaround`) should still be scrubbed to ensure pre-commit hooks do not fail on it.

---

## 4. Conclusion & Cleanup Strategy

The `ui/` directory contains functional code but violates repository style and code quality rules. The following cleanup strategy is recommended for the implementer:

### Step 1: Scrub AI Slop & Legacy Comments
*   **`text.salt`**: Rewrite lines 6-14 to be a clean, declarative description of the 5x7 font layout.
*   **`widget.salt`**:
    *   Delete the commented-out `ClickCallback` type (line 13).
    *   Scrub all conversational thoughts in `widget_build` (lines 138-148, 159) and `widget_event` (lines 212-218). Replace them with clear documentation of the layout propagation/transforms.
*   **`test_ui.salt`**: Remove the conversational monologue lines 735-748. Replace the `hack` and `Workaround` keyword mentions with clean technical descriptions.

### Step 2: Refactor to Meet Indentation Constraints
*   **`text.salt`**: Extract the innermost pixel extraction loop in `text_build_path` into a helper function `fn text_render_glyph_pixel(p: Ptr<Path>, bits: u8, cursor_x: f32, y: f32, col: i64)`. This reduces the nesting depth in `text_build_path` to 2 levels.

### Step 3: Decompose Over-length Functions
*   **`widget.salt`**: Decompose the widget handlers by splitting layout, build, and event logic for specific widget types:
    *   Extract column layout into `fn widget_layout_column`.
    *   Extract button build logic into `fn widget_build_button`.
    *   Extract column event handling into `fn widget_event_column`.

---

## 5. Verification Method

1.  **Build Verification**:
    Ensure the `ui` module compiles cleanly before and after changes by running:
    ```bash
    saltc ui/test_ui.salt --lib -o /dev/null
    ```
    Alternatively, run the test suite target in the Makefile:
    ```bash
    make test
    ```
2.  **Constraint Verification**:
    Run a grep search to verify no conversational words or forbidden mutants (`hack`, `workaround`, `todo`, `fixme`, `xxx`) exist in the source comments.
