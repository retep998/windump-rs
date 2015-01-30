
#![feature(plugin)]
#![allow(unstable)]

extern crate regex;
#[plugin] #[no_link] extern crate regex_macros;

use std::borrow::ToOwned;
use std::collections::BTreeMap;
use std::default::Default;
use std::io::process::Command;
use std::os;

#[derive(Debug, Default)]
struct Export {
    arm: bool,
    x86: bool,
    x64: bool,
}

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

fn main() {
    let args = os::args();
    let mut map = BTreeMap::new();
    let name = &*args[1];
    for r in exports(name, "x86").into_iter() {
        map.entry(r).get().unwrap_or_else(|x| x.insert(<Export as Default>::default())).x86 = true;
    }
    for r in exports(name, "x64").into_iter() {
        map.entry(r).get().unwrap_or_else(|x| x.insert(<Export as Default>::default())).x64 = true;
    }
    for r in exports(name, "arm").into_iter() {
        map.entry(r).get().unwrap_or_else(|x| x.insert(<Export as Default>::default())).arm = true;
    }
    for (k, v) in map.iter() {
        match (v.arm, v.x86, v.x64) {
            (true, true, true) => (),
            (true, true, false) => println!("// #[cfg(any(target_arch = \"arm\", target_arch = \"x86\"))]"),
            (true, false, true) => println!("// #[cfg(any(target_arch = \"arm\", target_arch = \"x86_64\"))]"),
            (true, false, false) => println!("// #[cfg(target_arch = \"arm\")]"),
            (false, true, true) => println!("// #[cfg(any(target_arch = \"x86\", target_arch = \"x86_64\"))]"),
            (false, true, false) => println!("// #[cfg(target_arch = \"x86\")]"),
            (false, false, true) => println!("// #[cfg(target_arch = \"x86_64\")]"),
            (false, false, false) => unreachable!(),
        }
        println!("// pub fn {}();", k);
    }
}
