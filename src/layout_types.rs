use crate::layout_types::MaybeRegex::{Exact, RE};
use core_graphics_types::geometry::{CGPoint, CGRect, CGSize};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layout {
    pub screens: Vec<ScreenInfo>,
    pub windows: Vec<WindowInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenInfo {
    pub id: u32,
    pub frame: Rect,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowInfo {
    pub owner_name: MaybeRegex,
    pub name: MaybeRegex,
    // For our purposes we want to move every window that has the same Owner Name + Name to the same position. So
    // we need to keep track of the process_id + window_id of all matching windows.
    #[serde(skip_serializing, skip_deserializing)]
    pub ids: Vec<WindowIds>,
    pub screen_num: usize,
    pub bounds: Rect,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct WindowIds {
    pub process_id: i32,
    pub window_id: u32,
}

impl WindowInfo {
    pub fn matches(&self, other: &Self) -> bool {
        (self.owner_name.matches(&other.owner_name.to_string())
            && self.name.matches(&other.name.to_string()))
            || (other.owner_name.matches(&self.owner_name.to_string())
                && other.name.matches(&self.name.to_string()))
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

impl ToString for MaybeRegex {
    fn to_string(&self) -> String {
        match self {
            Exact(value) => value.clone(),
            RE(value) => value.as_str().to_string(),
        }
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
    pub(crate) fn contains_origin(&self, p0: &Rect) -> bool {
        p0.x >= self.x && p0.x < self.x + self.w && p0.y >= self.y && p0.y < self.y + self.h
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
