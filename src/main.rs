use crate::idref::IdRef;
use clap::builder::TypedValueParser;
use clap::Parser;
use cocoa::appkit::NSScreen;
use cocoa_foundation::base::{id, nil};
use cocoa_foundation::foundation::{NSFastEnumeration, NSString, NSUInteger};
use core_foundation::base::*;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::*;
use core_foundation::string::*;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::string::{kCFStringEncodingUTF8, CFStringGetCStringPtr, CFStringRef};
use core_graphics::display;
use core_graphics::display::{CGDirectDisplayID, CGDisplay};
use core_graphics_types::geometry::CGRect;
use layout_types::{Layout, Rect, ScreenLayout, WindowLayout};
use log::error;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::config::{Appender, Root};
use objc::msg_send;
use std::collections::{BTreeMap, HashSet};
use std::ffi::{c_void, CStr};
use std::fs::File;
use std::io::BufReader;

mod idref;
mod layout_types;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Save the current layout (prints to stdout). If not specified, then the layout in ~/.layout.json will be loaded.
    #[arg(short, long)]
    save: bool,
}

const MIN_WIDTH: i32 = 64;
const MIN_HEIGHT: i32 = 64;

fn main() {
    let args = Args::parse();

    initialize_logging();

    if args.save {
        save_layout();
    } else {
        restore_layout();
    }
}

fn initialize_logging() {
    let log_appender = Appender::builder().build(
        "stdout".to_string(),
        Box::new(ConsoleAppender::builder().target(Target::Stderr).build()),
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

fn save_layout() {
    let layout = get_layout();

    println!("{}", serde_json::to_string_pretty(&layout).unwrap());
}

fn get_layout() -> Layout {
    let screens = get_screens();
    let windows = get_windows(&screens);
    let layout = Layout { screens, windows };
    layout
}

fn restore_layout() {
    let home = env!("HOME");
    let path = format!("{}/.layout.json", home);

    let file = File::open(&path).expect(&format!("Failed to open {}", path));
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `User`.
    let desired_layout: Layout =
        serde_json::from_reader(reader).expect(&format!("Failed to parse JSON file {}", path));

    let current_layout = get_layout();

    for window_layout in current_layout.windows {}
}

fn owners_to_ignore() -> HashSet<String> {
    HashSet::from([
        "Control Center".into(),
        "Dock".into(),
        "Window Server".into(),
    ])
}

fn get_windows(screens: &Vec<ScreenLayout>) -> Vec<WindowLayout> {
    // Use a map of maps here to get a nicely ordered list. Ordered by owner name and then name.
    let mut window_map: BTreeMap<String, BTreeMap<String, WindowLayout>> = BTreeMap::new();
    let owners_to_ignore = owners_to_ignore();

    let window_infos = CGDisplay::window_list_info(
        display::kCGWindowListExcludeDesktopElements | display::kCGWindowListOptionOnScreenOnly,
        None,
    );

    if window_infos.is_none() {
        error!("Failed to retrieve list of windows.");
        return Vec::new();
    }

    let window_infos = window_infos.unwrap();

    for window_info in window_infos.iter() {
        // window_info is a dictionary. Need to recast...
        let window_dict: CFDictionary<*const c_void, *const c_void> =
            unsafe { CFDictionary::wrap_under_get_rule(*window_info as CFDictionaryRef) };

        let mut window_layout = WindowLayout::default();

        // debug!("Window keys: {:?}", get_dict_keys(&window_dict));

        let owner_name = get_string_from_dict(&window_dict, "kCGWindowOwnerName");
        let name = get_string_from_dict(&window_dict, "kCGWindowName");

        // Skip some obvious windows: empty names, or names in the ignore list.
        if owner_name.is_empty() || name.is_empty() || owners_to_ignore.contains(&owner_name) {
            continue;
        }

        window_layout.owner_name = owner_name.clone();
        window_layout.name = name.clone();
        let bounds = match get_dict_from_dict(&window_dict, "kCGWindowBounds") {
            Some(value) => value,
            None => continue,
        };

        let bounds = CGRect::from_dict_representation(&bounds).unwrap();

        window_layout.bounds = bounds.into();

        // Skip windows below a certain size
        if window_layout.bounds.w <= MIN_WIDTH && window_layout.bounds.h <= MIN_HEIGHT {
            continue;
        }

        let (display_id, adjusted_bounds) = adjusted_bounds(&window_layout.bounds, &screens);
        window_layout.display_id = display_id;
        window_layout.bounds = adjusted_bounds;

        if !window_map.contains_key(&owner_name) {
            window_map.insert(owner_name.clone(), BTreeMap::new());
        }
        window_map
            .get_mut(&owner_name)
            .unwrap()
            .insert(name, window_layout);
    }

    // Now flatten the map of maps into a vector that is sorted by Owner Name and then Name.
    window_map
        .into_iter()
        .map(|(_, windows_by_owner)| windows_by_owner)
        .collect::<Vec<BTreeMap<String, WindowLayout>>>()
        .into_iter()
        .flat_map(|windows_by_owner| {
            windows_by_owner
                .into_iter()
                .map(|(_, v)| v)
                .collect::<Vec<WindowLayout>>()
        })
        .collect()
}

#[macro_use]
extern crate objc;

fn get_screens() -> Vec<ScreenLayout> {
    let mut screens = Vec::new();

    unsafe {
        let nsscreens = NSScreen::screens(nil);
        let screen_num_key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));

        for screen in nsscreens.iter() {
            let frame = screen.frame();
            let device_desc = screen.deviceDescription();
            let value: id = msg_send![device_desc, objectForKey:*screen_num_key];
            let value: NSUInteger = msg_send![value, unsignedIntegerValue];
            screens.push(ScreenLayout {
                id: value as u32,
                frame: Rect {
                    x: frame.origin.x as i32,
                    y: frame.origin.y as i32,
                    w: frame.size.width as i32,
                    h: frame.size.height as i32,
                },
            });
        }
    }
    screens
}

#[allow(unused)]
fn get_dict_keys(dict: &CFDictionary) -> Vec<String> {
    let (keys, values) = dict.get_keys_and_values();
    let length = dict.len();

    let mut results = Vec::new();
    for i in 0..length {
        results.push(string_ref_to_string(keys[i] as CFStringRef));
    }
    results
}

fn get_string_from_dict(dict: &CFDictionary, key: &str) -> String {
    let key = CFString::new(key);
    match dict.find(key.to_void()) {
        Some(value) => unsafe { CFString::wrap_under_get_rule(*value as CFStringRef) }.to_string(),
        None => return String::new(),
    }
}

fn get_i32_from_dict(dict: &CFDictionary, key: &str) -> i32 {
    let key = CFString::new(key);
    let value = match dict.find(key.to_void()) {
        Some(n) => n,
        None => return 0,
    };

    let mut value_i32 = 0_i32;
    let out_value: *mut i32 = &mut value_i32;
    unsafe { CFNumberGetValue(*value as CFNumberRef, kCFNumberSInt32Type, out_value.cast()) };
    value_i32
}

fn get_dict_from_dict(dict: &CFDictionary, key: &str) -> Option<CFDictionary> {
    let key = CFString::new(key);
    let value = match dict.find(key.to_void()) {
        Some(n) => n,
        None => return None,
    };
    Some(unsafe { CFDictionary::wrap_under_get_rule(*value as CFDictionaryRef) })
}

fn adjusted_bounds(window_bounds: &Rect, screens: &Vec<ScreenLayout>) -> (CGDirectDisplayID, Rect) {
    for screen in screens {
        if screen.frame.contains(&window_bounds) {
            return (
                screen.id,
                Rect {
                    x: window_bounds.x - screen.frame.x,
                    y: window_bounds.y - screen.frame.y,
                    w: window_bounds.w,
                    h: window_bounds.h,
                },
            );
        }
    }

    (
        1,
        Rect {
            x: 0,
            y: 0,
            w: window_bounds.w,
            h: window_bounds.h,
        },
    )
}

// From https://github.com/hahnlee/canter
#[allow(unused)]
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
