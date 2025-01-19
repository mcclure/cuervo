This is command-line frontend for the [Servo](https://servo.org) web browser. You can look at web pages in a terminal. This is an early prototype with known problems.

Cuervo can currently run in English, Spanish, Polish and Toki Pona.

## BUILDING

This software is known to work on Debian "Trixie" built with Rust 1.83.

This software requires certain C software packages to be present on the system to build. In my case, I had to install the Debian packages `libunwind-dev`, `libfontconfig1-dev`, and a C compiler.

### LICENSE

This was created by Andi McClure. Assistance with translation was given by Fly and the Toki Pona discord. It is made available to you under the "MIT license".

	Copyright (c) 2024-2025 Andi McClure

	Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

	The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

	THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

### ADDITIONAL BUILDING NOTES

This software uses a custom fork of Servo. Although the correct fork will be downloaded if you build with Cargo, if you intend to modify the program it will be much more convenient to use a local Servo checkout. I do this by making a git checkout of [this GitHub repo](https://github.com/mcclure/servo) (branch `cuervo_support_traversal_experiment`) alongside the Cuervo repo in the directory `other/servo`; then I add a patch in the `.cargo/config.toml`. Once I have done this, my config.toml looks like:

	[patch.'https://github.com/servo/servo']
	libservo = { path = '../other/servo/components/servo' }
	servo_net = { path = '../other/servo/components/net', package="net" }
	[patch.'https://github.com/mcclure/servo']
	libservo = { path = '../other/servo/components/servo' }
	servo_net = { path = '../other/servo/components/net', package="net" }

When building this way, anytime you commit to your servo checkout, you must manually record it by updating the git hashes for `libservo` and `servo_net` in `Cargo.toml`.

Contributors to this project (for PR submissions, etc) are expected to comply with the [Servo project rules](https://book.servo.org/contributing.html).
