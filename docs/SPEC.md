# The Salt Programming Language — Specification

**Version 4.1 (July 2026)**  
**Status**: Draft — tracks the reference implementation (`salt-front`).

---

## Contents

1. [Lexical Structure](#1-lexical-structure)
2. [Types](#2-types)
3. [Expressions](#3-expressions)
4. [Statements](#4-statements)
5. [Functions](#5-functions)
6. [Modules and Name Resolution](#6-modules-and-name-resolution)
7. [Verification](#7-verification)
8. [Memory Model](#8-memory-model)
9. [Concurrency](#9-concurrency)
10. [Patterns](#10-patterns)
11. [The Preprocessor](#11-the-preprocessor)
12. [Standard Library](#12-standard-library)
13. [FFI and Unsafe](#13-ffi-and-unsafe)

[Appendix A: Complete EBNF Grammar](#appendix-a-complete-ebnf-grammar)  
[Appendix B: Operator Precedence](#appendix-b-operator-precedence-complete)  
[Appendix C: Compiler CLI](#appendix-c-compiler-cli)  
[Appendix D: Error Codes](#appendix-d-error-codes)

---

## Notation

Syntactic forms are specified in Extended Backus-Naur Form (EBNF, ISO/IEC 14977). A terminal symbol is written in `"double quotes"` or `'single quotes'`. A nonterminal is written in *italic*. The following meta-syntax is used:

> `[ X ]` — zero or one occurrence of X  
> `{ X }` — zero or more occurrences of X  
> `X | Y` — exactly one of X or Y  
> `( X )` — grouping

Where the grammar is insufficient to convey semantics, natural-language rules follow each production. The notation `⟦ expr ⟧` denotes the value obtained by evaluating `expr`.

This specification describes the language independently of any particular implementation. Where the reference implementation (`salt-front`) diverges, that divergence is noted.

---

## 1. Lexical Structure

### 1.1 Source Character Set

Salt source files are UTF-8 encoded. The syntax is defined over ASCII characters. Non-ASCII characters may appear only within string literals and comments.

```ebnf
SOURCE_CHAR = ? any UTF-8 code point ? ;
```

### 1.2 Comments

Line comments begin with `//` and extend to the end of the line. There are no block comments.

```ebnf
COMMENT = "//", { ? any character except newline ? }, ? newline ? ;
```

### 1.3 Whitespace

Spaces (`U+0020`), horizontal tabs (`U+0009`), carriage returns (`U+000D`), and newlines (`U+000A`) separate tokens. Whitespace carries no semantic meaning except as token delimiters.

### 1.4 Identifiers

An identifier names a variable, function, type, module, or trait.

```ebnf
IDENTIFIER = ( LETTER | "_" ), { LETTER | DIGIT | "_" } ;
LETTER     = "a".."z" | "A".."Z" ;
DIGIT      = "0".."9" ;
```

Identifiers containing `__` (two consecutive underscores) are reserved for compiler-generated names and must not be used in source code.

### 1.5 Keywords

The following are reserved and may not be used as identifiers:

```
fn   struct enum  trait  impl   let    mut    const
return  if   else  while  for    in     loop   match
break   continue  pub  unsafe self  Self  true   false
package  import requires ensures concept invariant
move  with  region  extern  as  ref  global  var
owned window  map_window
```

Additionally, `reinterpret_cast` and `shader` are reserved in specific syntactic contexts.

### 1.6 Literals

**Integer literals:**

```ebnf
INTEGER_LIT = DECIMAL_LIT | HEX_LIT | BINARY_LIT ;
DECIMAL_LIT = DIGIT, { DIGIT | "_" } ;
HEX_LIT     = "0x", HEX_DIGIT, { HEX_DIGIT | "_" } ;
BINARY_LIT  = "0b", BINARY_DIGIT, { BINARY_DIGIT | "_" } ;
HEX_DIGIT   = DIGIT | "a".."f" | "A".."F" ;
BINARY_DIGIT = "0" | "1" ;
```

An integer literal has type `i32` by default. An explicit suffix overrides: `42i64`, `255u8`, `1usize`.

**Floating-point literals:**

```ebnf
FLOAT_LIT = DECIMAL_LIT, ".", DECIMAL_LIT, [ EXPONENT ], [ FLOAT_SUFFIX ] ;
EXPONENT  = ("e" | "E"), [ "+" | "-" ], DECIMAL_LIT ;
FLOAT_SUFFIX = "f32" | "f64" ;
```

A float literal without suffix has type `f64`.

**Boolean literals:** `true` and `false`. Type: `bool`.

**Character literals:** A single character between single quotes, e.g., `'A'`. Escape sequences: `'\n'` (newline), `'\t'` (tab), `'\\'` (backslash), `'\0'` (null). A character literal has type `i8` and its value is the Unicode scalar value of the character.

**String literals:** A sequence of characters between double quotes, e.g., `"hello"`. The same escape sequences as character literals apply. A string literal has type `StringView`.

**F-string literals:** Prefixed with `f`, e.g., `f"x = {x}"`. The preprocessor expands these to `__fstring__!(...)` invocations.

**Hex literal prefix:** `hex"DEADBEEF"` expands to `__hex__!("DEADBEEF")`. Whitespace within the quotes is ignored.

### 1.7 Tokenization

The tokenizer uses maximal munch: at each position, the longest sequence of characters that matches a token production is consumed. Whitespace and comments are discarded during tokenization.

---

## 2. Types

### 2.1 Type Grammar

```ebnf
TYPE = PRIMITIVE
     | PtrType | RefType | ArrayType | TupleType
     | FnPtrType | TensorType | NamedType ;

PRIMITIVE = "i8" | "i16" | "i32" | "i64"
              | "u8" | "u16" | "u32" | "u64" | "usize"
              | "f32" | "f64" | "bool" | "char" | "()" ;

PtrType   = "Ptr", "<", TYPE, ">" ;
RefType   = "&", [ "mut" ], TYPE ;
ArrayType = "[", TYPE, ";", EXPRESSION, "]" ;
TupleType = "(", [ TYPE, { ",", TYPE } ], ")" ;
FnPtrType = "fn", "(", [ TYPE, { ",", TYPE } ], ")", [ "->", TYPE ] ;
TensorType = "Tensor", "<", TYPE, ",", "{", TENSOR_DIMS, "}", ">" ;
NamedType = IDENTIFIER, [ "<", TYPE, { ",", TYPE }, ">" ]
          | PATH, ".", IDENTIFIER, [ "<", TYPE, { ",", TYPE }, ">" ] ;
```

### 2.2 Primitive Types

| Type | Width | Signed | MLIR equivalent |
|------|-------|--------|-----------------|
| `i8` | 1 byte | Yes | `i8` |
| `i16` | 2 bytes | Yes | `i16` |
| `i32` | 4 bytes | Yes | `i32` |
| `i64` | 8 bytes | Yes | `i64` |
| `u8` | 1 byte | No | `i8` |
| `u16` | 2 bytes | No | `i16` |
| `u32` | 4 bytes | No | `i32` |
| `u64` | 8 bytes | No | `i64` |
| `usize` | platform-dependent | No | `index` |
| `f32` | 4 bytes | — | `f32` |
| `f64` | 8 bytes | — | `f64` |
| `bool` | 1 byte | — | `i1` or `i8` |
| `char` | 1 byte | — | `i8` |
| `()` | 0 bytes | — | unit |

All integer types use two's complement representation. Floating-point types follow IEEE 754. `usize` is the pointer-sized unsigned integer for the target platform.

### 2.3 Compound Types

**Pointer `Ptr<T>`**: A typed, provenance-tracked pointer to a value of type `T`. Null pointers are represented as `Ptr::empty()`. Dereferencing a `Ptr<T>` yields an lvalue of type `T`.

**Reference `&T` and `&mut T`**: A stack-rooted borrow of a value. `&T` allows read access; `&mut T` allows read-write access. References carry no ownership. The compiler verifies that references do not outlive their referent.

**Array `[T; N]`**: A fixed-size sequence of N values of type T, stored contiguously in memory. N is a compile-time constant expression. Indexing is zero-based. The `.length()` method returns N.

**Tuple `(T₁, T₂, ..., Tₙ)`**: A heterogeneous product type. The empty tuple `()` is the unit type. Tuple fields are accessed by destructuring: `let (a, b) = tuple;`.

**Function pointer `fn(T₁, ..., Tₙ) -> R`**: A first-class value representing the address of a function with the given signature. Created via `fn_addr(f)`. Called via `f(arg₁, ..., argₙ)`.

**Tensor `Tensor<T, {D₁, D₂, ..., Dₙ}>`**: A shaped multi-dimensional array. Compiles to a pointer. The `@` operator dispatches to `linalg.matmul` for two-dimensional tensors.

### 2.4 User-Defined Types

**Structs:**

```ebnf
STRUCT_DECL = [ ATTRIBUTE ], "struct", IDENTIFIER, [ "<", GENERIC_PARAMS, ">" ],
              "{", { STRUCT_FIELD }, "}" ;
STRUCT_FIELD = [ ATTRIBUTE ], IDENTIFIER, ":", TYPE, "," ;
```

A struct defines a named product type. Fields are accessed with dot notation: `s.field`.

**Enums:**

```ebnf
ENUM_DECL = "enum", IDENTIFIER, [ "<", GENERIC_PARAMS, ">" ],
            "{", { ENUM_VARIANT, "," }, "}" ;
ENUM_VARIANT = IDENTIFIER, [ "(", TYPE, { ",", TYPE }, ")" ] ;
```

An enum defines a tagged union. Each variant carries zero or more associated values. A variant with no values is a unit variant. A variant with one value carries that value directly. A variant with multiple values is stored as a tuple. Enum values are constructed as `VariantName(val₁, ..., valₙ)` and destroyed by `match`.

### 2.5 Generics

```ebnf
GENERIC_PARAMS = IDENTIFIER, { ",", IDENTIFIER } ;
```

Generic type and function parameters are monomorphized at compile time. Each unique instantiation produces a separate copy of the code. There is no runtime type erasure.

### 2.6 Memory Layout

`sizeof(T)` and `alignof(T)` are compiler-determined for each type T. The `@align(N)` attribute overrides the default alignment for a struct field. If the requested alignment cannot be satisfied (e.g., `@align(64)` on a field at offset that is not a multiple of 64), the compiler reports an error.

---

## 3. Expressions

### 3.1 Expression Classification

An expression is either an *rvalue* (produces a value) or an *lvalue* (designates a memory location). The following forms are lvalues: local variables, function parameters, field accesses `e.field`, array indexing `e[i]`, dereferences `*p`, and parenthesized lvalues. All other forms are rvalues.

### 3.2 Operator Precedence

From highest to lowest binding:

| Precedence | Operators | Associativity |
|------------|-----------|---------------|
| 17 | `.` `::` | left |
| 16 | `()` `[]` (call, index) | left |
| 15 | `-` `!` `*` `&` (unary) | right |
| 14 | `as` | left |
| 13 | `*` `/` `%` | left |
| 12 | `+` `-` | left |
| 11 | `<<` `>>` | left |
| 10 | `&` (bitwise) | left |
| 9 | `^` | left |
| 8 | `\|` | left |
| 7 | `<` `<=` `>` `>=` | left |
| 6 | `==` `!=` | left |
| 5 | `&&` | left |
| 4 | `\|\|` | left |
| 3 | `=` `+=` `-=` `*=` `/=` `%=` `<<=` `>>=` | right |
| 2 | `\|>` `\|?>` | left |
| 1 | `@` (matmul) | left |

Parentheses override precedence: `(expr)` evaluates `expr` before any enclosing operator.

### 3.3 Binary Operators

**Arithmetic:** `+`, `-`, `*`, `/`, `%` work on integer and floating-point types. Both operands must have the same type after numeric promotion. Division by zero and signed overflow are runtime errors unless the compiler proves them impossible at compile time.

**Bitwise:** `&`, `|`, `^`, `<<`, `>>` work on integer types. Shift amount must be non-negative.

**Relational:** `<`, `<=`, `>`, `>=` compare integers and floats, returning `bool`.

**Equality:** `==`, `!=` compare any two values of the same type. For structs and enums, equality is field-wise; for arrays, element-wise; for tuples, component-wise. Reference equality is NOT provided — `Ptr<T>` equality compares the addresses.

**Logical:** `&&` and `||` short-circuit. The right operand is evaluated only if the left operand does not determine the result.

### 3.4 Unary Operators

`-expr` negates an integer or float. `!expr` negates a boolean or integer (bitwise NOT for integers). `*expr` dereferences a pointer or reference, producing an lvalue. `&expr` takes the address of an lvalue, producing a reference.

### 3.5 Type Casts

`expr as T` converts `expr` to type `T`. Numeric casts between integer widths truncate or sign-extend. Float-to-integer casts truncate toward zero. The `as` operator cannot cast between unrelated pointer types; use `reinterpret_cast` in `unsafe` blocks for low-level type punning.

### 3.6 Path Expressions

A path expression refers to a named entity: a local variable, a function, a type constructor, or an imported name. Paths are dot-separated: `std.collections.HashMap`. The leading component is resolved in the current scope; subsequent components are resolved within the preceding module or type.

### 3.7 Call Expressions

`f(a₁, ..., aₙ)` calls function `f` with arguments `a₁` through `aₙ`. Arguments are evaluated left-to-right. Each argument is moved into the callee; the caller loses ownership. If the callee's parameter type is a reference, the argument is implicitly borrowed rather than moved.

### 3.8 Index Expressions

`e[i]` indexes into array `e` at position `i`. The index must be an integer type. Bounds are checked at runtime unless the compiler proves `0 <= i < e.length()` at compile time.

### 3.9 Field Access

`e.field` accesses the named field of a struct value `e`. The type of the expression is the declared type of the field.

### 3.10 Struct Literals

`TypeName { field₁: val₁, field₂: val₂ }` constructs a value of the named struct type. All fields must be provided. Field order is irrelevant. Shorthand `TypeName { x, y }` is permitted when the field name matches a variable in scope.

---

## 4. Statements

### 4.1 `let` Binding

```ebnf
LET_STMT = "let", [ "mut" ], PATTERN, [ ":", TYPE ], "=", EXPRESSION, ";" ;
```

A `let` statement introduces a new local binding. The initializer expression is evaluated and bound to the pattern. If `mut` is present, the binding is mutable. If the type annotation is omitted, the type is inferred from the initializer.

The binding is in scope from its declaration to the end of the enclosing block.

### 4.2 `let`-`else`

```ebnf
LET_ELSE_STMT = "let", PATTERN, "=", EXPRESSION, "else", BLOCK ;
```

The pattern must be irrefutable. If the expression evaluates to a value matching the pattern, the bound variables are in scope for the remainder of the enclosing block. If the expression does NOT match (e.g., `None` when `Some(x)` is expected), the `else` block is executed, which must diverge (return, break, or call a diverging function).

### 4.3 Assignment

```ebnf
ASSIGN_STMT = EXPRESSION, ASSIGN_OP, EXPRESSION, ";" ;
ASSIGN_OP   = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" ;
```

The left-hand side must be a mutable lvalue. The right-hand side is evaluated and the result is written to the location. Compound assignment operators compute the result as if by `lhs = lhs OP rhs`, except that `lhs` is evaluated once.

### 4.4 `if` Statement

```ebnf
IF_STMT = "if", EXPRESSION, BLOCK, [ "else", ( BLOCK | IF_STMT ) ] ;
```

The condition expression must have type `bool`. If true, the first block is executed; otherwise, the `else` block (if present) is executed. `if` is an expression when every branch produces a value of the same type.

### 4.5 `while` Statement

```ebnf
WHILE_STMT = "while", EXPRESSION, BLOCK ;
```

The condition is evaluated before each iteration. If true, the block is executed and the loop repeats. If false, execution continues after the block.

### 4.6 `for` Statement

```ebnf
FOR_STMT = "for", IDENTIFIER, "in", EXPRESSION, BLOCK ;
```

The expression must produce an iterator (a value with a `.next()` method). The loop variable takes each successive element. The loop variable is immutable.

### 4.7 `loop` Statement

```ebnf
LOOP_STMT = "loop", BLOCK ;
```

Infinite loop. Exit via `break` or `return`.

### 4.8 `match` Statement

```ebnf
MATCH_STMT = "match", EXPRESSION, "{", { MATCH_ARM, "," }, "}" ;
MATCH_ARM  = PATTERN, [ "if", EXPRESSION ], "=>", ( BLOCK | EXPRESSION, "," ) ;
```

The scrutinee is evaluated. Arms are tested in order. The first arm whose pattern matches (and whose optional guard evaluates to `true`) is executed. The compiler verifies exhaustiveness: every possible value of the scrutinee type must be covered by at least one arm.

### 4.9 `return` Statement

```ebnf
RETURN_STMT = "return", [ EXPRESSION ], ";" ;
```

Evaluates the expression (if present) and transfers control to the function's caller. The expression type must match the function's declared return type. In a function returning `()`, `return;` with no expression is permitted.

### 4.10 `unsafe` Block

```ebnf
UNSAFE_BLOCK = "unsafe", BLOCK ;
```

Within an `unsafe` block, raw pointer arithmetic, `reinterpret_cast`, and direct memory operations are permitted. The compiler does not verify memory safety within `unsafe` blocks.

### 4.11 `region` Block

```ebnf
REGION_STMT = "with", "region", IDENTIFIER, BLOCK ;
REGION_CALL = "region", "(", STRING_LIT, ")", BLOCK ;
```

Declares a memory region. All arena allocations within the block are associated with the named region. When the block exits, the region is freed.

---

## 5. Functions

### 5.1 Function Declarations

```ebnf
FN_DECL = { ATTRIBUTE }, [ "pub" ], "fn", IDENTIFIER,
          [ "<", GENERIC_PARAMS, ">" ],
          "(", [ PARAMS ], ")", [ "->", TYPE ],
          { CONTRACT_CLAUSE },
          BLOCK ;
PARAMS = PARAM, { ",", PARAM } ;
PARAM  = PATTERN, ":", TYPE ;
```

A function declares a named, callable computation. `pub` makes it visible outside the current module. Generic parameters enable parametric polymorphism. The return type defaults to `()` if omitted.

### 5.2 Attributes

Attributes are prefixed with `@` and appear before the item they modify.

| Attribute | Valid on | Effect |
|-----------|----------|--------|
| `@inline` | Functions | Hint to inline at call sites |
| `@trusted` | Functions | Bypass contract verification for the function body. Used for FFI wrappers and hand-audited code. |
| `@export` | Functions | Emit with C-compatible symbol name (no name mangling) |
| `@yielding(N)` | Functions | Inject a yield point every N loop iterations, enabling cooperative scheduling. If N is omitted, a default interval is used. |
| `@pulse(N)` | Functions | Register the function to be invoked at N Hz by the scheduler's pulse timer. The function must take no arguments and return nothing. |
| `@align(N)` | Struct fields | Override the field's alignment to N bytes. The compiler verifies the requested alignment is satisfiable. |
| `@derive(T₁, ..., Tₙ)` | Structs, enums | Auto-generate trait implementations from the type's fields or variants.

### 5.3 Move Semantics

When a function is called, each argument is *moved* into the corresponding parameter. The caller's binding becomes unavailable. Subsequent use of a moved variable is a compile-time error.

If the parameter type is a reference (`&T` or `&mut T`), the argument is *borrowed* rather than moved. The caller retains ownership for the duration of the call. References must not outlive their referent; the compiler verifies this intraprocedurally via the scope ladder.

### 5.4 Function Pointers

A function pointer type `fn(T₁, ..., Tₙ) -> R` designates the address of any function with that signature. The built-in `fn_addr(f)` returns a function pointer to `f`. Calling a function pointer has the same syntax and semantics as calling a named function.

### 5.5 Extern Functions

```ebnf
EXTERN_FN_DECL = "extern", "fn", IDENTIFIER, "(", [ PARAMS ], ")", [ "->", TYPE ],
            { CONTRACT_CLAUSE }, ";" ;
```

Declares a function with C ABI linkage. Only primitive types, `Ptr<T>`, and function pointers may appear in the signature of an `extern fn`. The body is provided by an external object file.

---

## 6. Modules and Name Resolution

### 6.1 Package Declaration

```ebnf
PACKAGE_DECL = "package", IDENTIFIER, { ".", IDENTIFIER }, ";" ;
```

A package declaration names the module. It must appear as the first non-comment item in a source file.

### 6.2 Imports

```ebnf
IMPORT_DECL = "import", PATH, [ ".", ( "*" | "{", IDENTIFIER, { ",", IDENTIFIER }, "}" ) ], ";" ;
PATH        = IDENTIFIER, { ".", IDENTIFIER } ;
```

An import makes names from another module available in the current scope. `import std.core.ptr.*` imports all public names from `std.core.ptr`. `import std.io.file.{File, BufferedReader}` imports specific names.

### 6.3 Visibility

A top-level item prefixed with `pub` is visible to other modules. Without `pub`, the item is private to the declaring module. Struct fields follow the struct's visibility; there is no per-field visibility.

---

## 7. Verification

### 7.1 Contracts

```ebnf
CONTRACT_CLAUSE = ( "requires" | "ensures" ), EXPRESSION, ";" ;
```

A `requires` clause declares a precondition: a boolean expression that must hold at every call site. A `ensures` clause declares a postcondition: a boolean expression that must hold at every return site. The special identifier `result` in an `ensures` clause refers to the function's return value.

### 7.2 Verification Model

Contracts are checked at compile time using an SMT solver. The process for each contract clause is:

1. The compiler substitutes the actual arguments into the expression.
2. The constant folder attempts to reduce the expression. If it evaluates to `true`, the check is elided — zero runtime instructions are emitted.
3. If constant folding fails, the compiler checks whether the negation of the expression is satisfiable.
4. **Proved:** No input can violate the condition. The check is elided.
5. **Counterexample:** A violating input exists. The compiler reports the specific values and stops with an error.
6. **Timeout:** The solver cannot decide within a fixed time budget (100ms). The compiler emits a runtime assertion as a fallback. The program compiles and runs, but will trap if the condition is violated at runtime.

Postconditions are checked similarly at each return site, with `result` bound to the returned expression.

### 7.3 Type-Bound Proofs

Before checking a contract, the compiler injects type range constraints into the solver. For a parameter `x: u8`, the solver is informed that `0 <= x <= 255`. This means `requires(x < 256)` on a `u8` parameter is trivially proved for any argument value, even when the argument is not a compile-time constant.

### 7.4 Loop Invariants

While loops may contain `invariant` statements that the compiler checks using Hoare logic:

```salt
let mut i: i64 = 0;
while i < 5 {
    invariant i >= 0 && i < 5;
    arr[i] = 0;  // Z3 proves this is safe using the invariant
    i = i + 1;
}
```

The compiler verifies two properties:
1. **Base case**: the invariant holds at loop entry
2. **Inductive step**: assuming the invariant and loop condition hold, the invariant still holds after one iteration (modelled by havocking modified variables)

If either check fails, the compiler reports a counterexample. Invariants that constrain an index variable to a known range enable Z3 to prove array bounds safety inside while loops — the same way `for`-loop induction variables do automatically.

### 7.5 Limitations

Contracts cannot prove all properties. Known limitations of the current implementation:

- Floating-point properties: the solver's theory of floating-point arithmetic is incomplete. Contracts with non-trivial float expressions may timeout.
- String length and content: only compile-time-known string literals are reliably folded to constants. Properties of strings from runtime sources (I/O, network) rely on the timeout fallback.
- Non-linear integer arithmetic: multiplication of two variables may timeout.
- The `@trusted` attribute bypasses verification entirely for the annotated function body.

---

## 8. Memory Model

### 8.1 Allocation

Salt provides three allocation strategies:

**Arena allocation:** `Arena::new(capacity)` creates a bump-allocated region. `arena.alloc::<T>()` allocates a value of type T within the region. `arena.alloc_bytes(n)` allocates n bytes. `arena.mark()` captures the current offset. `arena.reset_to(mark)` frees all allocations since the mark. Arena allocation is O(1) and deterministic; the region is bulk-freed.

**Heap allocation:** `HeapAllocator` wraps the platform `malloc`/`free`. Used via `Box::new(value)` or `Vec::with_capacity(n)` with a heap allocator.

**Prelude default:** The default allocator is an arena. `Vec::new()` and `String::new()` use the prelude's `DefaultAllocator` unless an explicit allocator is provided.

### 8.2 Moves

Assignment, function argument passing, and return transfer ownership. After `let y = x;`, `x` is uninitialized and cannot be used. To explicitly transfer ownership, write `move x;`.

### 8.3 Scope Ladder

The compiler tracks the *depth* of every pointer expression:

- Depth 0: globals and statics. Live for the program's duration.
- Depth 1: function arguments. Live for the call's duration.
- Depth 2+: local variables. Live for the enclosing block's duration. Nested blocks increase depth.

Three rules govern pointer safety:

1. **Return Rule:** A pointer of depth d >= 2 must not be returned. The return value must have depth <= 1.
2. **Assignment Rule:** A pointer `b` can be stored into location `a` only if depth(b) <= depth(a). Shorter-lived pointers cannot be stored into longer-lived containers.
3. **Transitivity:** Field access and indexing preserve the depth of the parent object.

Violations are reported at compile time with reference to the specific rule violated.

---

## 9. Concurrency

Salt supports cooperative concurrency through two function attributes.

`@yielding(N)` annotates a function as cooperatively yielding. The compiler injects a yield point every N loop iterations, allowing the scheduler to preempt the function between iterations. If N is omitted, a default interval is used. The function must not hold locks across yield points.

`@pulse(N)` registers a function to be invoked by the scheduler's pulse timer at N Hz. The function must take no arguments and return nothing. Pulse functions are used for periodic tasks like cursor blinking, keepalive packets, and I/O polling.

These attributes target KeuOS's scheduler. In native (non-KeuOS) builds, `@yielding` has no effect and `@pulse` functions are never called.

---

## 10. Patterns

```ebnf
PATTERN = "_"                                  (* wildcard *)
        | LITERAL                              (* literal match *)
        | [ "mut" ], IDENTIFIER                (* binding *)
        | PATH, "(", [ PATTERN, { ",", PATTERN } ], ")"  (* enum variant *)
        | "(", [ PATTERN, { ",", PATTERN } ], ")"        (* tuple *)
        | PATH, "{", [ FIELD_PAT, { ",", FIELD_PAT } ], "}"  (* struct *)
        | PATTERN, "|", PATTERN                (* or-pattern *)
        ;
FIELD_PAT = IDENTIFIER, [ ":", PATTERN ] ;
```

A pattern is *irrefutable* if it matches every value of the scrutinee type. A pattern is *refutable* otherwise. `let` and function parameters require irrefutable patterns. `match` arms may be refutable.

---

## 11. The Preprocessor

Before parsing, the Salt source text undergoes the following transformations:

1. `use a.b.c` is rewritten to `import a.b.c`.
2. `a::b::c` path syntax is rewritten to `a.b.c`.
3. `f<A, B>(x)` is rewritten to `f::<A, B>(x)` (turbofish insertion).
4. `a |> f(b, _)` is rewritten to `f(b, a)` (pipe operator).
5. `a |?> f(b, _)` is rewritten to `__railway__!(a, f(b, _))`.
6. `a @ b` is rewritten to `a.matmul(b)`.
7. `f"text {expr}"` is rewritten to `__fstring__!("text {expr}")`.
8. `target.f"text {expr}"` is rewritten to `__target_fstring__!(target, "text {expr}")`.
9. `hex"AB CD"` is rewritten to `__hex__!("ABCD")`.
10. `expr~` is rewritten to `__force_unwrap__!(expr)`.
11. `@derive(T₁, T₂)` on a type declaration is expanded to the corresponding `impl` blocks.

These transformations are purely syntactic and do not affect the semantics of the resulting program.

---

## 12. Standard Library

The standard library is organized under `std.`. The module tree is:

| Path | Contents |
|------|----------|
| `std.core.ptr` | `Ptr<T>`, `Ptr::empty()`, pointer utilities |
| `std.core.option` | `Option<T>` (Some/None) |
| `std.core.result` | `Result<T>` (Ok/Err with `Status`) |
| `std.core.str` | `StringView`, string operations |
| `std.core.iter` | `Range`, iterator combinators |
| `std.core.clone` | `Clone` trait |
| `std.eq` | `Eq` trait |
| `std.hash` | `Hash` trait |
| `std.ord` | `Ord` trait |
| `std.string` | `String` (heap-owning) |
| `std.arena` | `Arena`, arena allocation |
| `std.arena.default` | `DefaultAllocator` |
| `std.io` | `print`, `println`, I/O primitives |
| `std.io.file` | `File`, `BufferedReader`, `BufferedWriter` |
| `std.io.ring` | `IoUring` submission/completion queues |
| `std.collections` | `Vec<T, A>`, `HashMap`, `StringMap` |
| `std.sync` | `Mutex`, `AtomicI64`, `AtomicU64` |
| `std.channel` | `Channel<T>`, `UnboundedChannel<T>` |
| `std.thread` | `Thread::spawn`, `Thread::join` |
| `std.process` | `Command` execution |
| `std.http` | HTTP client (`connect`, `send`, `recv`, `close`) |
| `std.json` | JSON parser/writer |
| `std.json.json` | `JsonParser`, `JsonWriter`, `JsonArray`, `JsonObject` |
| `std.net` | Network primitives |
| `std.time` | `sleep_ms`, `sleep_nanos`, clock utilities |
| `std.simd` | Vector intrinsics (`v_load`, `v_store`, `v_fma`, etc.) |

The prelude implicitly imports `Ptr`, `Option`, `Result`, `Status`, `DefaultAllocator`, and `print`. These names are available without explicit imports in any source file.

---

## 13. FFI and Unsafe

### 13.1 Extern Functions

`extern fn` declares a function with C ABI linkage. The compiler does not generate a body. At link time, the symbol must be provided by an external object file. Only the following types may cross the FFI boundary: `i8` through `i64`, `u8` through `u64`, `f32`, `f64`, `bool`, `Ptr<T>`, `fn(T₁,...,Tₙ) -> R`. Attempting to use any other type in an extern function signature is a compile-time error.

### 13.2 `@export`

The `@export` attribute suppresses name mangling. An exported function can be called from C code by its declared name.

### 13.3 Unsafe Blocks

Within `unsafe { ... }`, the following operations are permitted:
- Raw pointer arithmetic on `Ptr<T>`
- `reinterpret_cast<T>(expr)` — reinterpret the bytes of `expr` as type `T`
- Dereferencing pointers without bounds or validity checks
- Calling functions not annotated with safety contracts

The compiler does not verify memory safety within `unsafe` blocks. Safety is the programmer's responsibility.

---

## Appendix A: Complete EBNF Grammar

```ebnf
(* Source file *)
COMPILATION_UNIT = [ PACKAGE_DECL ], { IMPORT_DECL }, { ITEM } ;

(* Declarations *)
PACKAGE_DECL    = "package", IDENTIFIER, { ".", IDENTIFIER }, ";" ;
IMPORT_DECL     = "import", PATH, [ ".", ( "*" | "{", IDENTIFIER, { ",", IDENTIFIER }, "}" ) ], ";" ;
PATH            = IDENTIFIER, { ".", IDENTIFIER } ;
ITEM            = FN_DECL | STRUCT_DECL | ENUM_DECL | CONST_DECL
                | GLOBAL_DECL | TRAIT_DECL | IMPL_BLOCK | CONCEPT_DECL
                | EXTERN_FN_DECL ;

(* Functions *)
FN_DECL         = { ATTRIBUTE }, [ "pub" ], "fn", IDENTIFIER,
                  [ "<", GENERIC_PARAMS, ">" ],
                  "(", [ PARAMS ], ")", [ "->", TYPE ],
                  { CONTRACT_CLAUSE }, BLOCK ;
PARAMS          = PARAM, { ",", PARAM } ;
PARAM           = PATTERN, ":", TYPE ;
EXTERN_FN_DECL  = "extern", "fn", IDENTIFIER, "(", [ PARAMS ], ")", [ "->", TYPE ],
                  { CONTRACT_CLAUSE }, ";" ;

(* Generics *)
GENERIC_PARAMS  = IDENTIFIER, { ",", IDENTIFIER } ;

(* Types *)
TYPE            = PRIMITIVE | PtrType | RefType | ArrayType
                | TupleType | FnPtrType | NamedType ;
PRIMITIVE       = "i8"|"i16"|"i32"|"i64"|"u8"|"u16"|"u32"|"u64"
                | "usize"|"f32"|"f64"|"bool"|"char"|"()" ;
PtrType         = "Ptr", "<", TYPE, ">" ;
RefType         = "&", [ "mut" ], TYPE ;
ArrayType       = "[", TYPE, ";", EXPRESSION, "]" ;
TupleType       = "(", [ TYPE, { ",", TYPE } ], ")" ;
FnPtrType       = "fn", "(", [ TYPE, { ",", TYPE } ], ")", [ "->", TYPE ] ;
NamedType       = PATH, [ "<", TYPE, { ",", TYPE }, ">" ] ;

(* Declarations *)
STRUCT_DECL     = { ATTRIBUTE }, "struct", IDENTIFIER, [ "<", GENERIC_PARAMS, ">" ],
                  "{", { STRUCT_FIELD }, "}" ;
STRUCT_FIELD    = { ATTRIBUTE }, IDENTIFIER, ":", TYPE, "," ;
ENUM_DECL       = "enum", IDENTIFIER, [ "<", GENERIC_PARAMS, ">" ],
                  "{", { ENUM_VARIANT, "," }, "}" ;
ENUM_VARIANT    = IDENTIFIER, [ "(", TYPE, { ",", TYPE }, ")" ] ;
CONST_DECL      = "const", IDENTIFIER, ":", TYPE, "=", EXPRESSION, ";" ;
GLOBAL_DECL     = "global", IDENTIFIER, ":", TYPE, "=", EXPRESSION, ";" ;
TRAIT_DECL      = "trait", IDENTIFIER, [ "<", GENERIC_PARAMS, ">" ],
                  "{", { TRAIT_METHOD }, "}" ;
TRAIT_METHOD    = { ATTRIBUTE }, "fn", IDENTIFIER, "(", [ PARAMS ], ")", [ "->", TYPE ], ";" ;
IMPL_BLOCK      = "impl", [ GENERIC_PARAMS ], [ PATH, "for" ], TYPE,
                  "{", { IMPL_ITEM }, "}" ;
IMPL_ITEM       = FN_DECL | TRAIT_METHOD ;
CONCEPT_DECL    = "concept", IDENTIFIER, "(", TYPE, ")", "requires", "(", EXPRESSION, ")", ";" ;

(* Statements *)
BLOCK           = "{", { STMT }, "}" ;
STMT            = LET_STMT | LET_ELSE_STMT | ASSIGN_STMT
                | IF_STMT | WHILE_STMT | FOR_STMT | LOOP_STMT
                | MATCH_STMT | RETURN_STMT | UNSAFE_BLOCK
                | REGION_STMT | EXPR_STMT ;

(* Expressions *)
EXPR_STMT       = EXPRESSION, ";" ;
LET_STMT        = "let", [ "mut" ], PATTERN, [ ":", TYPE ], "=", EXPRESSION, ";" ;
LET_ELSE_STMT   = "let", PATTERN, "=", EXPRESSION, "else", BLOCK ;
ASSIGN_STMT     = EXPRESSION, ASSIGN_OP, EXPRESSION, ";" ;
IF_STMT         = "if", EXPRESSION, BLOCK, [ "else", ( BLOCK | IF_STMT ) ] ;
WHILE_STMT      = "while", EXPRESSION, BLOCK ;
FOR_STMT        = "for", IDENTIFIER, "in", EXPRESSION, BLOCK ;
LOOP_STMT       = "loop", BLOCK ;
MATCH_STMT      = "match", EXPRESSION, "{", { MATCH_ARM, "," }, "}" ;
MATCH_ARM       = PATTERN, [ "if", EXPRESSION ], "=>", ( BLOCK | EXPRESSION, "," ) ;
RETURN_STMT     = "return", [ EXPRESSION ], ";" ;
UNSAFE_BLOCK    = "unsafe", BLOCK ;
REGION_STMT     = ( "with", "region", IDENTIFIER, BLOCK )
                | ( "region", "(", STRING_LIT, ")", BLOCK ) ;

(* Patterns *)
PATTERN         = "_" | PATTERN_LIT | PATTERN_BIND | PATTERN_VARIANT
                | PATTERN_TUPLE | PATTERN_STRUCT | PATTERN_OR ;
PATTERN_LIT     = LITERAL ;
PATTERN_BIND    = [ "mut" ], IDENTIFIER ;
PATTERN_VARIANT = PATH, "(", [ PATTERN, { ",", PATTERN } ], ")" ;
PATTERN_TUPLE   = "(", [ PATTERN, { ",", PATTERN } ], ")" ;
PATTERN_STRUCT  = PATH, "{", [ FIELD_PAT, { ",", FIELD_PAT } ], "}" ;
PATTERN_OR      = PATTERN, "|", PATTERN ;

(* Contracts *)
CONTRACT_CLAUSE = ( "requires" | "ensures" ), EXPRESSION, ";" ;

(* Attributes *)
ATTRIBUTE       = "@", IDENTIFIER, [ "(", ATTRIBUTE_ARGS, ")" ] ;
ATTRIBUTE_ARGS  = EXPRESSION, { ",", EXPRESSION } ;

(* Identifiers and literals *)
IDENTIFIER      = ( LETTER | "_" ), { LETTER | DIGIT | "_" } ;
LITERAL         = INTEGER_LIT | FLOAT_LIT | "true" | "false" | CHAR_LIT | STRING_LIT ;
```

---

## Appendix B: Operator Precedence (Complete)

| Prec | Operators | Assoc | Category |
|------|-----------|-------|----------|
| 17 | `.` | left | Field/method access |
| 16 | `()` `[]` | left | Call, index |
| 15 | `-` `!` `*` `&` | right | Unary |
| 14 | `as` | left | Type cast |
| 13 | `*` `/` `%` | left | Multiplicative |
| 12 | `+` `-` | left | Additive |
| 11 | `<<` `>>` | left | Shift |
| 10 | `&` | left | Bitwise AND |
| 9 | `^` | left | Bitwise XOR |
| 8 | `\|` | left | Bitwise OR |
| 7 | `<` `<=` `>` `>=` | left | Relational |
| 6 | `==` `!=` | left | Equality |
| 5 | `&&` | left | Logical AND |
| 4 | `\|\|` | left | Logical OR |
| 3 | `=` `+=` `-=` `*=` `/=` `%=` `<<=` `>>=` | right | Assignment |
| 2 | `\|>` `\|?>` | left | Pipe |
| 1 | `@` | left | Matmul |

---

## Appendix C: Compiler CLI

```
saltc <file.salt> [-o <path>] [flags]

  --release              Optimizations enabled (default: debug)
  --binary               Produce native executable (Mach-O, ELF, or PE)
  -c                     Produce .o object file
  --target <target>      Target platform: macos, linux-arm64, windows, keuos, keuos-x86_64
  --lib                  Library mode (no main entry point required)
  --sip                  Mode B SIP safety enforcement
  --verify               Enable contract verification (default: on)
  --danger-no-verify     Skip all verification (debug builds only)
  --skip-scan            Skip import dependency scanning
  --emit-sir             Emit SIR as JSON for tooling
  -g, --debug-info       Emit DWARF debug information
  --disable-alias-scopes Suppress LLVM alias scope metadata
  -o <path>              Output path (MLIR by default, or binary with --binary)
```

---

## Appendix D: Error Codes

| Code | Description |
|------|-------------|
| E001 | Failed to read source file |
| E002 | Parse error |
| E003 | Compilation failed (verification error, type error, etc.) |
| E004 | Unknown argument or invalid flag |
| E005 | Linker error |
| E006 | Verification error |
