# Layout

### A tool for restoring your carefully-arranged window layout on your MacBook

---
## Background

I have a very particular preference for how my application windows are laid out on my Macbook. 
Outlook goes _here_, all my IDE windows go _here_, and so on. 

MacOS tries to remember where things go, but let's face it, it doesn't do a terribly good job,
especially in the scenario where you are regularly disconnecting and reconnecting your external
monitors.

Hence this app. I wrote it because I once wrote a similar app for Windows, and I was
curious how hard it would be to write one for MacOS (spoiler alert: way harder).

## Rust

I originally [wrote this app in Swift](https://github.com/jafischer/layout) in 2019,
because Swift was relatively new at the time, and I wanted to learn it. The Swift version wasn't
pretty -- I didn't so much learn the language as wrestled something into working. But anyway, it
worked, and I used it happily for almost 5 years.

Then came MacOS Sonoma, which broke Layout for... reasons, that I now needed to figure out. By now I no
longer had any interest in Swift, but over the past year or so had become completely enamored with Rust...

It uses the [FFI bindings for Cocoa](https://crates.io/crates/cocoa), which... yuck. Your code ends up
sprinkled with a bunch of `unsafe` blocks; it would be great to have all that hidden away in a library
that exposed a nice idiomatic Rust interface. But... it works.

## Instructions

1. Arrange your application windows for maximum viewing pleasure.
2. Run `layout --save > ~/.layout.yaml`
3. Edit the file to
   - remove windows you don't care about
   - optionally use regular expressions to handle windows whose titles change depending on which file is
     opened, say (such as IDEs).<br>
4. Now run `layout` with no arguments to restore your layout.
5. Even better, install [Alfred](https://www.alfredapp.com/) and
   [set up a simple workflow](https://www.alfredapp.com/workflows/) that launches Layout when
   you press a shortcut key (I use `Ctrl+Cmd+L`).

See [sample-layout.yaml](./sample-layout.yaml) for an example.
