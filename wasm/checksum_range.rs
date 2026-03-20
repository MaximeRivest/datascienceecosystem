#![no_std]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn checksum_u8_range(start_byte: u32, byte_len: u32) -> u64 {
    let start = start_byte as usize;
    let end = start.saturating_add(byte_len as usize);
    let mut checksum: u64 = 0;
    let mut index = start;

    while index < end {
        let ptr = index as *const u8;
        let value = unsafe { ptr.read_volatile() };
        checksum = checksum.wrapping_add(value as u64);
        index += 1;
    }

    checksum
}

#[unsafe(no_mangle)]
pub extern "C" fn checksum_u8_range_stride(start_byte: u32, byte_len: u32, stride: u32) -> u64 {
    let start = start_byte as usize;
    let end = start.saturating_add(byte_len as usize);
    if start >= end {
        return 0;
    }

    let step = core::cmp::max(stride as usize, 1);
    let mut checksum: u64 = 0;
    let mut index = start;

    while index < end {
        let ptr = index as *const u8;
        let value = unsafe { ptr.read_volatile() };
        checksum = checksum.wrapping_add(value as u64);
        index = index.saturating_add(step);
    }

    let last_index = end - 1;
    if ((last_index - start) % step) != 0 {
        let ptr = last_index as *const u8;
        let value = unsafe { ptr.read_volatile() };
        checksum = checksum.wrapping_add(value as u64);
    }

    checksum
}
