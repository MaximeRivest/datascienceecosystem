# Technical Specification & Project Brief

**Project:** Browser-Native Distributed Data Science Ecosystem
**Document Type:** Architecture Overview & Minimum Viable Test (MVT) Blueprint

---

## 1. Executive Summary

The objective is to engineer a local-first, Progressive Web App (PWA) data science notebook capable of executing heavy linear algebra, machine learning (e.g., Random Forests), and data aggregations entirely within a Chromium-based browser.

This platform will bypass traditional browser constraints—specifically memory limits and background tab throttling—by synthesizing bleeding-edge web APIs. The end goal is to deliver cloud-cluster performance using the user's local CPU/GPU without requiring backend compute, data uploads, or native desktop installations.

## 2. Target Architecture & Tech Stack

The ecosystem relies on building a distributed compute cluster inside a single browser tab.

* **Frontend Interface:** mrmd-editor
* **Data Ingestion:** File System Access API (`window.showDirectoryPicker`) for direct local disk read/write batching.
* **SQL/Data Engine:** DuckDB-Wasm and Polars (compiled to Wasm).
* **Memory Management:** `SharedArrayBuffer` (SAB) combined with Apache Arrow for zero-copy data structures across threads.
* **Compute Nodes:** `navigator.hardwareConcurrency` mapped to a pool of Web Workers running custom Rust/C++ compiled to WebAssembly (Wasm) and WebGPU compute shaders.
* **Persistence (Anti-Throttling):** Document Picture-in-Picture (PiP) API paired with the Screen Wake Lock API to prevent background throttling during long-running tasks.

---

## 3. Infrastructure Prerequisites

To unlock the necessary memory APIs (specifically `SharedArrayBuffer` to prevent Spectre attacks), the hosting environment must enforce strict Cross-Origin Isolation.

The web server **must** be configured to deliver the following HTTP headers:

* `Cross-Origin-Opener-Policy: same-origin`
* `Cross-Origin-Embedder-Policy: require-corp`
* **Note:** The application must be served over HTTPS (or `localhost` for development).

---

## 4. Phase 1: Minimum Viable Tests (MVTs)

Before developing the notebook UI or compiling complex ML algorithms, the engineering team must validate the core browser mechanics. The following 5 MVTs must be built as isolated, bare-bones JavaScript prototypes to prove technical feasibility.

### MVT 1: The Zero-Copy Memory Proof

**Goal:** Prove multiple Web Workers can read/mutate a massive single block of memory simultaneously without duplicating data and crashing the tab.

* **Setup:** On the main thread, allocate a 1GB `SharedArrayBuffer` filled with integers.
* **Execution:** Spawn 4 Web Workers. Use `postMessage()` to pass the SAB reference to all workers. Assign each worker a distinct 250MB chunk to sum.
* **Success Criteria:** Chrome Task Manager shows the tab's memory footprint remaining stable at ~1GB throughout the computation.
* **Failure Criteria:** Memory spikes to ~4GB or the browser crashes (OOM error).

### MVT 2: The Anti-Throttling Proof (Persistence Anchor)

**Goal:** Prove a 10+ minute compute job will survive when the user minimizes the browser window.

* **Setup:** Create a Web Worker running a heavy `while` loop, logging a timestamp via `performance.now()` to the console every 1.0 seconds.
* **Execution:** 1. Trigger `window.documentPictureInPicture.requestWindow()` via a button click.
2. Request `navigator.wakeLock.request('screen')` from within the PiP window's context.
3. Start the Web Worker.
4. Minimize the main browser window completely for 5 minutes.
* **Success Criteria:** Upon restoring the window, the console logs show exactly 300 sequential timestamps with zero throttling gaps.

### MVT 3: The Local I/O Pipeline Proof

**Goal:** Prove silent, programmatic read/write access to a local directory for batch processing.

* **Setup:** Create a local folder containing a dummy 10MB `.csv` file.
* **Execution:** Invoke `window.showDirectoryPicker({ mode: 'readwrite' })`. Write a script that streams the `.csv` file into memory, modifies a column, and writes a new `processed_data.csv` back to the exact same folder.
* **Success Criteria:** The new file appears on the local hard drive without triggering a "Save As" dialogue or secondary browser prompts.

### MVT 4: WebAssembly Speed Benchmark

**Goal:** Quantify the Wasm sandbox overhead for heavy math.

* **Setup:** Write a basic Rust function that performs a 1000x1000 matrix multiplication.
* **Execution:** Compile to a native desktop binary and to a `.wasm` binary (via `wasm-pack`). Run the native binary in a terminal and the Wasm binary in the browser.
* **Success Criteria:** The Wasm execution time is within 1.2x to 1.5x of the native execution time.

### MVT 5: WebGPU Memory Handoff (Optional but Recommended)

**Goal:** Prove data can move rapidly from system RAM (SAB) to GPU VRAM for parallel compute.

* **Setup:** Take a data chunk from the MVT 1 `SharedArrayBuffer`.
* **Execution:** Inside a Web Worker, map a `GPUBuffer`, copy the SAB chunk into VRAM, execute a WebGPU Compute Shader to multiply the matrix, and copy the results back to the SAB.
* **Success Criteria:** The round-trip VRAM transfer and compute time is significantly faster than executing the same multiplication sequentially on the Web Worker's CPU.

---

## 5. Phase 2: System Integration (Post-MVT)

Upon successful validation of the MVTs, the engineering team will proceed to assemble the core ecosystem:

1. **UI/UX:** Develop the notebook interface (cell execution, markdown rendering, chart plotting).
2. **Data Glue:** Integrate Apache Arrow to serve as the unified, zero-copy data format sitting inside the `SharedArrayBuffer`.
3. **Engine Routing:** Plumb DuckDB-Wasm to read from the File System Access API and output Arrow tables directly into the SAB.
4. **ML Integration:** Compile the required linear algebra and ML algorithms (e.g., Random Forest) into Wasm, configuring them to read the Arrow memory addresses directly.

---

Would you like me to draft the boilerplate JavaScript and HTML for MVT 1 (The Zero-Copy Memory Proof) so you can include a starting code snippet in your hand-off to the dev team?

---

## 6. MVT 1 Prototype Included

This repository now includes a single-file MVT 1 prototype:

- `index.html` — browser UI + inline Web Worker source for the zero-copy `SharedArrayBuffer` test
- `wasm-workers.html` — shared `WebAssembly.Memory` test with 4 Wasm-backed workers
- `wasm/sum_range.wasm` — tiny Rust-compiled Wasm module used by `wasm-workers.html`
- `serve.mjs` — tiny local static server that sets the required COOP/COEP headers

### Run locally

Requirements:

- Node.js 18+
- Chromium-based browser

Start the local server:

```bash
node serve.mjs
```

Then open:

```text
http://127.0.0.1:8787/
```

If you want a different port:

```bash
PORT=9090 node serve.mjs
```

### What the demo does

1. Allocates one `SharedArrayBuffer` on the main thread
2. Fills it with `Int32` values
3. Spawns multiple Web Workers from inline code via `Blob`
4. Gives each worker a distinct range of the same shared memory
5. Sums all chunks and verifies the final result

### How to validate MVT 1

- Start with `256 MiB`
- Increase to `1024 MiB` if your machine can handle it
- Keep worker count at `4` for the target test
- Open Chrome Task Manager while the computation runs

Success signal:

- memory stays roughly near the chosen shared allocation size instead of scaling with worker count

Failure signal:

- memory rises as if each worker received its own full copy, or the tab crashes

### Notes

- `SharedArrayBuffer` only works when `crossOriginIsolated === true`
- opening `index.html` directly with `file://` will not work
- the included `serve.mjs` sets:
  - `Cross-Origin-Opener-Policy: same-origin`
  - `Cross-Origin-Embedder-Policy: require-corp`
  - `Cross-Origin-Resource-Policy: same-origin`
- the on-page heap metric is Chrome-only and is not a substitute for Chrome Task Manager process memory

### Wasm worker follow-up

Open:

```text
http://127.0.0.1:8787/wasm-workers.html
```

This prototype validates that the shared-memory pattern can be carried into Wasm-backed workers by using a shared `WebAssembly.Memory` imported into each worker's Wasm instance.

Important limit:

- a single shared Wasm memory is commonly limited to `<= 4 GiB` in current Chromium-class browsers
- an `8192 MiB` request may fail even on a machine with plenty of RAM
- if `8192 MiB` fails, that does **not** mean 8 GiB total browser RAM usage is impossible; it means you likely need multiple shards/buffers instead of one shared Wasm memory
