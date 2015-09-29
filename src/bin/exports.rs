
#![feature(path_ext)]

extern crate enum_set;
extern crate regex;

use enum_set::{CLike, EnumSet};
use regex::{Regex};
use std::borrow::{ToOwned};
use std::collections::{BTreeMap};
use std::fs::{read_dir};
use std::io::{Write};
use std::mem::{transmute};
use std::fs::{File, PathExt};
use std::path::{Path, PathBuf};
use std::process::{Command};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Linkage {
    Cdecl,
    Fastcall,
    Stdcall,
    Static,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
enum Arch {
    X86,
    X64,
    Arm,
}
impl CLike for Arch {
    fn to_u32(&self) -> u32 {
        *self as u32
    }
    unsafe fn from_u32(v: u32) -> Arch {
        transmute(v)
    }
}
impl Arch {
    fn make_path(self, name: &str) -> PathBuf {
        let pbase = Path::new(r"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.10240.0\um");
        let arch = match self {
            Arch::X86 => "x86",
            Arch::X64 => "x64",
            Arch::Arm => "arm",
        };
        pbase.join(arch).join(name).with_extension("lib")
    }
    fn cfg_name(self) -> &'static str {
        match self {
            Arch::X86 => "x86",
            Arch::X64 => "x86_64",
            Arch::Arm => "arm",
        }
    }
}
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Export {
    name: String,
    link: Linkage,
    arch: Arch,
}
fn get_stuff(name: &str, arch: Arch) -> Vec<Export> {
    let plib = arch.make_path(name);
    if !plib.is_file() { return Vec::new() }
    let mut exports = exports(&plib, arch);
    exports.append(&mut symbols(&plib, arch));
    exports
}
fn symbols(plib: &Path, arch: Arch) -> Vec<Export> {
    let pdumpbin = Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\bin\amd64\dumpbin.exe");
    let output = Command::new(&pdumpbin).arg("/SYMBOLS").arg(plib).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let reg = if arch == Arch::X86 {
        Regex::new("^.* External +\\| _([a-zA-Z0-9_]+)$").unwrap()
    } else {
        Regex::new("^.* External +\\| ([a-zA-Z0-9_]+)$").unwrap()
    };
    stdout.lines().filter_map(|line| {
        reg.captures(line).map(|cap| {
            let name = cap.at(1).unwrap().to_owned();
            Export {
                name: name,
                link: Linkage::Static,
                arch: arch,
            }
        })
    }).filter(|thing| {
        !thing.name.contains("IMPORT_DESCRIPTOR") && !thing.name.contains("NULL_THUNK_DATA")
    }).collect()
}
fn exports(plib: &Path, arch: Arch) -> Vec<Export> {
    let pdumpbin = Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\bin\amd64\dumpbin.exe");
    let output = Command::new(&pdumpbin).arg("/EXPORTS").arg(plib).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines();
    loop {
        match lines.next() {
            Some("     Exports") => break,
            Some(_) => (),
            None => {
                println!("No exports found!");
                return Vec::new()
            },
        }
    }
    assert!(lines.next() == Some(""));
    assert!(lines.next() == Some("       ordinal    name"));
    assert!(lines.next() == Some(""));
    let mut exports = Vec::new();
    let system = Regex::new("^[ 0-9]{18}([a-zA-Z0-9_]+)$").unwrap();
    let stdcall = Regex::new("^[ 0-9]{18}_([a-zA-Z0-9_]+)@[0-9]+$").unwrap();
    let fastcall = Regex::new("^[ 0-9]{18}@([a-zA-Z0-9_]+)@[0-9]+$").unwrap();
    let cdecl = Regex::new("^[ 0-9]{18}_([a-zA-Z0-9_]+)+$").unwrap();
    loop {
        match lines.next() {
            Some("") => return exports,
            Some(line) => if arch == Arch::X86 {
                if let Some(cap) = stdcall.captures(line) {
                    let name = cap.at(1).unwrap().to_owned();
                    exports.push(Export {
                        name: name,
                        link: Linkage::Stdcall,
                        arch: arch,
                    });
                } else if let Some(cap) = fastcall.captures(line) {
                    let name = cap.at(1).unwrap().to_owned();
                    exports.push(Export {
                        name: name,
                        link: Linkage::Fastcall,
                        arch: arch,
                    });
                } else if let Some(cap) = cdecl.captures(line) {
                    let name = cap.at(1).unwrap().to_owned();
                    exports.push(Export {
                        name: name,
                        link: Linkage::Cdecl,
                        arch: arch,
                    });
                } else {
                    println!("Unknown {:?}: {:?}", arch, line);
                }
            } else {
                if let Some(cap) = system.captures(line) {
                    let name = cap.at(1).unwrap().to_owned();
                    exports.push(Export {
                        name: name,
                        link: Linkage::Stdcall,
                        arch: arch,
                    });
                } else {
                    println!("Unknown {:?}: {:?}", arch, line);
                }
            },
            None => panic!("Unexpected line!"),
        }
    }
}
fn export(name: &str) {
    println!("Dumping {:?}", name);
    let mut all = Vec::new();
    all.append(&mut get_stuff(name, Arch::X86));
    all.append(&mut get_stuff(name, Arch::X64));
    all.append(&mut get_stuff(name, Arch::Arm));
    if all.is_empty() {
        println!("Seriously nothing?");
        return
    }
    let mut combined: BTreeMap<_, EnumSet<_>> = BTreeMap::new();
    for Export { name, link, arch } in all {
        combined.entry((name, link)).or_insert(EnumSet::new()).insert(arch);
    }
    let mut results: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for ((name, link), archs) in combined {
        let archs: Vec<_> = archs.iter().collect();
        results.entry((link, archs)).or_insert(Vec::new()).push(name);
    }
    let mut file = File::create(&Path::new("work").join(name).with_extension("rs")).unwrap();
    for ((link, archs), names) in results {
        if archs.len() > 1 {
            write!(&mut file, "#[cfg(any(").unwrap();
            write!(&mut file, "target_arch = \"{}\"", archs[0].cfg_name()).unwrap();
            for arch in &archs[1..] {
                write!(&mut file, ", target_arch = \"{}\"", arch.cfg_name()).unwrap();
            }
            writeln!(&mut file, "))]").unwrap();
        } else if archs.len() == 1 {
            writeln!(&mut file, "#[cfg(target_arch = \"{}\")]", archs[0].cfg_name()).unwrap();
        } else { unreachable!() }
        writeln!(&mut file, "{}", match link {
            Linkage::Cdecl => "extern \"cdecl\" {",
            Linkage::Fastcall => "extern \"fastcall\" {",
            Linkage::Stdcall => "extern \"system\" {",
            Linkage::Static => "extern {",
        }).unwrap();
        for name in names {
            if link == Linkage::Static {
                writeln!(&mut file, "    // pub static {};", name).unwrap();
            } else {
                writeln!(&mut file, "    // pub fn {}();", name).unwrap();
            }
        }
        writeln!(&mut file, "}}").unwrap();
    }
}
fn do_exports() {
    let path = Path::new(r"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.10240.0\um");
    let mut names: Vec<_> = ["arm", "x86", "x64"].iter().flat_map(|arch|
        read_dir(path.join(arch)).unwrap().filter_map(|p|
            p.ok().and_then(|p|
                if let Ok(meta) = p.metadata() {
                    let path: PathBuf = p.path().to_str().unwrap().to_lowercase().into();
                    if meta.is_file() && path.extension() == Some("lib".as_ref()) {
                        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_owned())
                    } else { None }
                } else { None }
            )
        )
    ).collect();
    names.sort();
    names.dedup();
    for name in names { export(&name) }
}
fn main() {
    do_exports();
}