/* tslint:disable */
/* eslint-disable */

/**
 * Parse-only check: validates Salt syntax without full compilation.
 * Faster than `compile()` — use for real-time editor feedback on keystroke.
 */
export function check(source: string): string;

/**
 * Compile Salt source code and return structured JSON.
 *
 * # Arguments
 * * `source` - Salt source code string
 *
 * # Returns
 * JSON string with shape `{ success: bool, mlir: string, diagnostics: [...] }`
 */
export function compile(source: string): string;

/**
 * Run a Salt program and return its stdout output.
 *
 * Parses the source, then executes it via the AST interpreter.
 * Returns JSON with shape `{ success: bool, stdout: string, exit_code: number, error: string }`
 */
export function run(source: string): string;

/**
 * Return the compiler version string.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly check: (a: number, b: number) => [number, number];
    readonly compile: (a: number, b: number) => [number, number];
    readonly run: (a: number, b: number) => [number, number];
    readonly version: () => [number, number];
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
