use cocoa::appkit::NSScreen;
use cocoa_foundation::base::nil;
use cocoa_foundation::foundation::{NSArray, NSDictionary, NSString};
use core_foundation::base::*;
use core_foundation::number::*;
use core_foundation::string::*;
use core_foundation_sys::dictionary::{CFDictionaryGetCount, CFDictionaryGetKeysAndValues};
use core_foundation_sys::string::{kCFStringEncodingUTF8, CFStringGetCStringPtr, CFStringRef};
use core_graphics::display::*;
use core_graphics_types::base::CGFloat;
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Root};
use serde::{Deserialize, Serialize};
use std::ffi::{c_void, CStr};

// #[derive(Debug, Clone, Default, Serialize, Deserialize)]
// struct Rect {
//     pub x: i32,
//     pub y: i32,
//     pub w: i32,
//     pub h: i32,
// }
//
// impl Into<CGRect> for Rect {
//     fn into(self) -> CGRect {
//         CGRect {
//             origin: CGPoint {
//                 x: self.x.into(),
//                 y: self.y.into(),
//             },
//             size: CGSize {
//                 width: self.w.into(),
//                 height: self.h.into(),
//             },
//         }
//     }
// }
//
// impl Rect {
//     fn origin(&self) -> CGPoint {
//         CGPoint {
//             x: self.x as CGFloat,
//             y: self.y as CGFloat,
//         }
//     }
// }
//
// impl From<CGRect> for Rect {
//     fn from(value: CGRect) -> Self {
//         Rect {
//             x: value.origin.x as i32,
//             y: value.origin.y as i32,
//             w: value.size.width as i32,
//             h: value.size.height as i32,
//         }
//     }
// }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WindowLayout {
    pub owner_name: String,
    pub name: String,
    pub display_id: u32,
    // pub bounds: Rect,
    pub bounds: Vec<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ScreenLayout {
    pub id: u32,
    // pub frame: Rect,
    pub frame: Vec<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Layout {
    pub screens: Vec<ScreenLayout>,
    pub windows: Vec<WindowLayout>,
}

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    name: String,

    /// Number of times to greet
    #[arg(short, long, default_value_t = 1)]
    count: u8,
}

fn main() {
    initialize_logging();

    let layout = Layout {
        screens: get_screen_layouts(),
        windows: get_window_layouts(),
    };

    println!("{}", serde_json::to_string_pretty(&layout).unwrap());
}

fn initialize_logging() {
    let log_appender = Appender::builder().build(
        "stdout".to_string(),
        Box::new(ConsoleAppender::builder().build()),
    );
    let log_root = Root::builder()
        .appender("stdout".to_string())
        .build(log::LevelFilter::Debug); // TODO: make configurable
    let log_config = log4rs::config::Config::builder()
        .appender(log_appender)
        .build(log_root);

    // Keeping the redundant prefix here to make it clear just what "config" is being initialized here.
    log4rs::config::init_config(log_config.unwrap()).unwrap();
}

fn get_window_layouts() -> Vec<WindowLayout> {
    const OPTIONS: CGWindowListOption =
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let window_list_info = unsafe { CGWindowListCopyWindowInfo(OPTIONS, kCGNullWindowID) };

    let mut window_list = Vec::new();

    let count = unsafe { CFArrayGetCount(window_list_info) };
    for i in 0..count {
        let mut window_info = WindowLayout::default();

        let window_dict =
            unsafe { CFArrayGetValueAtIndex(window_list_info, i as isize) as CFDictionaryRef };

        //debug!("Window keys: {:?}", get_dict_keys(window_dict));

        window_info.owner_name = get_string_from_dict(window_dict, "kCGWindowOwnerName");
        window_info.name = get_string_from_dict(window_dict, "kCGWindowName");
        let bounds = get_dict_from_dict(window_dict, "kCGWindowBounds");
        if bounds.is_null() {
            continue;
        }
        //debug!("Bounds keys: {:?}", get_dict_keys(bounds));
        window_info.bounds[0] = get_i32_from_dict(bounds, "X");
        window_info.bounds[1] = get_i32_from_dict(bounds, "Y");
        window_info.bounds[2] = get_i32_from_dict(bounds, "Width");
        window_info.bounds[3] = get_i32_from_dict(bounds, "Height");

        let (display_id, adjusted_bounds) = screen_origin_for_window(&window_info.bounds);
        window_info.display_id = display_id;
        window_info.bounds = adjusted_bounds.into();

        window_list.push(window_info);
    }

    unsafe { CFRelease(window_list_info as CFTypeRef) }

    window_list
}

fn get_screen_layouts() -> Vec<ScreenLayout> {
    let mut screen_layouts = Vec::new();

    unsafe {
        let screens = NSScreen::screens(nil);
        for i in 0..NSArray::count(screens) {
            let screen = screens.objectAtIndex(i);
            let frame = screen.frame();
            let device_desc = screen.deviceDescription();
            // debug!("device_desc keys: {:?}", get_dict_keys(device_desc as CFDictionaryRef));

            screen_layouts.push(ScreenLayout {
                id: get_i32_from_dict(device_desc as CFDictionaryRef, "NSScreenNumber") as u32,
                frame: vec![
                    frame.origin.x as i32,
                    frame.origin.y as i32,
                    frame.size.width as i32,
                    frame.size.height as i32,
                ],
            });
        }
    }
    screen_layouts
}

fn get_dict_keys(dict: CFDictionaryRef) -> Vec<String> {
    let length = unsafe { CFDictionaryGetCount(dict) as usize };

    let mut keys = Vec::with_capacity(length);
    let mut values = Vec::with_capacity(length);

    unsafe {
        CFDictionaryGetKeysAndValues(dict, keys.as_mut_ptr(), values.as_mut_ptr());
        keys.set_len(length);
    }

    let mut results = Vec::new();
    for i in 0..length {
        results.push(string_ref_to_string(keys[i] as CFStringRef));
    }
    results
}

fn get_string_from_dict(dict: CFDictionaryRef, key: &str) -> String {
    let key = CFString::new(key);
    let mut value: *const c_void = std::ptr::null();
    if unsafe { CFDictionaryGetValueIfPresent(dict, key.to_void(), &mut value) != 0 } {
        let cf_ref = value as CFStringRef;
        let c_ptr = unsafe { CFStringGetCStringPtr(cf_ref, kCFStringEncodingUTF8) };
        if !c_ptr.is_null() {
            let c_result = unsafe { CStr::from_ptr(c_ptr) };
            return String::from(c_result.to_str().unwrap());
        }
    }
    String::new()
}

fn get_i32_from_dict(dict: CFDictionaryRef, key: &str) -> i32 {
    let mut value_i32 = 0_i32;
    let key = CFString::new(key);
    let mut value: *const c_void = std::ptr::null();
    if unsafe { CFDictionaryGetValueIfPresent(dict, key.to_void(), &mut value) != 0 } {
        let value = value as CFNumberRef;
        let out_value: *mut i32 = &mut value_i32;
        unsafe { CFNumberGetValue(value, kCFNumberSInt32Type, out_value.cast()) };
    }
    value_i32
}

fn get_dict_from_dict(dict: CFDictionaryRef, key: &str) -> CFDictionaryRef {
    let key = CFString::new(key);
    let mut value: *const c_void = std::ptr::null();
    unsafe { CFDictionaryGetValueIfPresent(dict, key.to_void(), &mut value) };
    value as CFDictionaryRef
}

/// Determines which screen the given window resides on.
///
/// - parameter windowBounds: the window's rectangle
///
/// - returns: the display ID and the rectangle for the window's screen.
fn screen_origin_for_window(window_bounds: &Vec<i32>) -> (CGDirectDisplayID, Vec<i32>) {
    unsafe {
        let main_screen = NSScreen::mainScreen(nil);
        let main_screen_rect = NSScreen::frame(main_screen);

        let screens = NSScreen::screens(nil);
        for i in 0..NSArray::count(screens) {
            let mut screen_rect = NSScreen::frame(screens.objectAtIndex(i))
                .as_CGRect()
                .clone();
            // Unbelievably, NSScreen coordinates are different from CGWindow coordinates! NSScreen 0,0 is bottom-left,
            // and CGWindow is top-left. O.o
            screen_rect.origin.y = /*no NSMaxY?*/ (main_screen_rect.origin.y + main_screen_rect.size.height)
                - (screen_rect.origin.y + screen_rect.size.height);

            if screen_rect.contains(&CGPoint {
                x: window_bounds[0] as CGFloat,
                y: window_bounds[1] as CGFloat,
            }) {
                return (
                    i as u32,
                    vec![
                        screen_rect.origin.x as i32,
                        screen_rect.origin.y as i32,
                        screen_rect.size.width as i32,
                        screen_rect.size.height as i32,
                    ],
                );
            }
        }
    }
    (0, vec![0, 0, 0, 0])
}

// From https://github.com/hahnlee/canter
fn string_ref_to_string(string_ref: CFStringRef) -> String {
    // reference: https://github.com/servo/core-foundation-rs/blob/355740/core-foundation/src/string.rs#L49
    unsafe {
        let char_ptr = CFStringGetCStringPtr(string_ref, kCFStringEncodingUTF8);
        if !char_ptr.is_null() {
            let c_str = CStr::from_ptr(char_ptr);
            return String::from(c_str.to_str().unwrap());
        }

        let char_len = CFStringGetLength(string_ref);

        let mut bytes_required: CFIndex = 0;
        CFStringGetBytes(
            string_ref,
            CFRange {
                location: 0,
                length: char_len,
            },
            kCFStringEncodingUTF8,
            0,
            false as Boolean,
            std::ptr::null_mut(),
            0,
            &mut bytes_required,
        );

        // Then, allocate the buffer and actually copy.
        let mut buffer = vec![b'\x00'; bytes_required as usize];

        let mut bytes_used: CFIndex = 0;
        CFStringGetBytes(
            string_ref,
            CFRange {
                location: 0,
                length: char_len,
            },
            kCFStringEncodingUTF8,
            0,
            false as Boolean,
            buffer.as_mut_ptr(),
            buffer.len() as CFIndex,
            &mut bytes_used,
        );

        return String::from_utf8_unchecked(buffer);
    }
}
