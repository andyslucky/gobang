<div align="center">

![gobang](../resources/logo.png)

gobang is currently in alpha

A cross-platform TUI database management tool written in Rust.
[Project Kanban Board](https://github.com/users/andyslucky/projects/2)

[![github workflow status](https://img.shields.io/github/workflow/status/andyslucky/gobang/CI/main)](https://github.com/andyslucky/gobang/actions)

[//]: # ([![crates]&#40;https://img.shields.io/crates/v/gobang.svg?logo=rust&#41;]&#40;https://crates.io/crates/gobang&#41;)
![gobang](../resources/gobang.gif)

</div>

## Features

- [X] Cross-platform support (macOS, Windows, Linux)
- [X] Build in multi-line editor for writing queries.
- [X] Auto completion
- [X] Multiple Database support (MySQL, PostgreSQL, SQLite)
- [X] Intuitive keyboard only control

## TODOs
- [ ] Custom/Customizable theme support.
- [ ] Saving editors and opening files.
- [ ] In app setting menu.
- [ ] Context-based autocomplete.
- [ ] Support the other databases.

## What does "gobang" come from?
*Quote from the author of the original gobang project.*

    gobang means a Japanese game played on goban, a go board. The appearance of goban looks like table structure. 
    And I live in Kyoto, Japan. In Kyoto city, streets are laid out on a grid (We call it “goban no me no youna (碁盤の目のような)”). 
    They are why I named this project "gobang".


[//]: # (## Installation)

[//]: # ()
[//]: # (### With Homebrew &#40;Linux, macOS&#41;)

[//]: # ()
[//]: # (If you’re using Homebrew or Linuxbrew, install the gobang formula:)

[//]: # ()
[//]: # (```)

[//]: # (brew install tako8ki/tap/gobang)

[//]: # (```)

[//]: # ()
[//]: # (### On Windows)

[//]: # ()
[//]: # (If you're a Windows Scoop user, then you can install gobang from the [official bucket]&#40;https://github.com/ScoopInstaller/Main/blob/master/bucket/gobang.json&#41;:)

[//]: # ()
[//]: # (```)

[//]: # (scoop install gobang)

[//]: # (```)

[//]: # ()
[//]: # (### On NetBSD)

[//]: # ()
[//]: # (If you're a NetBSD user, then you can install gobang from [pkgsrc]&#40;https://pkgsrc.se/databases/gobang&#41;:)

[//]: # ()
[//]: # (```)

[//]: # (pkgin install gobang)

[//]: # (```)

[//]: # ()
[//]: # (### With Cargo &#40;Linux, macOS, Windows&#41;)

[//]: # ()
[//]: # (If you already have a Rust environment set up, you can use the `cargo install` command:)

[//]: # ()
[//]: # (```)

[//]: # (cargo install --version 0.1.0-alpha.5 gobang)

[//]: # (```)

[//]: # ()
[//]: # (### From binaries &#40;Linux, macOS, Windows&#41;)

[//]: # ()
[//]: # (- Download the [latest release binary]&#40;https://github.com/TaKO8Ki/gobang/releases&#41; for your system)

[//]: # (- Set the `PATH` environment variable)

## Usage

```
$ gobang
```

```
$ gobang -h
USAGE:
    gobang [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config-path <config-path>    Set the config file
```

If you want to add connections, you need to edit your config file. For more information, please see [Configuration](#Configuration).

## Keymap

| Key | Description |
| ---- | ---- |
| <kbd>h</kbd>, <kbd>j</kbd>, <kbd>k</kbd>, <kbd>l</kbd> | Scroll left/down/up/right |
| <kbd>Ctrl</kbd> + <kbd>u</kbd>, <kbd>Ctrl</kbd> + <kbd>d</kbd> | Scroll up/down multiple lines |
| <kbd>g</kbd> , <kbd>G</kbd> | Scroll to top/bottom |
| <kbd>H</kbd>, <kbd>J</kbd>, <kbd>K</kbd>, <kbd>L</kbd> | Extend selection by one cell left/down/up/right |
| <kbd>y</kbd> | Copy a cell value |
| <kbd>←</kbd>, <kbd>→</kbd> | Move focus to left/right |
| <kbd>c</kbd> | Move focus to connections |
| <kbd>/</kbd> | Filter |
| <kbd>?</kbd> | Help |
| <kbd>1</kbd>, <kbd>2</kbd>, <kbd>3</kbd>, <kbd>4</kbd>, <kbd>5</kbd> | Switch to records/columns/constraints/foreign keys/indexes tab |

## Configuration

The location of the file depends on your OS:

- macOS: `$HOME/.config/gobang/config.toml`
- Linux: `$HOME/.config/gobang/config.toml`
- Windows: `%APPDATA%/gobang/config.toml`

The following is a sample config.toml file:

```toml
[[conn]]
type = "mysql"
user = "root"
host = "localhost"
port = 3306

[[conn]]
type = "mysql"
user = "root"
host = "localhost"
port = 3306
password = "password"
database = "foo"

[[conn]]
type = "postgres"
user = "root"
host = "localhost"
port = 5432
database = "bar"

[[conn]]
type = "sqlite"
path = "/path/to/baz.db"
```

## Contribution

Contributions, issues and pull requests are welcome!
