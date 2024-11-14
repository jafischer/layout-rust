use std::fmt::Display;

use core_graphics_types::base::CGFloat;
use core_graphics_types::geometry::{CGPoint, CGRect, CGSize};
use log::debug;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::layout_types::MaybeRegex::{Exact, RE};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layout {
    pub windows: Vec<WindowInfo>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreenInfo {
    pub frame: Rect,
    pub screen_id: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowInfo {
    pub owner_name: MaybeRegex,
    pub name: MaybeRegex,
    // For our purposes we want to move every window that has the same Owner Name + Name to the same position. So
    // we need to keep track of the process_id, window_id and position of all matching windows.
    // But we don't need to store this info when saving the current layout; hence
    // the `skip_serializing, skip_deserializing`.
    #[serde(skip_serializing, skip_deserializing)]
    pub matching_windows: Vec<MatchingWindowInfo>,
    pub screen_num: usize,
    pub pos: WindowPos,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum WindowPos {
    #[default]
    Maxed,
    Pos(Rect),
    Left(f32),
    Right(f32),
    Top(f32),
    Bottom(f32),
}

impl WindowPos {
    pub fn to_absolute(&self, screen: &ScreenInfo) -> Rect {
        match self {
            WindowPos::Maxed => Rect {
                x: screen.frame.x,
                y: screen.frame.y,
                w: screen.frame.w,
                h: screen.frame.h,
            },
            WindowPos::Pos(rect) => {
                let x = if rect.x < 0 { screen.frame.w + rect.x } else { rect.x };
                let y = if rect.y < 0 { screen.frame.h + rect.y } else { rect.y };
                Rect {
                    x: x + screen.frame.x,
                    y: y + screen.frame.y,
                    w: rect.w,
                    h: rect.h,
                }
            }
            WindowPos::Left(fraction) => Rect {
                x: screen.frame.x,
                y: screen.frame.y,
                w: (screen.frame.w as f32 * fraction) as i32,
                h: screen.frame.h,
            },
            WindowPos::Right(fraction) => {
                let w = (screen.frame.w as f32 * fraction) as i32;
                Rect {
                    x: screen.frame.x + screen.frame.w - w,
                    y: screen.frame.y,
                    w,
                    h: screen.frame.h,
                }
            }
            WindowPos::Top(fraction) => Rect {
                x: screen.frame.x,
                y: screen.frame.y,
                w: screen.frame.w,
                h: (screen.frame.h as f32 * fraction) as i32,
            },
            WindowPos::Bottom(fraction) => {
                let h = (screen.frame.h as f32 * fraction) as i32;
                Rect {
                    x: screen.frame.x,
                    y: screen.frame.y + screen.frame.h - h,
                    w: screen.frame.w,
                    h,
                }
            }
        }
    }

    pub fn to_relative(&self, screens: &Vec<ScreenInfo>) -> (usize, Rect) {
        match self {
            WindowPos::Pos(rect) => {
                for (index, screen) in screens.iter().enumerate() {
                    if screen.frame.contains_origin(rect) {
                        return (
                            index + 1,
                            Rect {
                                x: rect.x - screen.frame.x,
                                y: rect.y - screen.frame.y,
                                w: rect.w,
                                h: rect.h,
                            },
                        );
                    }
                }

                debug!("WindowPos::to_relative(): no screen found for abs pos ({}, {})", rect.x, rect.y);

                (
                    1, // Default to the first screen.
                    Rect {
                        x: 0,
                        y: 0,
                        w: rect.w,
                        h: rect.h,
                    },
                )
            }
            _ => (0, Rect::default()),
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct MatchingWindowInfo {
    pub process_id: i32,
    pub window_id: u32,
    // We also store a copy of the window position here, because in the "save" case we'll just save a single
    // position to the output file, but in the "restore" case we need to know the position of each window with
    // the same owner & window names.
    pub screen_num: usize,
    pub bounds: Rect,
}

impl WindowInfo {
    pub fn matches(&self, other: &Self) -> bool {
        (self.owner_name.matches(&other.owner_name.to_string()) && self.name.matches(&other.name.to_string()))
            || (other.owner_name.matches(&self.owner_name.to_string()) && other.name.matches(&self.name.to_string()))
    }
}

#[derive(Debug, Clone)]
pub enum MaybeRegex {
    Exact(String),
    RE(Regex),
}

impl MaybeRegex {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Exact(s) => s.eq(value),
            RE(r) => r.is_match(value),
        }
    }
}

impl Display for MaybeRegex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Exact(value) => value.clone(),
            RE(value) => value.as_str().to_string(),
        };
        write!(f, "{}", str)
    }
}

impl serde::Serialize for MaybeRegex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Exact(value) => serializer.serialize_str(value),
            RE(value) => serializer.serialize_str(value.as_str()),
        }
    }
}

impl<'de> serde::Deserialize<'de> for MaybeRegex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text_val = String::deserialize(deserializer)?;

        Ok(match Regex::new(&text_val) {
            Ok(value) => RE(value),
            Err(_) => Exact(text_val),
        })
    }
}

impl Default for MaybeRegex {
    fn default() -> Self {
        Exact("".into())
    }
}

impl Rect {
    // I only care if the origin is inside the rect, not the whole rect being contained.
    pub fn contains_origin(&self, p0: &Rect) -> bool {
        p0.x >= self.x && p0.x < self.x + self.w && p0.y >= self.y && p0.y < self.y + self.h
    }

    // Rather than checking for equality, check for "within a couple of pixels" because I've found
    // that after moving, the window coords don't always exactly match what I sent.
    pub fn is_close(&self, other: &Rect) -> bool {
        (self.x - other.x).abs() < 4
            && (self.y - other.y).abs() < 4
            && (self.w - other.w).abs() < 4
            && (self.h - other.h).abs() < 4
    }

    pub fn origin(&self) -> CGPoint {
        CGPoint {
            x: self.x as CGFloat,
            y: self.y as CGFloat,
        }
    }

    pub fn size(&self) -> CGSize {
        CGSize {
            width: self.w as CGFloat,
            height: self.h as CGFloat,
        }
    }
}

// This is silly, I know, but I'm implementing custom serialization just so that the bounds can be printed on one line
// instead of 6.
// I.e., this:
// bounds: "0,0,32,32"
// instead of this:
// bounds: [
//   0,
//   0,
//   32,
//   32
// ]
impl serde::Serialize for Rect {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let output = format!("{},{},{},{}", self.x, self.y, self.w, self.h);
        serializer.serialize_str(&output)
    }
}

impl<'de> serde::Deserialize<'de> for Rect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text_val = String::deserialize(deserializer)?;
        let coords: Vec<_> = text_val.split(",").collect();
        Ok(Rect {
            x: coords[0].parse().unwrap(),
            y: coords[1].parse().unwrap(),
            w: coords[2].parse().unwrap(),
            h: coords[3].parse().unwrap(),
        })
    }
}

impl Into<CGRect> for Rect {
    fn into(self) -> CGRect {
        CGRect {
            origin: CGPoint {
                x: self.x.into(),
                y: self.y.into(),
            },
            size: CGSize {
                width: self.w.into(),
                height: self.h.into(),
            },
        }
    }
}

impl From<CGRect> for Rect {
    fn from(value: CGRect) -> Self {
        Rect {
            x: value.origin.x as i32,
            y: value.origin.y as i32,
            w: value.size.width as i32,
            h: value.size.height as i32,
        }
    }
}

pub const MIN_WIDTH: i32 = 64;
pub const MIN_HEIGHT: i32 = 64;
