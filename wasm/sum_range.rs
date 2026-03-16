#![no_std]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn sum_i32_range(start_index: u32, end_index: u32) -> i64 {
    let mut sum: i64 = 0;
    let mut index = start_index as usize;
    let end = end_index as usize;

    while index < end {
        let ptr = (index * 4) as *const i32;
        let value = unsafe { ptr.read_volatile() };
        sum += value as i64;
        index += 1;
    }

    sum
}
