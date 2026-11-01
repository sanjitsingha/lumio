//! Minimal registry helpers (string and DWORD values).

use windows::core::HSTRING;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};

pub fn read_string(root: HKEY, key: &str, value: &str) -> Option<String> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut size: u32 = 0;
    let status = unsafe { RegGetValueW(root, &key, &value, RRF_RT_REG_SZ, None, None, Some(&mut size)) };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe {
        RegGetValueW(root, &key, &value, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size))
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

pub fn read_dword(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(root, &key, &value, RRF_RT_REG_DWORD, None, Some((&mut data as *mut u32).cast()), Some(&mut size))
    };
    (status == ERROR_SUCCESS).then_some(data)
}

pub fn write_string(root: HKEY, key: &str, value: &str, data: &str) -> Result<(), String> {
    let wide: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
    let status = unsafe {
        RegSetKeyValueW(
            root,
            &HSTRING::from(key),
            &HSTRING::from(value),
            REG_SZ.0,
            Some(wide.as_ptr().cast()),
            (wide.len() * 2) as u32,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("registry write failed ({})", status.0))
    }
}

pub fn delete_value(root: HKEY, key: &str, value: &str) -> Result<(), String> {
    let status = unsafe { RegDeleteKeyValueW(root, &HSTRING::from(key), &HSTRING::from(value)) };
    match status.0 {
        0 | 2 => Ok(()), // ERROR_SUCCESS, ERROR_FILE_NOT_FOUND
        code => Err(format!("registry delete failed ({code})")),
    }
}
