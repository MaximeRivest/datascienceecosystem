#!/usr/bin/env python3
import argparse
import os
import sys
import time
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq


def format_bytes(num: int) -> str:
    value = float(num)
    for unit in ["B", "KiB", "MiB", "GiB", "TiB"]:
        if value < 1024 or unit == "TiB":
            if unit == "B":
                return f"{int(value)} {unit}"
            return f"{value:.2f} {unit}"
        value /= 1024
    return f"{value:.2f} TiB"


def build_batch(start_id: int, rows: int, payload_bytes: int, rng: np.random.Generator) -> pa.Table:
    ids = np.arange(start_id, start_id + rows, dtype=np.int64)
    partition = rng.integers(0, 4096, size=rows, dtype=np.int32)
    feature_a = rng.normal(loc=0.0, scale=1.0, size=rows).astype(np.float64)
    feature_b = rng.normal(loc=10.0, scale=5.0, size=rows).astype(np.float64)
    feature_c = rng.uniform(low=-1000.0, high=1000.0, size=rows).astype(np.float64)
    label = rng.integers(0, 2, size=rows, dtype=np.int8)

    payload_raw = rng.bytes(rows * payload_bytes)
    payload_np = np.frombuffer(payload_raw, dtype=f"S{payload_bytes}")
    payload = pa.array(payload_np, type=pa.binary(payload_bytes))

    return pa.table(
        {
            "id": pa.array(ids),
            "partition": pa.array(partition),
            "feature_a": pa.array(feature_a),
            "feature_b": pa.array(feature_b),
            "feature_c": pa.array(feature_c),
            "label": pa.array(label),
            "payload": payload,
        }
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate a synthetic Parquet file of roughly a target size.")
    parser.add_argument("output", help="Output parquet path")
    parser.add_argument("--target-gib", type=float, default=6.0, help="Approximate target size on disk in GiB")
    parser.add_argument("--batch-rows", type=int, default=250_000, help="Rows per row group")
    parser.add_argument("--payload-bytes", type=int, default=128, help="Bytes in the fixed-size binary payload column")
    parser.add_argument("--seed", type=int, default=42, help="PRNG seed")
    args = parser.parse_args()

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)

    if output.exists():
        print(f"Refusing to overwrite existing file: {output}", file=sys.stderr)
        return 1

    target_bytes = int(args.target_gib * (1024 ** 3))
    rng = np.random.default_rng(args.seed)
    writer = None
    rows_written = 0
    batches_written = 0
    started = time.time()

    compression = "NONE"
    use_dictionary = False
    write_statistics = False

    try:
        while True:
            table = build_batch(rows_written, args.batch_rows, args.payload_bytes, rng)
            if writer is None:
                writer = pq.ParquetWriter(
                    output,
                    table.schema,
                    compression=compression,
                    use_dictionary=use_dictionary,
                    write_statistics=write_statistics,
                )

            writer.write_table(table, row_group_size=args.batch_rows)
            rows_written += table.num_rows
            batches_written += 1

            if batches_written == 1 or batches_written % 5 == 0:
                current_size = output.stat().st_size
                elapsed = time.time() - started
                rate = current_size / elapsed if elapsed > 0 else 0
                eta_seconds = max(0.0, (target_bytes - current_size) / rate) if rate > 0 and current_size < target_bytes else 0.0
                print(
                    f"batch={batches_written} rows={rows_written:,} size={format_bytes(current_size)} "
                    f"rate={format_bytes(int(rate))}/s eta={eta_seconds/60:.1f} min"
                )
                sys.stdout.flush()

            if output.stat().st_size >= target_bytes:
                break
    finally:
        if writer is not None:
            writer.close()

    final_size = output.stat().st_size
    elapsed = time.time() - started
    print("done")
    print(f"output={output}")
    print(f"rows={rows_written:,}")
    print(f"row_groups={batches_written}")
    print(f"size={format_bytes(final_size)} ({final_size} bytes)")
    print(f"elapsed={elapsed/60:.1f} min")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
