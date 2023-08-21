use core::ffi::c_char;

pub fn bricks_memcpy(dst: *mut c_char, src: *mut c_char, cnt: u32) {
    let mut dst_cp = dst;
    let mut src_cp = src;
    for _ in 0..cnt {
        unsafe {
            *dst_cp = *src_cp;
        }
        dst_cp = ((dst_cp as u64) + 1) as *mut c_char;
        src_cp = ((src_cp as u64) + 1) as *mut c_char;
    }
}

pub fn bricks_strlen(str: *mut c_char) -> u32 {
    let mut buff_cpy = str;
    let mut cnt_chars = 0;
    loop {
        cnt_chars += 1;
        unsafe {
            if *buff_cpy == ('\0' as i8) {
                break;
            }
        }
        buff_cpy = ((buff_cpy as u64) + 1) as *mut c_char;
    }
    cnt_chars
}
