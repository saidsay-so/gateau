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

### Output (piping) cookies

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
wget --load-cookies <(gateau output --browser=chrome --format netscape) https://example.com
```

Since wget can also import cookies from a file in Netscape "cookies.txt" format,
gateau can be used to output cookies in this format and pipe them to wget.

#### httpie sessions

gateau can also output cookies in httpie sessions format.
The support is experimental though, as httpie session format is neither documented nor stable.
The current implementation should be compatible with httpie 3.2.0+,
but it could change if httpie stop supporting this format in the future.

```bash
http --session-read-only <(gateau output --format httpie-session example.com) example.com
```

In this example, gateau will output cookies from Firefox in httpie session format,
and httpie will import it as an anonymous session.

You can also save named sessions,
by writing them to file which can be then used with `--session`:

```bash
# Just an example, should be changed
HOST=adventofcode.com
SESSION_NAME=aoc
# Usual path for httpie sessions on Unix systems, see https://httpie.io/docs#sessions
# and https://httpie.io/docs/cli/config-file-directory
CONFIG_PATH=${XDG_CONFIG_HOME:-$HOME/.config}
gateau output --format=httpie-session $HOST > $CONFIG_PATH/httpie/sessions/$HOST/$SESSION_NAME.json
https --session=$SESSION_NAME $HOST
```

### Aliases

You can define aliases to make gateau easier to use.
For example, you can add the following aliases to your shell configuration file:

```bash
alias curlfire="curl -b <(gateau output --format netscape)"
alias chrul="curl -b <(gateau output --format netscape --browser chromium)"
alias wgetfire="wget --load-cookies <(gateau output --format netscape)"
alias wgetchr="wget --load-cookies <(gateau output --format netscape --browser chromium)"
alias httpfire="http --session-read-only <(gateau output --format httpie-session)"
alias httpchr="http --session-read-only <(gateau output --format httpie-session --browser chromium)"
alias httpsfire="https --session-read-only <(gateau output --format httpie-session)"
alias httpschr="https --session-read-only <(gateau output --format httpie-session --browser chromium)"
```

Please note that this would probably not work as intended
if you use `curl` with [`-:`](https://curl.se/docs/manpage.html) and multiple URLs,
as the cookies would be imported for the first request only.

### Windows users

If you are using Windows, you can either use a shell which supports process substitution
(bash, zsh, fish, etc.) or use the `wrap` command.

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
to pass the cookies to the command.
However, it is not always 
[possible](https://serverfault.com/questions/688645/powershells-equivalent-to-bashs-process-substitution)
to use process substitution,
as it is not supported by all shells (cmd.exe or Powershell for example).

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

### Session

It is possible to use gateau to create a browser session within a new context,
and output the cookies after the session is finished.

```bash
gateau wrap --browser=firefox --session -- curl https://example.com
```

## Help

```
> gateau --help

A simple wrapper to import cookies from browsers for curl, wget and httpie.

Usage: [-c ARG] [-b ARG] [--bypass-lock] COMMAND ...

Available options:
    -c, --cookie-db <ARG>  Ccookie database path
    -b, --browser <ARG>    Browser(s) to import cookies from
        --bypass-lock      Bypass the lock on the database (can cause read errors)
    -h, --help             Prints help information
    -V, --version          Prints version information

Available commands:
    output  Output cookies to stdout in the specified format
    wrap    Wrap a command with the imported cookies

> gateau output --help

Output cookies to stdout in the specified format

Usage: [--format ARG] [--session] --session-urls ARG... <HOSTS>...

Available positional items:
    <HOSTS>  Hosts to filter cookies by

Available options:
        --format <ARG>        Output format
                              Supported formats: netscape, httpie-session
        --session             Open the browser in a new context and output the saved cookies when it closes
        --session-urls <ARG>  URL to open in the session
    -h, --help                Prints help information

> gateau wrap --help

Wrap a command with the imported cookies

Usage: <COMMAND> <ARGS>...

Available positional items:
    <COMMAND>  Command which should be wrapped
               Supported commands: curl, wget, http, https
    <ARGS>      Arguments for the wrapped command

Available options:
    -h, --help  Prints help information
```

## Performance

gateau is written in Rust, so it should be pretty fast, even if it is not
really optimized yet. Here are some non-scientific benchmarks:

```
Benchmark 1: gateau wrap curl localhost:8000
  Time (mean ± σ):      14.4 ms ±   2.3 ms    [User: 6.3 ms, System: 5.0 ms]
  Range (min … max):    10.7 ms …  20.5 ms    200 runs

Benchmark 2: curl <(gateau output) localhost:8000
  Time (mean ± σ):       9.8 ms ±   1.6 ms    [User: 3.6 ms, System: 3.2 ms]
  Range (min … max):     7.3 ms …  21.1 ms    200 runs

Benchmark 3: curl localhost:8000
  Time (mean ± σ):       9.2 ms ±   2.1 ms    [User: 3.4 ms, System: 3.0 ms]
  Range (min … max):     6.3 ms …  14.3 ms    200 runs

Summary
  'curl localhost:8000' ran
    1.07 ± 0.30 times faster than 'curl <(gateau output) localhost:8000'
    1.56 ± 0.44 times faster than 'gateau wrap curl localhost:8000'
```

> The benchmarks were done with [hyperfine](https://github.com/sharkdp/hyperfine),
> with a Firefox cookie database containing around 1000 cookies
> and most importantly, with a _laptop_.

The results are not really consistent, as the benchmarks were done on a laptop,
but it seems that gateau incurs only a tiny overhead
which is not really noticeable in most cases.

Note that it takes more time to import cookies from Chrome than Firefox,
since Chrome encrypts its cookies while Firefox does not.
