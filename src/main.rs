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

/*
use std::fs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
struct ScreenLayout {
    display_id: u32,
    frame: Frame,
}

#[derive(Debug, Serialize, Deserialize)]
struct WindowLayout {
    kcg_window_owner_name: String,
    kcg_window_name: String,
    display_id: u32,
    kcg_window_bounds: Frame,
}

#[derive(Debug, Serialize, Deserialize)]
struct Frame {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn debug_log(message: &str, debug_logging: bool) {
    if debug_logging {
        println!("{}", message.blue().bold());
    }
}

fn get_cg_window_list() -> Vec<std::collections::HashMap<String, serde_json::Value>> {
    let list_options = vec![
        serde_json::Value::String(String::from("kCGWindowListOptionExcludeDesktopElements")),
        serde_json::Value::String(String::from("kCGWindowListOptionOnScreenOnly")),
    ];

    let cg_window_list: Vec<std::collections::HashMap<String, serde_json::Value>> =
        serde_json::from_str(
            &format!(
                "{}",
                quartz::cg_window_list_copy_window_info(list_options, 0).unwrap()
            ),
        )
        .unwrap();
    cg_window_list
}

fn save_screen_bounds() {
    println!("  \"screens\": [");

    for screen in quartz::nsscreen::NSScreen::screens() {
        let frame = screen.frame();
        let screen_layout = ScreenLayout {
            display_id: screen.device_description().device_id(),
            frame: Frame {
                x: frame.origin.x as i32,
                y: frame.origin.y as i32,
                width: frame.size.width as i32,
                height: frame.size.height as i32,
            },
        };

        println!(
            "    {}",
            serde_json::to_string_pretty(&screen_layout).unwrap()
        );
    }

    println!("  ],");
}

fn screen_origin_for_window(window_bounds: Frame) -> (u32, Frame) {
    let main_screen_rect = quartz::nsscreen::NSScreen::screens()[0].frame();
    for screen in quartz::nsscreen::NSScreen::screens() {
        let mut screen_rect = screen.frame();

        screen_rect.origin.y = main_screen_rect.origin.y + main_screen_rect.size.height
            - screen_rect.origin.y
            - screen_rect.size.height;

        if screen_rect.contains_point(window_bounds.x as f64, window_bounds.y as f64) {
            return (
                screen.device_description().device_id(),
                Frame {
                    x: screen_rect.origin.x as i32,
                    y: screen_rect.origin.y as i32,
                    width: screen_rect.size.width as i32,
                    height: screen_rect.size.height as i32,
                },
            );
        }
    }

    (0, Frame {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    })
}

fn save_window_layouts() {
    println!("  \"windows\": [");

    for cg_window in get_cg_window_list() {
        if let Some(window_bounds) = cg_window.get("kCGWindowBounds") {
            let window_bounds =
                serde_json::from_value(window_bounds.clone()).expect("Failed to parse window bounds");
            let window_bounds: Frame = Frame {
                x: window_bounds["X"].as_i64().unwrap() as i32,
                y: window_bounds["Y"].as_i64().unwrap() as i32,
                width: window_bounds["Width"].as_i64().unwrap() as i32,
                height: window_bounds["Height"].as_i64().unwrap() as i32,
            };
            let (display_id, screen_bounds) = screen_origin_for_window(window_bounds);

            if display_id != 0 {
                if let Some(cg_owner_name) = cg_window.get("kCGWindowOwnerName").and_then(|v| v.as_str()) {
                    if let Some(cg_window_name) = cg_window.get("kCGWindowName").and_then(|v| v.as_str()) {
                        let window_bounds = Frame {
                            x: window_bounds.x - screen_bounds.x,
                            y: window_bounds.y - screen_bounds.y,
                            width: window_bounds.width,
                            height: window_bounds.height,
                        };

                        let window_layout = WindowLayout {
                            kcg_window_owner_name: cg_owner_name.to_string(),
                            kcg_window_name: cg_window_name.to_string(),
                            display_id,
                            kcg_window_bounds: window_bounds,
                        };

                        println!("    {}", serde_json::to_string_pretty(&window_layout).unwrap());
                    }
                }
            }
        }
    }

    println!("  ]");
}

fn read_layout_config() -> (std::collections::HashMap<u32, ScreenLayout>, Vec<WindowLayout>) {
    let file_content =
        fs::read_to_string("/Users/jafischer/.layout.json").expect("Error reading layout config");
    let json_obj: Value = serde_json::from_str(&file_content).expect("Error parsing layout config");

    let mut screen_layouts = std::collections::HashMap::new();
    let mut desired_window_layouts = Vec::new();

    if let Some(screens) = json_obj["screens"].as_array() {
        for screen in screens {
            let screen_layout: ScreenLayout = serde_json::from_value(screen.clone()).expect("Error parsing screen layout");
            screen_layouts.insert(screen_layout.display_id, screen_layout);
        }
    }

    if let Some(windows) = json_obj["windows"].as_array() {
        for window in windows {
            let window_layout: WindowLayout = serde_json::from_value(window.clone()).expect("Error parsing window layout");
            desired_window_layouts.push(window_layout);
        }
    }

    (screen_layouts, desired_window_layouts)
}

fn is_close(x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    (x1 - x2).abs() < 4.0 && (y1 - y2).abs() < 4.0
}

fn convert_relative_coords_to_absolute(
    window_pos: Frame,
    saved_display_id: u32,
    screen_layouts: &std::collections::HashMap<u32, ScreenLayout>,
) -> Frame {
    let main_screen_rect = quartz::nsscreen::NSScreen::screens()[0].frame();
    let target_screen = screen_layouts.get(&saved_display_id);

    let mut screen_rect: Frame;

    if let Some(target_screen) = target_screen {
        screen_rect = target_screen.frame.clone();
    } else {
        screen_rect = find_closest_screen(&screen_layouts, &saved_display_id).unwrap();
    }

    screen_rect.y = main_screen_rect.origin.y + main_screen_rect.size.height
        - screen_rect.y
        - screen_rect.height;

    Frame {
        x: window_pos.x + screen_rect.x,
        y: window_pos.y + screen_rect.y,
        width: window_pos.width,
        height: window_pos.height,
    }
}

fn find_closest_screen(
    screen_layouts: &std::collections::HashMap<u32, ScreenLayout>,
    saved_display_id: &u32,
) -> Result<Frame, Box<dyn std::error::Error>> {
    let saved_screen_rect = &screen_layouts[saved_display_id].frame;
    let mut closest_screen_rect = Frame {
        x: -9999,
        y: -9999,
        width: 1,
        height: 1,
    };

    for screen in quartz::nsscreen::NSScreen::screens() {
        let screen_rect = screen.frame();
        if screen_rect == saved_screen_rect.clone() {
            return Ok(screen_rect);
        } else if screen_rect.size == saved_screen_rect.size {
            let current_closest = ((closest_screen_rect.x - saved_screen_rect.x).abs()
                + (closest_screen_rect.y - saved_screen_rect.y).abs() as f32)
                .hypot(2.0);
            let distance_to_this_screen = ((screen_rect.origin.x - saved_screen_rect.x).abs()
                + (screen_rect.origin.y - saved_screen_rect.y).abs() as f32)
                .hypot(2.0);
            if distance_to_this_screen < current_closest {
                closest_screen_rect = screen_rect;
            }
        }
    }

    if closest_screen_rect.x == -9999 {
        return Err("Failed to find target screen".into());
    }

    Ok(closest_screen_rect)
}

fn find_ax_ui_window(
    cg_window: &std::collections::HashMap<String, serde_json::Value>,
    use_regex: bool,
    regex: &regex::Regex,
    window_name: &str,
) -> Option<()> {
    if let Some(window_pid) = cg_window.get("kCGWindowOwnerPID").and_then(|v| v.as_i64()) {
        let axui_app = quartz::axui_element_create_application(window_pid as i32);
        if let Ok(axui_app) = axui_app {
            let mut value: std::option::Option<quartz::axui_element::AXUIElement> = None;
            if let Ok(result) =
                quartz::axui_element_copy_attribute_value(axui_app, kAXWindowsAttribute, &mut value)
            {
                if result == quartz::ax_error::AXError::Success {
                    if let Some(axui_window_list) = value {
                        for axui_window in axui_window_list.iter() {
                            if let Ok(value2) =
                                quartz::axui_element_copy_attribute_value(axui_window, kAXTitleAttribute, &mut value)
                            {
                                if value2 == quartz::ax_error::AXError::Success {
                                    if let Some(window_title) = value.and_then(|v| v.to_string()) {
                                        debug_log(
                                            &format!(
                                                "    findAXUIWindow (retry == {}): checking windowTitle [{}]",
                                                0,
                                                window_title
                                            ),
                                            true,
                                        );

                                        if (window_title == window_name
                                            || window_title == window_name.to_string() + " - Chrome"
                                            || 0 == 1)
                                            && find_matching_position_size(
                                                cg_window,
                                                axui_window,
                                            )?
                                        {
                                            return Some(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn find_matching_position_size(
    cg_window: &std::collections::HashMap<String, serde_json::Value>,
    axui_window: quartz::axui_element::AXUIElement,
) -> Result<bool, quartz::ax_error::AXError> {
    let cg_window_bounds = serde_json::from_value(cg_window["kCGWindowBounds"].clone()).unwrap();
    let axui_pos = quartz::axui_element_copy_attribute_value(axui_window, kAXPositionAttribute)?;
    let axui_size = quartz::axui_element_copy_attribute_value(axui_window, kAXSizeAttribute)?;

    let mut axui_pos_value: std::option::Option<quartz::axui_element::AXValue> = None;
    let mut axui_size_value: std::option::Option<quartz::axui_element::AXValue> = None;

    if axui_pos == quartz::ax_error::AXError::Success {
        axui_pos_value = Some(axui_pos.unwrap());
    }

    if axui_size == quartz::ax_error::AXError::Success {
        axui_size_value = Some(axui_size.unwrap());
    }

    let mut axui_pos_value = axui_pos_value.unwrap();
    let mut axui_size_value = axui_size_value.unwrap();

    let mut axui_pos: CGPoint = Default::default();
    let mut axui_size: CGSize = Default::default();

    quartz::ax_value_get_value(&mut axui_pos_value, quartz::ax_value_type::AXValueTypeCGPoint, &mut axui_pos);
    quartz::ax_value_get_value(&mut axui_size_value, quartz::ax_value_type::AXValueTypeCGSize, &mut axui_size);

    let current_pos: CGPoint = Default::default();
    let current_size: CGSize = Default::default();

    let mut value: std::option::Option<quartz::axui_element::AXValue> = None;
    let mut result =
        quartz::axui_element_copy_attribute_value(axui_window, kAXPositionAttribute, &mut value);
    let mut current_pos_value: std::option::Option<quartz::axui_element::AXValue> = None;

    if result == quartz::ax_error::AXError::Success {
        current_pos_value = value;
    }

    let mut value: std::option::Option<quartz::axui_element::AXValue> = None;
    let mut result = quartz::axui_element_copy_attribute_value(axui_window, kAXSizeAttribute, &mut value);
    let mut current_size_value: std::option::Option<quartz::axui_element::AXValue> = None;

    if result == quartz::ax_error::AXError::Success {
        current_size_value = value;
    }

    let mut value: std::option::Option<quartz::axui_element::AXValue> = None;
    let mut result =
        quartz::axui_element_copy_attribute_value(axui_window, kAXPositionAttribute, &mut value);
    if result == quartz::ax_error::AXError::Success {
        current_pos_value = value;
    }

    let mut value: std::option::Option<quartz::axui_element::AXValue> = None;
    let mut result = quartz::axui_element_copy_attribute_value(axui_window, kAXSizeAttribute, &mut value);
    if result == quartz::ax_error::AXError::Success {
        current_size_value = value;
    }

    quartz::ax_value_get_value(
        &mut current_pos_value.unwrap(),
        quartz::ax_value_type::AXValueTypeCGPoint,
        &mut current_pos,
    );

    quartz::ax_value_get_value(
        &mut current_size_value.unwrap(),
        quartz::ax_value_type::AXValueTypeCGSize,
        &mut current_size,
    );

    debug_log(
        &format!("    currentPos: {}, {}", current_pos.x, current_pos.y),
        true,
    );
    debug_log(
        &format!("    desiredPos: {}, {}", axui_pos.x, axui_pos.y),
        true,
    );
    debug_log(
        &format!("    current

 */
