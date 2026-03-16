use std::io::Cursor;
use std::sync::Mutex;

use arrow_array::{Array, Float64Array, Int32Array, RecordBatch};
use arrow_ipc::reader::StreamReader;

#[derive(Clone, Copy, Default)]
struct Aggregates {
    row_count: u64,
    sum_feature_a: f64,
    sum_feature_b: f64,
    sum_feature_c: f64,
    label_0: u64,
    label_1: u64,
}

static LAST_AGGREGATES: Mutex<Aggregates> = Mutex::new(Aggregates {
    row_count: 0,
    sum_feature_a: 0.0,
    sum_feature_b: 0.0,
    sum_feature_c: 0.0,
    label_0: 0,
    label_1: 0,
});

fn compute_batch_aggregates(batch: &RecordBatch) -> Result<Aggregates, String> {
    let feature_a = batch
        .column_by_name("feature_a")
        .ok_or_else(|| "missing column feature_a".to_string())?
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| "feature_a is not Float64".to_string())?;

    let feature_b = batch
        .column_by_name("feature_b")
        .ok_or_else(|| "missing column feature_b".to_string())?
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| "feature_b is not Float64".to_string())?;

    let feature_c = batch
        .column_by_name("feature_c")
        .ok_or_else(|| "missing column feature_c".to_string())?
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| "feature_c is not Float64".to_string())?;

    let label = batch
        .column_by_name("label")
        .ok_or_else(|| "missing column label".to_string())?
        .as_any()
        .downcast_ref::<Int32Array>()
        .ok_or_else(|| "label is not Int32".to_string())?;

    let mut result = Aggregates {
        row_count: batch.num_rows() as u64,
        ..Aggregates::default()
    };

    for i in 0..batch.num_rows() {
        if !feature_a.is_null(i) {
            result.sum_feature_a += feature_a.value(i);
        }
        if !feature_b.is_null(i) {
            result.sum_feature_b += feature_b.value(i);
        }
        if !feature_c.is_null(i) {
            result.sum_feature_c += feature_c.value(i);
        }
        if !label.is_null(i) {
            match label.value(i) {
                0 => result.label_0 += 1,
                1 => result.label_1 += 1,
                _ => {}
            }
        }
    }

    Ok(result)
}

fn parse_stream(start_byte: u32, byte_len: u32) -> Result<Aggregates, String> {
    let start = start_byte as usize;
    let len = byte_len as usize;
    let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, len) };
    let cursor = Cursor::new(bytes);
    let mut reader = StreamReader::try_new(cursor, None).map_err(|e| e.to_string())?;

    let mut total = Aggregates::default();
    for batch in &mut reader {
        let batch = batch.map_err(|e| e.to_string())?;
        let agg = compute_batch_aggregates(&batch)?;
        total.row_count += agg.row_count;
        total.sum_feature_a += agg.sum_feature_a;
        total.sum_feature_b += agg.sum_feature_b;
        total.sum_feature_c += agg.sum_feature_c;
        total.label_0 += agg.label_0;
        total.label_1 += agg.label_1;
    }
    Ok(total)
}

#[unsafe(no_mangle)]
pub extern "C" fn parse_arrow_ipc_batch(start_byte: u32, byte_len: u32) -> u32 {
    match parse_stream(start_byte, byte_len) {
        Ok(aggregates) => {
            if let Ok(mut slot) = LAST_AGGREGATES.lock() {
                *slot = aggregates;
            }
            0
        }
        Err(_) => 1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn last_row_count() -> u64 {
    LAST_AGGREGATES.lock().map(|v| v.row_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn last_sum_feature_a() -> f64 {
    LAST_AGGREGATES.lock().map(|v| v.sum_feature_a).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn last_sum_feature_b() -> f64 {
    LAST_AGGREGATES.lock().map(|v| v.sum_feature_b).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn last_sum_feature_c() -> f64 {
    LAST_AGGREGATES.lock().map(|v| v.sum_feature_c).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn last_label_0() -> u64 {
    LAST_AGGREGATES.lock().map(|v| v.label_0).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn last_label_1() -> u64 {
    LAST_AGGREGATES.lock().map(|v| v.label_1).unwrap_or(0)
}
