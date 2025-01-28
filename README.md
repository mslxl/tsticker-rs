<div align=center>

# tsticker-rs

[![NixOS](https://img.shields.io/badge/Made_for-Rust-orange.svg?logo=rust&style=for-the-badge)](https://www.rust-lang.org/) [![NixOS](https://img.shields.io/badge/Flakes-Nix-informational.svg?logo=nixos&style=for-the-badge)](https://nixos.org) ![License](https://img.shields.io/github/license/mslxl/tsticker-rs?style=for-the-badge)

Telegram sticker package download API and CLI

![Video](./screen.mp4)

</div>

## About

This is a rust version of [FHPythonUtils/TStickers](https://github.com/FHPythonUtils/TStickers), providing basic function for downloading
sticker packs from https://t.me/addstickers.

## Usage

```text
Usage: tsticker-cli [OPTIONS] <LINKS>...

Arguments:
  <LINKS>...  Sticker links, get it by sharing button

Options:
  -t, --token <TOKEN>    Telegram bot token, or set TELEGRAM_BOT_TOKEN in environment variable
  -o, --output <OUTPUT>  [default: current working directory]
  -f, --fast-failure
  -h, --help             Print help
  -V, --version          Print version
```

## Install

TODO: other installation methods are comming soon

### via GitHub Releases

See [Releases](https://github.com/mslxl/tstickers-rs/releases) for more details

### via Flake

```nix
-- TODO, see flake.nix for details
```

## As Rust crate

You can intergrate [tstickers-rs](tsticker/) as lib in your rust application

TODO: add example
