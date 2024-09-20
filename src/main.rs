#[macro_use]
extern crate objc;

use accessibility_sys::{
    kAXErrorSuccess, kAXPositionAttribute, kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize,
    kAXWindowsAttribute, AXError, AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementRef,
    AXUIElementSetAttributeValue, AXValueCreate,
};
use anyhow::anyhow;
use clap::Parser;
use cocoa::appkit::NSScreen;
use cocoa_foundation::base::{id, nil};
use cocoa_foundation::foundation::{NSArray, NSFastEnumeration, NSString, NSUInteger};
use core_foundation::base::*;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::*;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_graphics::display;
use core_graphics::display::{CGDisplay, CGWindowID};
use core_graphics_types::geometry::{CGPoint, CGRect, CGSize};
use log::{debug, error, trace, LevelFilter};
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::config::{Appender, Root};
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::ffi::c_void;
use std::fs::File;
use std::io::BufReader;
use std::thread::sleep;
use std::time::Duration;

use args::Args;
use layout_types::{Layout, Rect, ScreenInfo, WindowInfo, MIN_HEIGHT, MIN_WIDTH};

use crate::args::Command;
use crate::dict_utils::{get_dict_from_dict, get_num_from_dict, get_string_from_dict};
use crate::idref::IdRef;
use crate::layout_types::MaybeRegex::Exact;
use crate::layout_types::{MatchingWindowInfo, WindowPos};

extern "C" {
    pub fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut CGWindowID) -> AXError;
}

mod args;
mod dict_utils;
mod idref;
mod layout_types;

/// See args.rs for command line arguments.
fn main() {
    let args = Args::parse();

    initialize_logging(args.log_level);

    match args.command() {
        Command::Restore => restore_layout(args.path),
        Command::Save => save_layout(),
    }
}

/// Initialize the logging. Logging goes to stderr, so as not to interfere with the layout output when
/// save is specified.
fn initialize_logging(log_level: LevelFilter) {
    let log_appender = Appender::builder()
        .build("stdout".to_string(), Box::new(ConsoleAppender::builder().target(Target::Stderr).build()));
    let log_root = Root::builder().appender("stdout".to_string()).build(log_level);
    let log_config = log4rs::config::Config::builder().appender(log_appender).build(log_root);

    // Keeping the redundant prefix here to make it clear just what "config" is being initialized here.
    log4rs::config::init_config(log_config.unwrap()).unwrap();
}

/// Enumerate the current screens and windows, and dump to stdout.
fn save_layout() {
    let screens = get_screens();
    let layout = get_current_layout(&screens);

    println!("{}", serde_yaml::to_string(&layout).unwrap());
}

/// Loads the desired layout, and moves all matching windows to their desired position.
fn restore_layout(path: String) {
    let desired_layout = load_layout_file(path);

    // I have noticed that when moving from a small monitor to a large (e.g. 4K) one, the window gets
    // moved but does not get resized properly. So rather than introduce complex logic I'm just going to
    // try looping through all windows twice.
    let screens = get_screens();

    for _ in 0..2 {
        let current_layout = get_current_layout(&screens);

        for window_info in current_layout.windows {
            // See if there's a match for the Owner + Window names in the desired layout.
            //
            // Note: Vec::find() is O(n) and thus the entire loop is basically O(n^2) but whatevs.
            // We're talking dozens, not millions.
            if let Some(desired_window_info) = desired_layout.windows.iter().find(|d| d.matches(&window_info)) {
                debug!(
                    "Found match for window {:?}/{:?}: {:?}/{:?}",
                    window_info.owner_name, window_info.name, desired_window_info.owner_name, desired_window_info.name,
                );
                debug!(
                    "Current bounds: {}/{:?}, desired: {}/{:?}",
                    window_info.screen_num, window_info.pos, desired_window_info.screen_num, desired_window_info.pos
                );

                for matching_window in &window_info.matching_windows {
                    // Now compare the current position with the desired position to see if we need to move the window.
                    let matching_pos = WindowPos::Pos(matching_window.bounds.clone());
                    // If the screen index is higher than the current number of screens, just take the right-most.
                    let screen_index = (matching_window.screen_num - 1).min(screens.len() - 1);
                    let screen = screens.get(screen_index).unwrap();
                    let current_absolute_bounds = matching_pos.to_absolute(&screen);
                    let desired_absolute_bounds = desired_window_info.pos.to_absolute(&screen);

                    // Rather than checking for equality, check for "within a couple of pixels" because I've found
                    // that after moving, the window coords don't always exactly match what I sent.
                    if !current_absolute_bounds.is_close(&desired_absolute_bounds) {
                        debug!(
                            "Needs to be moved: {:?}/{:?}: {:?}->{:?}",
                            window_info.owner_name, window_info.name, current_absolute_bounds, desired_absolute_bounds
                        );

                        move_window(&window_info, matching_window, desired_absolute_bounds);
                    } else {
                        trace!("No need to move {:?}/{:?}", window_info.owner_name, window_info.name);
                    }
                }
            } else {
                trace!("No match for {:?}/{:?}", window_info.owner_name, window_info.name);
            }
        }

        sleep(Duration::from_millis(500));
    }
}

/// Loads the user's layout file.
fn load_layout_file(path: String) -> Layout {
    // Don't need the portable "home" crate, because this is MacOs-only.
    let home: String = env!("HOME").into();

    let path: String = Regex::new("^~").unwrap().replace(&path, home).into();

    let file = File::open(&path).expect(&format!("Failed to open {}", path));
    let reader = BufReader::new(file);

    let desired_layout: Layout =
        serde_yaml::from_reader(reader).expect(&format!("Failed to parse layout file {}", path));
    desired_layout
}

/// Returns a list of the current screens, ordered from left to right.
fn get_screens() -> Vec<ScreenInfo> {
    let mut screens = vec![];

    unsafe {
        let ns_screens = NSScreen::screens(nil);
        let primary_screen = NSScreen::mainScreen(nil);
        let primary_frame = primary_screen.frame();
        let screen_num_key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));

        for screen in ns_screens.iter() {
            let frame = screen.frame();

            // I'm going to convert all NSScreen coords (which are bizarrely reversed in the Y axis) to the
            // same orientation as window coords.
            let fixed_y = primary_frame.size.height - frame.size.height - frame.origin.y;

            let device_desc = screen.deviceDescription();
            let screen_id: id = msg_send![device_desc, objectForKey:*screen_num_key];
            let screen_id: NSUInteger = msg_send![screen_id, unsignedIntegerValue];
            screens.push(ScreenInfo {
                screen_id: screen_id as u32,
                frame: Rect {
                    x: frame.origin.x as i32,
                    y: fixed_y as i32,
                    w: frame.size.width as i32,
                    h: frame.size.height as i32,
                },
            });
        }
    }

    if screens.is_empty() {
        panic!("Unable to enumerate screens.\nPlease add layout to the 'Screen & System Audio Recording' apps\nin System Preferences -> Privacy & Security")
    }

    // Sort the screens left-to-right
    screens.sort_by(|screen1, screen2| screen1.frame.x.cmp(&screen2.frame.x));

    screens
}

/// Returns the current window and screen layout.
fn get_current_layout(screens: &Vec<ScreenInfo>) -> Layout {
    let windows = get_windows(screens);

    Layout { windows }
}

/// Returns a list of window owners that we wish to ignore.
fn owners_to_ignore() -> HashSet<String> {
    HashSet::from(["Control Center".into(), "Dock".into(), "Window Server".into()])
}

/// Returns a list of `WindowInfo` for the current desktop windows.
fn get_windows(screens: &Vec<ScreenInfo>) -> Vec<WindowInfo> {
    // Use a map of maps here to get a nicely ordered list. Ordered by owner name and then window name.
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

        let owner_name = get_string_from_dict(&window_dict, "kCGWindowOwnerName");
        let window_name = get_string_from_dict(&window_dict, "kCGWindowName");

        // Skip some obvious windows: empty names, or names in the ignore list.
        if owner_name.is_empty() || window_name.is_empty() || owners_to_ignore.contains(&owner_name) {
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

        let bounds: Rect = CGRect::from_dict_representation(&bounds).unwrap().into();
        window_info.pos = WindowPos::Pos(bounds.clone());

        // Skip windows below a certain size
        if bounds.w <= MIN_WIDTH && bounds.h <= MIN_HEIGHT {
            continue;
        }

        // `bounds` is an absolute position, so convert to a position relative to the containing screen.
        let (screen_num, adjusted_bounds) = WindowPos::Pos(bounds).to_relative(&screens);
        window_info.screen_num = screen_num;
        window_info.pos = WindowPos::Pos(adjusted_bounds.clone());

        if !window_map.contains_key(&owner_name) {
            window_map.insert(owner_name.clone(), BTreeMap::new());
        }
        let owner_map = window_map.get_mut(&owner_name).unwrap();

        // There can be multiple windows with the same owner name and window name (e.g. multiple projects opened
        // in RustRover and they each have a "Find" window open). This is why window_info.ids is a Vector.
        if let Some(matching_window_info) = owner_map.get(&window_name) {
            window_info.matching_windows = matching_window_info.matching_windows.clone();
        } else {
            window_info.matching_windows = Vec::new();
        }
        window_info.matching_windows.push(MatchingWindowInfo {
            process_id,
            window_id,
            screen_num,
            bounds: adjusted_bounds.clone(),
        });
        owner_map.insert(window_name, window_info);
    }

    // Now flatten the map of maps into a vector that is sorted by Owner Name and then Name.
    let windows: Vec<WindowInfo> = window_map
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
        .collect();

    if windows.is_empty() {
        panic!("Unable to enumerate windows.\nPlease add layout to the 'Screen & System Audio Recording' apps\nin System Preferences -> Privacy & Security")
    }

    windows
}

/// Moves the specified window to the desired location.
fn move_window(window_info: &WindowInfo, matching_window: &MatchingWindowInfo, desired_absolute_bounds: Rect) {
    if let Ok(axwindow) = get_axwindow(matching_window.process_id, matching_window.window_id) {
        trace!(
            "Found axwindow for {:?}/{:?}/{:?}/{:?}",
            window_info.owner_name,
            window_info.name,
            matching_window.process_id,
            matching_window.window_id
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
            error!("AXUIElementSetAttributeValue(kAXPositionAttribute) failed: {:?}", result);
            return;
        }

        let mut cg_size: CGSize = desired_absolute_bounds.size();
        let size = unsafe { AXValueCreate(kAXValueTypeCGSize, &mut cg_size as *mut _ as *mut c_void) };

        let result = unsafe {
            AXUIElementSetAttributeValue(axwindow, CFString::new(kAXSizeAttribute).as_concrete_TypeRef(), size as _)
        };
        if result != kAXErrorSuccess {
            error!("AXUIElementSetAttributeValue(kAXSizeAttribute) failed: {:?}", result);
        }
    }
}

/// Given an Owner ID and Window ID from the `CGWindowList` API, returns the corresponding `AXUIElementRef` to
/// use with the Accessibility API.
/// <br>(We need this because, annoyingly, we enumerate desktop windows using the CGWindowList API,
/// and yet we have to use an entirely different API -- the Accessibility API -- to actually move the
/// windows).
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
            if unsafe { _AXUIElementGetWindow(ax_window as AXUIElementRef, &mut id) } != kAXErrorSuccess {
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
