# Layout

### A tool for restoring your carefully-arranged window layout on your MacBook

---
## Background

Let's face it, MacBooks are not great at managing multiple monitors, at least in the
scenario where you are regularly unplugging them and plugging them back in.

When you come back from a meeting, and plug in your 2nd (and maybe 3rd) monitor,
your MacBook **tries** to remember where all the windows used to be, and put them all back.
But -- at least in my case, where I have a very particular preference for how my
applications' windows are arranged -- it fails miserably.

Hence this app. I wrote it because I once wrote a similar app for Windows, and I was
curious how hard it would be to write one for MacOS (spoiler alert: way harder).

## Rust

I originally [wrote this app in Swift](https://github.com/jafischer/layout) in 2019,
because Swift was relatively new at the time, and I wanted to learn it. The Swift version wasn't
pretty -- I didn't so much learn the language as wrestled something into working. But anyway, it
worked, and I used it happily for almost 5 years.

Then came MacOS Sonoma, which broke Layout for... reasons, that I now needed to figure out. By now I no
longer had any interest in Swift, but over the past year or so had become completely enamored with Rust...
So I decided to give it a try in Rust, since I saw that there was at least a set of FFI bindings for Cocoa.

Using FFI is unpleasant, as your code ends up sprinkled with a bunch of `unsafe` blocks; I'd much prefer that
to be hidden in a library that exposed a native Rust interface. But it works.

## Instructions

1. Arrange your application windows in the way you want to preserve.
2. Run `layout --save > ~/.layout.yaml`
3. Edit the file to
   - remove windows you don't care about
   - optionally use regular expressions to handle windows whose titles change depending on which file is
     opened, say (such as IDEs).<br>

See [sample-layout.yaml](./sample-layout.yaml) for an example.
