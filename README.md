This is command-line frontend for the [servo](https://servo.org) web browser. You can look at web pages in a terminal.

## LICENSE

This was created by Andi McClure. Assistance with translation was given by Fly and the Toki Pona discord. It is made available to you under the "MIT license".

	Copyright (c) 2024 Andi McClure

	Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

	The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

	THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

### CONTRIBUTING

This software is known to work on Ubuntu Linux 24.04 built with Rust 1.81. On my machine, I did three things to make it compile. One, I installed the libunwind-dev package in Ubuntu (for me this was required). Two and three, I put these in a `.cargo/config.toml`:

	[patch.'https://github.com/servo/servo']
	libservo = { path = '../other/servo/components/servo' }

	[env]
	RUSTC_BOOTSTRAP = "crown,script,style_tests"

…where "../other/servo" is a path on my local hard drive to a checkout of <https://github.com/servo/servo>. This is not required, but to me it is good because otherwise cargo will check out a large git repo in place and it will be annoying. The third thing, "env", is currently required by servo UNLESS you run `cargo +nightly`. The requirement for nightly/RUSTC_BOOTSTRAP will be fixed in a soon-upcoming revision of servo.
