
#![feature(collections, core, io, os, path, plugin)]

extern crate regex;
#[plugin] #[no_link] extern crate regex_macros;

use std::borrow::ToOwned;
use std::collections::BTreeMap;
use std::old_io::process::Command;
use std::os;

fn exports(name: &str, arch: &str) -> Vec<String> {
    let pdumpbin = Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\bin\amd64\dumpbin.exe");
    let pbase = Path::new(r"C:\Program Files (x86)\Windows Kits\8.1\Lib\winv6.3\um");
    let plib = pbase.join(arch).join(name).with_extension("lib");
    let output = Command::new(&pdumpbin).arg("/EXPORTS").arg(&plib).output().unwrap();
    let stdout = String::from_utf8(output.output).unwrap();
    let reg = if arch == "x86" {
        regex!("^ {18}(?:@|_)([^ ]+?)(?:@[0-9]+)?$")
    } else {
        regex!("^ {18}([^ ]+)$")
    };
    stdout.lines_any().filter_map(|line| {
        reg.captures(line).map(|c| c.at(1).unwrap().to_owned())
    }).collect()
}

fn print_cfg(cfg: &[&str], c: bool) {
    if c { print!("#![cfg(") } else { print!("// #[cfg(") }
    if cfg.len() > 1 {
        print!("any(");
        print!("target_arch = \"{}\"", cfg[0]);
        for n in cfg[1..].iter() { print!(", target_arch = \"{}\"", n) }
        print!(")");
    } else {
        print!("target_arch = \"{}\"", cfg[0]);
    }
    println!(")]");
}

fn main() {
    let args = os::args();
    let mut map = BTreeMap::new();
    let mut all = Vec::new();
    let name = &*args[1];
    {
        let mut import = |a1: &str, a2: &'static str| {
            let e = exports(name, a1);
            if e.is_empty() { return }
            all.push(a2);
            for r in e.into_iter() {
                map.entry(r).get().unwrap_or_else(|x| x.insert(Vec::new())).push(a2);
            }
        };
        import("x86", "x86");
        import("x64", "x86_64");
        import("arm", "arm");
    }
    if all.is_empty() {
        println!("No exports found!");
        return;
    }
    if all.len() < 3 { print_cfg(&*all, true) }
    for (k, v) in map.iter() {
        if v.len() < all.len() { print_cfg(&**v, false) }
        println!("// pub fn {}();", k);
    }
}
