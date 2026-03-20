use polars_core::prelude::*;
use std::alloc::{alloc, dealloc, Layout};
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

fn empty_f64_chunked(name: &str) -> Float64Chunked {
    Float64Chunked::from_slice(name.into(), &[])
}

fn empty_i32_chunked(name: &str) -> Int32Chunked {
    Int32Chunked::from_slice(name.into(), &[])
}

unsafe fn chunked_f64_from_segment_slices(name: &str, segments: &[*const f64], lengths: &[usize]) -> Float64Chunked {
    if lengths.is_empty() {
        return empty_f64_chunked(name);
    }

    let mut out = {
        let first = std::slice::from_raw_parts(segments[0], lengths[0]);
        Float64Chunked::mmap_slice(name.into(), first)
    };

    for i in 1..lengths.len() {
        let slice = std::slice::from_raw_parts(segments[i], lengths[i]);
        out.append_owned(Float64Chunked::mmap_slice(name.into(), slice)).unwrap();
    }
    out
}

unsafe fn chunked_i32_from_segment_slices(name: &str, segments: &[*const i32], lengths: &[usize]) -> Int32Chunked {
    if lengths.is_empty() {
        return empty_i32_chunked(name);
    }

    let mut out = {
        let first = std::slice::from_raw_parts(segments[0], lengths[0]);
        Int32Chunked::mmap_slice(name.into(), first)
    };

    for i in 1..lengths.len() {
        let slice = std::slice::from_raw_parts(segments[i], lengths[i]);
        out.append_owned(Int32Chunked::mmap_slice(name.into(), slice)).unwrap();
    }
    out
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

fn polars_generated_sum_offset_impl(start_row: usize, rows: usize) -> f64 {
    let feature_a: Vec<f64> = (0..rows)
        .map(|i| {
            let global_i = start_row + i;
            ((global_i % 1000) as f64) * 0.5 - 100.0
        })
        .collect();
    let label: Vec<i32> = (0..rows)
        .map(|i| {
            let global_i = start_row + i;
            if global_i % 3 == 0 || global_i % 5 == 0 { 1 } else { 0 }
        })
        .collect();
    let partition: Vec<i32> = (0..rows)
        .map(|i| ((start_row + i) % 8) as i32)
        .collect();

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
pub extern "C" fn polars_generated_sum(rows: u32) -> f64 {
    polars_generated_sum_offset_impl(0, rows as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_generated_sum_offset(start_row: u32, rows: u32) -> f64 {
    polars_generated_sum_offset_impl(start_row as usize, rows as usize)
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
pub extern "C" fn polars_from_owned_buffers(feature_a_ptr: *mut f64, label_ptr: *mut i32, partition_ptr: *mut i32, rows: u32) -> f64 {
    let rows = rows as usize;
    if feature_a_ptr.is_null() || label_ptr.is_null() || partition_ptr.is_null() {
        return f64::NAN;
    }

    let feature_a = unsafe { Vec::from_raw_parts(feature_a_ptr, rows, rows) };
    let label = unsafe { Vec::from_raw_parts(label_ptr, rows, rows) };
    let partition = unsafe { Vec::from_raw_parts(partition_ptr, rows, rows) };

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
pub extern "C" fn polars_from_chunked_segment_buffers(
    feature_ptrs_ptr: *const u32,
    label_ptrs_ptr: *const u32,
    partition_ptrs_ptr: *const u32,
    lengths_ptr: *const u32,
    segment_count: u32,
) -> f64 {
    let segment_count = segment_count as usize;
    if segment_count == 0
        || feature_ptrs_ptr.is_null()
        || label_ptrs_ptr.is_null()
        || partition_ptrs_ptr.is_null()
        || lengths_ptr.is_null()
    {
        return f64::NAN;
    }

    let feature_ptrs = unsafe { std::slice::from_raw_parts(feature_ptrs_ptr, segment_count) };
    let label_ptrs = unsafe { std::slice::from_raw_parts(label_ptrs_ptr, segment_count) };
    let partition_ptrs = unsafe { std::slice::from_raw_parts(partition_ptrs_ptr, segment_count) };
    let lengths_u32 = unsafe { std::slice::from_raw_parts(lengths_ptr, segment_count) };
    let lengths: Vec<usize> = lengths_u32.iter().map(|v| *v as usize).collect();

    if feature_ptrs.iter().any(|ptr| *ptr == 0)
        || label_ptrs.iter().any(|ptr| *ptr == 0)
        || partition_ptrs.iter().any(|ptr| *ptr == 0)
    {
        return f64::NAN;
    }

    let total_rows: usize = lengths.iter().sum();

    let feature_segments: Vec<*const f64> = feature_ptrs.iter().map(|ptr| *ptr as *const f64).collect();
    let label_segments: Vec<*const i32> = label_ptrs.iter().map(|ptr| *ptr as *const i32).collect();
    let partition_segments: Vec<*const i32> = partition_ptrs.iter().map(|ptr| *ptr as *const i32).collect();

    let feature = unsafe { chunked_f64_from_segment_slices("feature_a", &feature_segments, &lengths) };
    let label = unsafe { chunked_i32_from_segment_slices("label", &label_segments, &lengths) };
    let partition = unsafe { chunked_i32_from_segment_slices("partition", &partition_segments, &lengths) };

    let df = match DataFrame::new(
        total_rows,
        vec![
            feature.into_column(),
            label.into_column(),
            partition.into_column(),
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
pub extern "C" fn polars_free_f64_buffer(ptr: *mut f64, rows: u32) {
    if ptr.is_null() {
        return;
    }
    let rows = rows as usize;
    if let Ok(layout) = Layout::array::<f64>(rows) {
        unsafe { dealloc(ptr as *mut u8, layout) };
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

#[unsafe(no_mangle)]
pub extern "C" fn polars_free_i32_buffer(ptr: *mut i32, rows: u32) {
    if ptr.is_null() {
        return;
    }
    let rows = rows as usize;
    if let Ok(layout) = Layout::array::<i32>(rows) {
        unsafe { dealloc(ptr as *mut u8, layout) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_alloc_u32_buffer(rows: u32) -> *mut u32 {
    let rows = rows as usize;
    let layout = Layout::array::<u32>(rows).ok();
    match layout {
        Some(layout) => unsafe { alloc(layout) as *mut u32 },
        None => core::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn polars_free_u32_buffer(ptr: *mut u32, rows: u32) {
    if ptr.is_null() {
        return;
    }
    let rows = rows as usize;
    if let Ok(layout) = Layout::array::<u32>(rows) {
        unsafe { dealloc(ptr as *mut u8, layout) };
    }
}
