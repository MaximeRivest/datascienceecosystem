/* tslint:disable */
/* eslint-disable */

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly polars_smoke_test: () => number;
    readonly polars_generated_sum: (a: number) => number;
    readonly polars_generated_sum_offset: (a: number, b: number) => number;
    readonly polars_generated_label1_count: (a: number) => number;
    readonly polars_last_partition0_count: () => number;
    readonly polars_last_partition1_count: () => number;
    readonly polars_from_buffers: (a: number, b: number, c: number, d: number) => number;
    readonly polars_from_owned_buffers: (a: number, b: number, c: number, d: number) => number;
    readonly polars_from_chunked_segment_buffers: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly polars_last_label1_count: () => number;
    readonly polars_alloc_f64_buffer: (a: number) => number;
    readonly polars_free_f64_buffer: (a: number, b: number) => void;
    readonly polars_alloc_i32_buffer: (a: number) => number;
    readonly polars_free_i32_buffer: (a: number, b: number) => void;
    readonly polars_alloc_u32_buffer: (a: number) => number;
    readonly polars_free_u32_buffer: (a: number, b: number) => void;
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
