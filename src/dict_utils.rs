use core_foundation::base::{TCFType, ToVoid};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::number::{kCFNumberSInt32Type, CFNumberGetValue, CFNumberRef};
use core_foundation_sys::string::CFStringRef;

#[allow(unused)]
pub fn get_dict_keys(dict: &CFDictionary) -> Vec<String> {
    let (keys, values) = dict.get_keys_and_values();
    let length = dict.len();

    let mut results = Vec::new();
    for i in 0..length {
        results.push(unsafe { CFString::wrap_under_get_rule(keys[i] as CFStringRef) }.to_string());
    }
    results
}

pub fn get_string_from_dict(dict: &CFDictionary, key: &str) -> String {
    let key = CFString::new(key);
    match dict.find(key.to_void()) {
        Some(value) => unsafe { CFString::wrap_under_get_rule(*value as CFStringRef) }.to_string(),
        None => return String::new(),
    }
}

pub fn get_num_from_dict<T>(dict: &CFDictionary, key: &str) -> T
where
    T: Default,
{
    let key = CFString::new(key);
    let value = match dict.find(key.to_void()) {
        Some(n) => n,
        None => return T::default(),
    };

    let mut result: T = T::default();
    let out_value: *mut T = &mut result;
    unsafe { CFNumberGetValue(*value as CFNumberRef, kCFNumberSInt32Type, out_value.cast()) };
    result
}

pub fn get_dict_from_dict(dict: &CFDictionary, key: &str) -> Option<CFDictionary> {
    let key = CFString::new(key);
    let value = match dict.find(key.to_void()) {
        Some(n) => n,
        None => return None,
    };
    Some(unsafe { CFDictionary::wrap_under_get_rule(*value as CFDictionaryRef) })
}
