---
title: Getting Started
---

Welcome to gateau! This guide will help you get started
with using gateau to manage your browser cookies for `curl`, `wget`, and `httpie` requests.

gateau is a command line tool that allows you to use cookies
from your browsers (Chromium/Chrome and Firefox) in your `curl`, `wget`, and `httpie` requests,
or export them to a file.
This makes it easier to handle authenticated requests without manually copying cookies.

## Installation

gateau supports all platforms supported by the browsers and the Rust toolchain, including Linux, macOS, and Windows.

### SQLite Dependency

If you download one of the pre-built releases, SQLite will be bundled with the executable, and gateau will not require SQLite to be installed.
If you want to use the system's SQLite instead, you will have to build gateau from source.

### From Source

To install gateau from source, you need to have Rust installed. You can then use the following command:

```bash
# You can remove the `--features=bundled` flag if you have SQLite installed
# on your system and want to use the system's SQLite instead of the bundled one.
cargo install --git github.com/musikid/gateau --features=bundled
```

### From binaries

You can download the latest release of gateau from the [releases page](https://github.com/musikid/gateau/releases/latest). Choose the appropriate binary for your operating system, download it, and place it in a directory that is included in your system's `PATH`.
