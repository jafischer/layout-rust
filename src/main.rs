use crate::idref::IdRef;
use crate::layout_types::MaybeRegex::Exact;
use crate::layout_types::WindowIds;
use accessibility_sys::{
    kAXErrorSuccess, kAXPositionAttribute, kAXSizeAttribute, kAXValueTypeCGPoint,
    kAXValueTypeCGSize, kAXWindowsAttribute, AXError, AXUIElementCopyAttributeValue,
    AXUIElementCreateApplication, AXUIElementRef, AXUIElementSetAttributeValue, AXValueCreate,
};
use anyhow::anyhow;
use clap::Parser;
use cocoa::appkit::NSScreen;
use cocoa_foundation::base::{id, nil};
use cocoa_foundation::foundation::{NSArray, NSFastEnumeration, NSString, NSUInteger};
use core_foundation::base::*;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::*;
use core_foundation::string::*;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::string::CFStringRef;
use core_graphics::display;
use core_graphics::display::{CGDirectDisplayID, CGDisplay, CGWindowID};
use core_graphics_types::geometry::{CGPoint, CGRect, CGSize};
use layout_types::{Layout, Rect, ScreenInfo, WindowInfo};
use log::{debug, error, trace};
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::config::{Appender, Root};
use objc::msg_send;
use std::collections::{BTreeMap, HashSet};
use std::ffi::c_void;
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

    /// Enable debug logging.
    #[arg(short, long)]
    debug: bool,
}

const MIN_WIDTH: i32 = 64;
const MIN_HEIGHT: i32 = 64;

fn main() {
    let args = Args::parse();

    initialize_logging(args.debug);

    if args.save {
        save_layout();
    } else {
        restore_layout();
    }
}

fn initialize_logging(debug: bool) {
    let log_appender = Appender::builder().build(
        "stdout".to_string(),
        Box::new(ConsoleAppender::builder().target(Target::Stderr).build()),
    );
    let log_root = Root::builder()
        .appender("stdout".to_string())
        .build(if debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        });
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

fn restore_layout() {
    let home = env!("HOME");
    let path = format!("{}/.layout.json", home);

    let file = File::open(&path).expect(&format!("Failed to open {}", path));
    let reader = BufReader::new(file);

    let desired_layout: Layout =
        serde_json::from_reader(reader).expect(&format!("Failed to parse JSON file {}", path));

    let current_layout = get_layout();

    for window_info in current_layout.windows {
        // This is O(n) and thus the entire loop is basically O(n^2) but whatevs.
        // We're talking dozens, not millions.
        if let Some(desired_window_info) = desired_layout
            .windows
            .iter()
            .find(|d| d.matches(&window_info))
        {
            trace!(
                "Found match for window {:?}/{:?}: {:?}/{:?}",
                window_info.owner_name,
                window_info.name,
                desired_window_info.owner_name,
                desired_window_info.name,
            );
            trace!(
                "Current bounds: {:?}, desired: {:?}",
                window_info.bounds,
                desired_window_info.bounds
            );

            let current_absolute_bounds = absolute_bounds(
                &window_info.bounds,
                window_info.screen_num,
                &current_layout.screens,
            );
            let desired_absolute_bounds = absolute_bounds(
                &desired_window_info.bounds,
                desired_window_info.screen_num,
                &current_layout.screens,
            );

            // Rather than checking for equality, check for "within a couple of pixels" because I've found
            // that after moving, the window coords don't always exactly match what I sent.
            if !current_absolute_bounds.is_close(&desired_absolute_bounds) {
                debug!(
                    "Needs to be moved: {:?}/{:?}: {:?}->{:?}",
                    window_info.owner_name,
                    window_info.name,
                    current_absolute_bounds,
                    desired_absolute_bounds
                );

                for ids in window_info.ids {
                    if let Ok(axwindow) = get_axwindow(ids.process_id, ids.window_id) {
                        debug!(
                            "Found axwindow for {:?}/{:?}/{:?}/{:?}",
                            window_info.owner_name, window_info.name, ids.process_id, ids.window_id
                        );

                        let mut cg_pos: CGPoint = desired_absolute_bounds.origin();
                        let position = unsafe {
                            AXValueCreate(
                                kAXValueTypeCGPoint,
                                // What a masterpiece of ugliness:
                                &mut cg_pos as *mut _ as *mut c_void,
                            )
                        };
                        let result = unsafe {
                            AXUIElementSetAttributeValue(
                                axwindow,
                                CFString::new(kAXPositionAttribute).as_concrete_TypeRef(),
                                position as _,
                            )
                        };
                        if result != kAXErrorSuccess {
                            error!(
                                "AXUIElementSetAttributeValue(kAXPositionAttribute) failed: {:?}",
                                result
                            );
                            continue;
                        }

                        let mut cg_size: CGSize = desired_absolute_bounds.size();
                        let size = unsafe {
                            AXValueCreate(kAXValueTypeCGSize, &mut cg_size as *mut _ as *mut c_void)
                        };

                        let result = unsafe {
                            AXUIElementSetAttributeValue(
                                axwindow,
                                CFString::new(kAXSizeAttribute).as_concrete_TypeRef(),
                                size as _,
                            )
                        };
                        if result != kAXErrorSuccess {
                            error!(
                                "AXUIElementSetAttributeValue(kAXSizeAttribute) failed: {:?}",
                                result
                            );
                        }
                    }
                }
            }
        }
    }
}

fn get_layout() -> Layout {
    let screens = get_screens();
    let windows = get_windows(&screens);
    Layout { screens, windows }
}

fn owners_to_ignore() -> HashSet<String> {
    HashSet::from([
        "Control Center".into(),
        "Dock".into(),
        "Window Server".into(),
    ])
}

fn get_windows(screens: &Vec<ScreenInfo>) -> Vec<WindowInfo> {
    // Use a map of maps here to get a nicely ordered list. Ordered by owner name and then name.
    let mut window_map: BTreeMap<String, BTreeMap<String, WindowInfo>> = BTreeMap::new();
    let owners_to_ignore = owners_to_ignore();

    let cg_window_infos = CGDisplay::window_list_info(
        display::kCGWindowListExcludeDesktopElements | display::kCGWindowListOptionOnScreenOnly,
        None,
    );

    if cg_window_infos.is_none() {
        error!("Failed to retrieve list of windows.");
        return Vec::new();
    }

    let cg_window_infos = cg_window_infos.unwrap();

    for cg_window_info in cg_window_infos.iter() {
        // window_info is a dictionary. Need to recast...
        let window_dict: CFDictionary<*const c_void, *const c_void> =
            unsafe { CFDictionary::wrap_under_get_rule(*cg_window_info as CFDictionaryRef) };

        let mut window_info = WindowInfo::default();

        // debug!("Window keys: {:?}", get_dict_keys(&window_dict));
        // Window keys:
        // ["kCGWindowLayer", "kCGWindowAlpha", "kCGWindowMemoryUsage", "kCGWindowIsOnscreen",
        // "kCGWindowSharingState", "kCGWindowOwnerPID", "kCGWindowNumber", "kCGWindowOwnerName",
        // "kCGWindowStoreType", "kCGWindowBounds", "kCGWindowName"]

        let owner_name = get_string_from_dict(&window_dict, "kCGWindowOwnerName");
        let window_name = get_string_from_dict(&window_dict, "kCGWindowName");

        // Skip some obvious windows: empty names, or names in the ignore list.
        if owner_name.is_empty() || window_name.is_empty() || owners_to_ignore.contains(&owner_name)
        {
            continue;
        }

        window_info.owner_name = Exact(owner_name.clone());
        window_info.name = Exact(window_name.clone());
        let bounds = match get_dict_from_dict(&window_dict, "kCGWindowBounds") {
            Some(value) => value,
            None => continue,
        };

        let process_id: i32 = get_num_from_dict(&window_dict, "kCGWindowOwnerPID");
        let window_id: u32 = get_num_from_dict(&window_dict, "kCGWindowNumber");

        let bounds = CGRect::from_dict_representation(&bounds).unwrap();

        window_info.bounds = bounds.into();

        // Skip windows below a certain size
        if window_info.bounds.w <= MIN_WIDTH && window_info.bounds.h <= MIN_HEIGHT {
            continue;
        }

        let (display_id, adjusted_bounds) = relative_bounds(&window_info.bounds, &screens);
        window_info.screen_num = display_id as usize;
        window_info.bounds = adjusted_bounds;

        if !window_map.contains_key(&owner_name) {
            window_map.insert(owner_name.clone(), BTreeMap::new());
        }
        let owner_map = window_map.get_mut(&owner_name).unwrap();

        if let Some(matching_window_info) = owner_map.get(&window_name) {
            window_info.ids = matching_window_info.ids.clone();
        } else {
            window_info.ids = Vec::new();
        }
        window_info.ids.push(WindowIds {
            process_id,
            window_id,
        });
        owner_map.insert(window_name, window_info);
    }

    // Now flatten the map of maps into a vector that is sorted by Owner Name and then Name.
    window_map
        .into_iter()
        .map(|(_, windows_by_owner)| windows_by_owner)
        .collect::<Vec<BTreeMap<String, WindowInfo>>>()
        .into_iter()
        .flat_map(|windows_by_owner| {
            windows_by_owner
                .into_iter()
                .map(|(_, v)| v)
                .collect::<Vec<WindowInfo>>()
        })
        .collect()
}

#[macro_use]
extern crate objc;

fn get_screens() -> Vec<ScreenInfo> {
    let mut screens = Vec::new();

    unsafe {
        let ns_screens = NSScreen::screens(nil);
        let primary_screen = NSScreen::mainScreen(nil);
        let primary_frame = primary_screen.frame();
        let screen_num_key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));

        for screen in ns_screens.iter() {
            let frame = screen.frame();

            // I'm going to convert all NSScreen coords (which are bizarrely reversed in the Y axis) to the
            // same as window coords.
            let fixed_y = primary_frame.size.height - frame.size.height - frame.origin.y;

            let device_desc = screen.deviceDescription();
            let value: id = msg_send![device_desc, objectForKey:*screen_num_key];
            let value: NSUInteger = msg_send![value, unsignedIntegerValue];
            screens.push(ScreenInfo {
                id: value as u32,
                frame: Rect {
                    x: frame.origin.x as i32,
                    y: fixed_y as i32,
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
        results.push(unsafe { CFString::wrap_under_get_rule(keys[i] as CFStringRef) }.to_string());
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

fn get_num_from_dict<T>(dict: &CFDictionary, key: &str) -> T
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

fn get_dict_from_dict(dict: &CFDictionary, key: &str) -> Option<CFDictionary> {
    let key = CFString::new(key);
    let value = match dict.find(key.to_void()) {
        Some(n) => n,
        None => return None,
    };
    Some(unsafe { CFDictionary::wrap_under_get_rule(*value as CFDictionaryRef) })
}

// Convert absolute window pos to one that's relative to the screen the window is on.
fn relative_bounds(window_bounds: &Rect, screens: &Vec<ScreenInfo>) -> (CGDirectDisplayID, Rect) {
    for screen in screens {
        if screen.frame.contains_origin(window_bounds) {
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

// Convert absolute window pos to one that's relative to the screen the window is on.
fn absolute_bounds(window_bounds: &Rect, screen_num: usize, screens: &Vec<ScreenInfo>) -> Rect {
    let screen: &ScreenInfo = screens
        .get(screen_num - 1)
        .unwrap_or(screens.get(0).unwrap());

    Rect {
        x: window_bounds.x + screen.frame.x,
        y: window_bounds.y + screen.frame.y,
        w: window_bounds.w,
        h: window_bounds.h,
    }
}

//
// One annoyance is that we have to enumerate all desktop windows using the CGWindowList API,
// and yet we have to use an entirely different API (the Accessibility API) to actually move the
// windows.
//
fn get_axwindow(owner_id: i32, window_id: u32) -> anyhow::Result<AXUIElementRef> {
    let ax_application = unsafe { AXUIElementCreateApplication(owner_id) };
    let mut windows_ref: CFTypeRef = std::ptr::null();

    if ax_application.is_null() {
        return Err(anyhow!("Failed to get application handle."));
    }

    if unsafe {
        AXUIElementCopyAttributeValue(
            ax_application,
            CFString::new(kAXWindowsAttribute).as_concrete_TypeRef(),
            &mut windows_ref as *mut CFTypeRef,
        )
    } != kAXErrorSuccess
    {
        unsafe {
            CFRelease(ax_application.cast());
        }

        return Err(anyhow!("Failed to get application attribute."));
    }

    if windows_ref.is_null() {
        unsafe {
            CFRelease(windows_ref.cast());
            CFRelease(ax_application.cast());
        }

        return Err(anyhow!("Failed to get application attribute."));
    }

    let windows_ref = windows_ref as id;

    let count = unsafe { NSArray::count(windows_ref) };

    for i in 0..count {
        let ax_window = unsafe { NSArray::objectAtIndex(windows_ref, i) };

        let ax_window_id = {
            let mut id: CGWindowID = 0;
            if unsafe { _AXUIElementGetWindow(ax_window as AXUIElementRef, &mut id) }
                != kAXErrorSuccess
            {
                continue;
            }
            id
        };

        if ax_window_id == window_id {
            unsafe {
                CFRetain(ax_window.cast());
                CFRelease(windows_ref.cast());
                CFRelease(ax_application.cast());
            }

            // return Ok(unsafe { AXUIElement::wrap_under_create_rule(ax_window as AXUIElementRef) });
            return Ok(ax_window as AXUIElementRef);
        }
    }

    unsafe {
        CFRelease(windows_ref.cast());
        CFRelease(ax_application.cast());
    }

    Err(anyhow!("Window not found"))
}

extern "C" {
    pub fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut CGWindowID) -> AXError;
}
