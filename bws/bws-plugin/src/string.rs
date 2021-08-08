#[repr(C)]
pub struct BwsString {
    ptr: *mut u8,
    length: usize,
    capacity: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BwsStr {
    ptr: *const u8,
    length: usize,
}

impl BwsString {
    pub fn from_string(mut s: String) -> Self {
        let bws_string = Self {
            ptr: s.as_mut_str().as_mut_ptr(),
            length: s.len(),
            capacity: s.capacity(),
        };

        Box::leak(s.into_boxed_str());

        bws_string
    }
    pub unsafe fn into_string(self) -> String {
        String::from_raw_parts(self.ptr, self.length, self.capacity)
    }
}

impl BwsStr {
    pub fn from_str(s: &str) -> Self {
        Self {
            ptr: s.as_bytes().as_ptr(),
            length: s.len(),
        }
    }
    pub unsafe fn into_str<'a>(self) -> &'a str {
        &std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.ptr, self.length))
    }
}
