#![feature(path_relative_from)]

extern crate regex;

use regex::{Regex};
use std::collections::{HashSet};
use std::fs::{read_dir};
use std::io::{BufWriter, Read, Write};
use std::fs::{File};
use std::path::{Path};

fn write_toml(path: &Path, cratename: &str, libname: &str, version: &str, winapiver: &str, buildver: &str, keywords: &[&'static str]) {
    let file = File::create(path).unwrap();
    let mut file = BufWriter::new(file);
    writeln!(&mut file, r#"[package]
name = "{0}-sys"
version = "{1}"
authors = ["Peter Atashian <retep998@gmail.com>"]
description = "Contains function definitions for the Windows API library {0}. See winapi for types and constants."
documentation = "https://retep998.github.io/doc/{0}/"
repository = "https://github.com/retep998/winapi-rs"
readme = "README.md"
keywords = {5:?}
license = "MIT"
build = "build.rs"
[lib]
name = "{4}"
[dependencies]
winapi = {{ version = "{2}", path = "../.." }}
[build-dependencies]
winapi-build = {{ version = "{3}", path = "../../build" }}"#,//"
    cratename, version, winapiver, buildver, libname, keywords).unwrap();
}
fn write_build(path: &Path, nativename: &str, bundled: bool) {
    let file = File::create(path).unwrap();
    let mut file = BufWriter::new(file);
    writeln!(&mut file, r#"// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
extern crate build;
fn main() {{
    build::link("{0}", {1:?})
}}"#, nativename, bundled).unwrap();
}
fn write_readme(path: &Path, cratename: &str, libname: &str, version: &str) {
    let file = File::create(path).unwrap();
    let mut file = BufWriter::new(file);
    writeln!(&mut file, r#"# {} #
Contains function definitions for the Windows API library {0}. See winapi for types and constants.

```toml
[dependencies]
{0}-sys = "{1}"
```

```rust
extern crate {2};
```

[Documentation](https://retep998.github.io/doc/{0}/)"#, cratename, version, libname).unwrap();
}
fn extract_toml(path: &Path) -> String {
    let mut file = File::open(path).unwrap();
    let reg = Regex::new(r#"version = "([0-9]+.[0-9]+.[0-9]+)""#).unwrap();
    let mut string = String::new();
    file.read_to_string(&mut string).unwrap();
    let ver = reg.captures(&string).unwrap().at(1).unwrap();
    ver.to_owned()
}

fn main() {
    let lib = Path::new(r"C:\msys64\home\Peter\winapi-rs\lib");
    let winapiver = "0.2.5";
    let buildver = "0.1.1";
    let bundled: HashSet<String> = [
        "advapi32", "bcrypt", "comctl32", "comdlg32", "crypt32", "gdi32", "imagehlp", "iphlpapi",
        "kernel32", "odbc32", "ole32", "oleaut32", "opengl32", "psapi", "rpcrt4", "setupapi",
        "shell32", "user32", "userenv", "uuid", "winhttp", "winmm", "winspool", "ws2_32", "wsock32"
    ].iter().map(|&x| x.to_owned()).collect();
    let directx: HashSet<String> = [
        "d2d1", "d3d9", "d3d10", "d3d10_1", "d3d11", "d3d12", "d3dcompiler", "d3dcsx", "d3dcsxd",
        "ddraw", "dwrite", "dxgi", "dxguid"
    ].iter().map(|&x| x.to_owned()).collect();
    for sub in read_dir(lib).unwrap() {
        let sub = sub.unwrap();
        let path = sub.path();
        let nativename = path.relative_from(lib).unwrap().to_str().unwrap();
        let cratename = nativename.replace(".", "-");
        let libname = cratename.replace("-", "_");
        let ver = extract_toml(&path.join("Cargo.toml"));
        let mut keywords = vec!["windows", "ffi", "win32"];
        if directx.contains(nativename) { keywords.push("directx") }
        write_toml(&path.join("Cargo.toml"), &cratename, &libname, &ver, winapiver, buildver, &keywords);
        write_build(&path.join("build.rs"), &nativename, !bundled.contains(nativename));
        write_readme(&path.join("README.md"), &cratename, &libname, &ver);
    }
}