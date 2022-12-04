<div align="center">

# gateau 🍰

![Launch example](assets/example.svg)

🍪 Piping your **cookies** will be a piece of **cake** 🍰!

gateau is a command line tool to use your cookies from browsers (Chromium/Chrome and Firefox)
in your **curl**, **wget** and **httpie\*** requests or export them to a file.

</div>

<sub>

\* httpie support is experimental, as httpie sessions are neither documented nor stable.
See [httpie sessions](#httpie-sessions) for more information.

</sub>

## Installation

gateau supports all platforms supported by the browsers and the Rust toolchain,
including Linux, macOS and Windows.

### From source

```bash
cargo install --git github.com/musikid/gateau
```

### From binaries

Download the latest release from [here](https://github.com/musikid/gateau/releases/latest).

## Usage

gateau can be used to output cookies in different formats, notably `netscape`
format which is used by curl and wget to import cookies from a file,
and httpie sessions.
It can also be used to wrap commands (curl, wget, httpie) and import cookies directly without
having to use shell's [process substitution](https://en.wikipedia.org/wiki/Process_substitution) or manually create temporary files.
It imports cookies from Firefox by default if the `--browser` flag is not specified.

### Aliases

You can define aliases to make gateau easier to use.
For example, you can add the following aliases to your shell configuration file:

```bash
alias curlfire="curl -b <(gateau output --format netscape)"
alias chrul="curl -b <(gateau output --format netscape --browser chromium)"
alias wgetfire="wget --load-cookies <(gateau output --format netscape)"
alias wgetchr="wget --load-cookies <(gateau output --format netscape --browser chromium)"
alias httpfire="http --session <(gateau output --format httpie-session)"
alias httpchr="http --session <(gateau output --format httpie-session --browser chromium)"
```

<sub>

Thanks Copilot for the aliases...

</sub>

Please note that this would probably not work as you intended
if you use `curl` with [`-:`](https://curl.se/docs/manpage.html) and multiple URLs,
as the cookies would be imported for the first request only.

### Output cookies

#### cookies.txt format (curl, wget)

Since curl and wget can import cookies from a file in Netscape "cookies.txt" format,
gateau can be used to output cookies in this format and pipe them to curl/wget.

```bash
curl -b <(gateau output --format netscape) https://example.com
```

In this example, gateau will output cookies from Firefox in Netscape format,
and curl will import those which match the requested domains
and use them for the request.

```bash
wget --load-cookies <(gateau output --format netscape) https://example.com
```

Since wget can also import cookies from a file in Netscape "cookies.txt" format,
gateau can be used to output cookies in this format and pipe them to wget.

#### httpie sessions

gateau can also output cookies in httpie sessions format.
The support is experimental though, as httpie session format is neither documented nor stable.
The current implementation should be compatible with httpie 3.2.0+,
but it could change if httpie stop supporting this format in the future.

```bash
http --session <(gateau output --format httpie-session) example.com
```

### Wrapping commands

gateau can also wrap commands (curl, wget, httpie) and import cookies for them.

```bash
gateau wrap curl https://example.com
```

This will wrap the command `curl https://example.com` and import cookies for the request.

```bash
cat data | gateau wrap curl --bypass-lock -- -X POST -d @- httpbin.org/post
```

This will wrap the command `curl -X POST -d @- httpbin.org/post` and import cookies for the request.
The arguments and standard input are directly forwarded to the wrapped command, 
so you can use them as usual.
They are separated from the gateau arguments by `--`, so gateau will not parse them.
Note that it is optional if you do not use gateau arguments after the wrapped command, e.g:

```bash
cat data | gateau wrap curl --bypass-lock -- -X POST -d @- httpbin.org/post
```

is equivalent to:

```bash
cat data | gateau --bypass-lock wrap curl -X POST -d @- httpbin.org/post
```

httpie is also supported (experimental, as stated in [httpie sessions](#httpie-sessions)):

```bash
gateau wrap --browser=chromium http GET https://example.com
```

### Piping vs wrapping

gateau can be used mostly in two ways to import cookies: piping or wrapping.

#### Piping with process substitution

Piping is the most common way to use gateau, as it is the most flexible.
It allows you to use gateau with any command, as long as the used shell supports
[process substitution](https://en.wikipedia.org/wiki/Process_substitution).
It is the most secure, as it does not use a temporary file
to communicate with the command.
However, it is not always possible to use
[process substitution](https://serverfault.com/questions/688645/powershells-equivalent-to-bashs-process-substitution),
as it is not supported by all shells or OSes (Windows/cmd.exe for example).

#### Wrapping

Wrapping allows you to use gateau with a command without process substitution
and avoids having to manually create temporary files,
as long as the command is supported by gateau.

### Bypass database file locking

When Firefox is running, or when Chrome saves its cookies,
they lock their database files, so gateau cannot access
them. To bypass this, you can use the `--bypass-lock` flag.
Be aware that this flag is not recommended, as it could cause read errors
if the database is being modified at the same time, especially with Chrome.
Although, the database files are opened in read-only mode, so your cookies will not be
altered if an error occurs.

## Help

```
A simple wrapper to import cookies from browsers for curl, wget and httpie.

Usage: [-c ARG] [-b ARG] [--bypass-lock] COMMAND ...

Available options:
    -c, --cookie-db <ARG>  Ccookie database path
    -b, --browser <ARG>    Browser to import cookies from
        --bypass-lock      Bypass the lock on the database (can cause read errors)
    -h, --help             Prints help information
    -V, --version          Prints version information

Available commands:
    output  Output cookies to stdout in the specified format
    wrap    Wrap a command (curl, wget, httpie) with the imported cookies
```

## Performance

gateau is written in Rust, so it should be pretty fast, even if it is not
really optimized yet. Here are some non-scientific benchmarks:

- with a local server:

```
Benchmark 1: gateau wrap curl localhost:8000
  Time (mean ± σ):      14.2 ms ±   2.9 ms    [User: 8.0 ms, System: 5.3 ms]
  Range (min … max):    10.4 ms …  30.3 ms    176 runs

Benchmark 2: curl -b <(gateau output) localhost:8000
  Time (mean ± σ):       9.5 ms ±   1.2 ms    [User: 4.3 ms, System: 3.0 ms]
  Range (min … max):     7.8 ms …  17.0 ms    208 runs

Benchmark 3: curl -b /tmp/gateau-test localhost:8000
  Time (mean ± σ):       6.5 ms ±   2.2 ms    [User: 3.3 ms, System: 2.9 ms]
  Range (min … max):     3.7 ms …  16.5 ms    213 runs

Benchmark 4: curl localhost:8000
  Time (mean ± σ):       7.8 ms ±   2.5 ms    [User: 3.5 ms, System: 3.7 ms]
  Range (min … max):     3.8 ms …  14.1 ms    256 runs

Summary
  'curl -b /tmp/gateau-test localhost:8000' ran
    1.19 ± 0.55 times faster than 'curl localhost:8000'
    1.45 ± 0.52 times faster than 'curl -b <(gateau output) localhost:8000'
    2.18 ± 0.85 times faster than 'gateau wrap curl localhost:8000'
```

- with a remote server:

```
Benchmark 1: gateau wrap curl -sSL example.com
  Time (mean ± σ):     194.8 ms ±   6.7 ms    [User: 9.4 ms, System: 7.4 ms]
  Range (min … max):   183.5 ms … 207.5 ms    14 runs

Benchmark 2: curl -b <(gateau output) -sSL example.com
  Time (mean ± σ):     193.2 ms ±  14.8 ms    [User: 6.7 ms, System: 3.6 ms]
  Range (min … max):   182.2 ms … 229.1 ms    15 runs

Benchmark 3: curl -sSL example.com
  Time (mean ± σ):     186.2 ms ±  11.1 ms    [User: 3.4 ms, System: 4.9 ms]
  Range (min … max):   177.3 ms … 222.7 ms    15 runs

Summary
  'curl -sSL example.com' ran
    1.04 ± 0.10 times faster than 'curl -b <(gateau output) -sSL example.com'
    1.05 ± 0.07 times faster than 'gateau wrap curl -sSL example.com'
```

> The benchmarks were done with [hyperfine](https://github.com/sharkdp/hyperfine),
> with a Firefox cookie database containing around 1000 cookies
> and most importantly, with a _laptop_.

The results are not really consistent, as the benchmarks were done on a laptop,
but it seems that gateau incurs only a tiny overhead
which is not really noticeable in most cases.

Note that it takes more time to import cookies from Chrome than Firefox,
since Chrome encrypts its cookies while Firefox does not.
