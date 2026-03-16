#![no_std]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn sum_f64_range(start_byte: u32, element_count: u32) -> f64 {
    let mut sum = 0.0f64;
    let mut index = 0usize;
    let base = start_byte as usize;
    let count = element_count as usize;

    while index < count {
        let ptr = (base + index * 8) as *const f64;
        let value = unsafe { ptr.read_volatile() };
        sum += value;
        index += 1;
    }

    sum
}

#[unsafe(no_mangle)]
pub extern "C" fn count_i32_eq(start_byte: u32, element_count: u32, target: i32) -> u64 {
    let mut count = 0u64;
    let mut index = 0usize;
    let base = start_byte as usize;
    let len = element_count as usize;

    while index < len {
        let ptr = (base + index * 4) as *const i32;
        let value = unsafe { ptr.read_volatile() };
        if value == target {
            count += 1;
        }
        index += 1;
    }

    count
}
