use polars_core::prelude::*;
use std::alloc::{alloc, Layout};
use std::sync::Mutex;

#[derive(Clone, Copy, Default)]
struct LastResult {
    sum_feature_a_label1: f64,
    label1_count: u32,
    partition0_count: u32,
    partition1_count: u32,
}

static LAST_RESULT: Mutex<LastResult> = Mutex::new(LastResult {
    sum_feature_a_label1: 0.0,
    label1_count: 0,
    partition0_count: 0,
    partition1_count: 0,
});

fn compute_metrics(df: &DataFrame) -> LastResult {
    let feature = df.column("feature_a").unwrap().f64().unwrap();
    let label = df.column("label").unwrap().i32().unwrap();
    let partition = df.column("partition").unwrap().i32().unwrap();

    let mut out = LastResult::default();
    for i in 0..df.height() {
        let partition_value = partition.get(i).unwrap_or_default();
        if partition_value == 0 {
            out.partition0_count += 1;
        } else if partition_value == 1 {
            out.partition1_count += 1;
        }

        if label.get(i) == Some(1) {
            out.label1_count += 1;
            out.sum_feature_a_label1 += feature.get(i).unwrap_or(0.0);
        }
    }
    out
}

fn store_result(result: LastResult) {
    if let Ok(mut slot) = LAST_RESULT.lock() {
        *slot = result;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_smoke_test() -> f64 {
    let s1 = Column::new("feature_a".into(), &[1.0f64, 2.0, 3.5, -1.0]);
    let s2 = Column::new("label".into(), &[0i32, 1, 1, 0]);
    let s3 = Column::new("partition".into(), &[0i32, 0, 1, 1]);
    let df = DataFrame::new(4, vec![s1, s2, s3]).unwrap();
    let result = compute_metrics(&df);
    store_result(result);
    result.sum_feature_a_label1
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_generated_sum(rows: u32) -> f64 {
    let rows = rows as usize;
    let feature_a: Vec<f64> = (0..rows)
        .map(|i| ((i % 1000) as f64) * 0.5 - 100.0)
        .collect();
    let label: Vec<i32> = (0..rows)
        .map(|i| if i % 3 == 0 || i % 5 == 0 { 1 } else { 0 })
        .collect();
    let partition: Vec<i32> = (0..rows).map(|i| (i % 8) as i32).collect();

    let df = DataFrame::new(
        rows,
        vec![
            Column::new("feature_a".into(), feature_a),
            Column::new("label".into(), label),
            Column::new("partition".into(), partition),
        ],
    )
    .unwrap();

    let result = compute_metrics(&df);
    store_result(result);
    result.sum_feature_a_label1
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_generated_label1_count(rows: u32) -> u32 {
    let _ = polars_generated_sum(rows);
    LAST_RESULT.lock().map(|v| v.label1_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_last_partition0_count() -> u32 {
    LAST_RESULT.lock().map(|v| v.partition0_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_last_partition1_count() -> u32 {
    LAST_RESULT.lock().map(|v| v.partition1_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_from_buffers(feature_a_ptr: *const f64, label_ptr: *const i32, partition_ptr: *const i32, rows: u32) -> f64 {
    let rows = rows as usize;
    if feature_a_ptr.is_null() || label_ptr.is_null() || partition_ptr.is_null() {
        return f64::NAN;
    }

    let feature_a = unsafe { std::slice::from_raw_parts(feature_a_ptr, rows) }.to_vec();
    let label = unsafe { std::slice::from_raw_parts(label_ptr, rows) }.to_vec();
    let partition = unsafe { std::slice::from_raw_parts(partition_ptr, rows) }.to_vec();

    let df = match DataFrame::new(
        rows,
        vec![
            Column::new("feature_a".into(), feature_a),
            Column::new("label".into(), label),
            Column::new("partition".into(), partition),
        ],
    ) {
        Ok(df) => df,
        Err(_) => return f64::NAN,
    };

    let result = compute_metrics(&df);
    store_result(result);
    result.sum_feature_a_label1
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_last_label1_count() -> u32 {
    LAST_RESULT.lock().map(|v| v.label1_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_alloc_f64_buffer(rows: u32) -> *mut f64 {
    let rows = rows as usize;
    let layout = Layout::array::<f64>(rows).ok();
    match layout {
        Some(layout) => unsafe { alloc(layout) as *mut f64 },
        None => core::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_alloc_i32_buffer(rows: u32) -> *mut i32 {
    let rows = rows as usize;
    let layout = Layout::array::<i32>(rows).ok();
    match layout {
        Some(layout) => unsafe { alloc(layout) as *mut i32 },
        None => core::ptr::null_mut(),
    }
}
