# Layout

### A tool for restoring your carefully-arranged window layout on your MacBook

---

## Background

I have a very particular preference for how my application windows are laid out
on my Macbook. Outlook goes _here_, all my IDE windows go _here_, and so on.

MacOS tries to remember where things go, but let's face it, it doesn't do a
terribly good job, especially in the scenario where you are regularly
disconnecting and reconnecting your external monitors.

Hence this app. I wrote it because I once wrote a similar app for Windows, and
I was curious how hard it would be to write one for MacOS (spoiler alert: way
harder).

## Rust

I originally [wrote this app in Swift](https://github.com/jafischer/layout) in 2019, because Swift was
relatively new at the time, and I wanted to learn it. The Swift version wasn't
pretty -- I didn't so much learn the language as wrestled something into
working. But anyway, it worked, and I used it happily for almost 5 years.

Then came MacOS Sonoma, which broke Layout for... reasons, that I now needed to
figure out. By now I no longer had any interest in Swift, but over the past
year or so had become completely enamored with Rust...

It uses the [FFI bindings for Cocoa](https://crates.io/crates/cocoa), which... yuck. Your code ends up
sprinkled with a bunch of `unsafe` blocks; it would be great to have all that
hidden away in a library that exposed a nice idiomatic Rust interface. But...
it works.

## Instructions

1. Arrange your application windows for maximum viewing pleasure.
2. Run `layout save > ~/.layout.yaml`
3. Edit the file to
   - remove windows you don't care about
   - optionally use regular expressions to handle windows whose titles change
     depending on which file is opened, say (such as IDEs).<br>
4. Now run `layout` with no arguments to restore your layout.
5. Even better, install [Alfred](https://www.alfredapp.com/) and
   [set up a simple workflow](https://www.alfredapp.com/workflows/) that launches Layout when
   you press a shortcut key (I use `Ctrl+Cmd+L`).

See [sample-layout.yaml](./sample-layout.yaml) for an example.

### Window Position Settings

#### Screen Number

The `screen_num` field used to refer to a specific screen ID, but now is simply
the left-to-right position of the screen. So 1 is the left-most screen,
and so on.

#### Position

Originally I just used a `Rect` in the layout file for specifying the exact
window position and size.

Recently I decided to introduce some new position options, and created
the [WindowPos](./src/layout_types.rs) enum for this purpose, with the
following values:

- `Maxed:` the window is maximized on the desired screen.
- `Pos(Rect):` the window is moved to the specified location and size.
  <br>Note: x and y can be negative, which then become relative to the right
  and bottom edges of the screen. This allows you to place certain windows
  (such as the Outlook Reminder popup, for example) in the bottom-right of
  the screen, regardless of the screen's size.
- `Left(f32), Right(f32), Top(f32), Bottom(f32):` the window will be moved to the left, right,
  top or bottom edge of the screen, accordingly. The float parameter
  represents a fraction of the screen width (for `Left` and
  `Right`) or height (for `Top` and `Bottom`).

I wasn't sure how to specify these new enum values in the yaml file, so I
simply ran `layout save` after making these changes to see how they're
serialized, and it turns out that you specify them like this:

```yaml
pos: !Pos -500,-300,400,143
```

```yaml
pos: !Maxed
```

```yaml
pos: !Left 0.5
```

etc.
