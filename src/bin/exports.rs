
extern crate regex;

use regex::{Regex};
use std::collections::{HashMap};
use std::env::args;
use std::fs::{File};
use std::io::{BufWriter, Write};
use std::path::{Path};
use std::process::{Command};
use std::str::FromStr;

//Fields:
//DLL name = name of DLL
//Hint = ordinal hint
//Library = name of import library
//Machine = 8664 (x64)|14C (x86)
//Name = name of the symbol in the DLL itself
//Name type = undecorate|name|ordinal|no prefix
//Ordinal = ordinal of symbol in dll for exports with no name
//SizeOfData = size of data in bytes
//Symbol name = possibly mangled symbol that the code links against
//TimeDateStamp = some sort of time stamp
//Type = code|data|const
//Version = 0

pub const DUMPBIN: &'static str = r"C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\bin\amd64\dumpbin.exe";
pub const SDKBASE: &'static str = r"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.14393.0\um";
pub const WINBASE: &'static str = r"C:\Users\Peter\Code\winapi-rs";
pub const DLLTOOL64: &'static str = r"C:\msys64\mingw64\bin\dlltool.exe";
pub const DLLTOOL32: &'static str = r"C:\msys64\mingw32\bin\dlltool.exe";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Machine {
    X64,
    X86,
}
impl Machine {
    fn msvc(&self) -> &'static str {
        match self {
            &Machine::X64 => "x64",
            &Machine::X86 => "x86",
        }
    }
    fn rust(&self) -> &'static str {
        match self {
            &Machine::X64 => "x86_64",
            &Machine::X86 => "i686",
        }
    }
}
impl FromStr for Machine {
    type Err = ();
    fn from_str(s: &str) -> Result<Machine, ()> {
        Ok(match s {
            "8664 (x64)" => Machine::X64,
            "14C (x86)" => Machine::X86,
            x => panic!("Unknown Machine of {:?}", x),
        })
    }
}
#[derive(Debug)]
enum NameType {
    Undecorate,
    Name,
    Ordinal,
    NoPrefix,
}
impl FromStr for NameType {
    type Err = ();
    fn from_str(s: &str) -> Result<NameType, ()> {
        Ok(match s {
            "undecorate" => NameType::Undecorate,
            "name" => NameType::Name,
            "ordinal" => NameType::Ordinal,
            "no prefix" => NameType::NoPrefix,
            x => panic!("Unknown Name type of {:?}", x),
        })
    }
}
#[derive(Debug, Eq, PartialEq)]
enum Type {
    Code,
    Data,
    Const,
}
impl FromStr for Type {
    type Err = ();
    fn from_str(s: &str) -> Result<Type, ()> {
        Ok(match s {
            "code" => Type::Code,
            "data" => Type::Data,
            "const" => Type::Const,
            x => panic!("Unknown Type of {:?}", x),
        })
    }
}
#[derive(Debug)]
struct Export {
    dll: String,
    hint: Option<u32>,
    machine: Machine,
    name: Option<String>,
    name_type: NameType,
    ordinal: Option<u32>,
    size_of_data: u32,
    symbol_name: String,
    time_date_stamp: String,
    data_type: Type,
}

fn export(name: &str, arch: Machine) {
    println!("Working on {}", name);
    let plibmsvc = Path::new(SDKBASE).join(arch.msvc()).join(format!("{}.lib", name));
    let pdef = Path::new(WINBASE).join(arch.rust()).join("lib").join(format!("{}.def", name));
    let plibgnu = Path::new(WINBASE).join(arch.rust()).join("lib").join(format!("lib{}.a", name));
    let reg = Regex::new("^  ([a-zA-Z ]*?) *: (.*)$").unwrap();
    let cin = Command::new(DUMPBIN).arg("/HEADERS").arg(&plibmsvc).output().unwrap();
    let input = String::from_utf8_lossy(&cin.stdout);
    let mut fout = BufWriter::new(File::create(&pdef).unwrap());
    let mut next: HashMap<String, String> = HashMap::new();
    let mut exports: Vec<Export> = Vec::new();
    for line in input.lines() {
        if let Some(cap) = reg.captures(line) {
            let key = cap.at(1).unwrap();
            let val = cap.at(2).unwrap();
            next.insert(key.into(), val.into());
        } else if !next.is_empty() {
            //println!("{:?}", next);
            let version: u32 = next.remove("Version").unwrap().parse().unwrap();
            assert!(version == 0);
            let export = Export {
                dll: next.remove("DLL name").unwrap(),
                hint: next.remove("Hint").map(|x| x.parse().unwrap()),
                machine: next.remove("Machine").unwrap().parse().unwrap(),
                name: next.remove("Name"),
                name_type: next.remove("Name type").unwrap().parse().unwrap(),
                ordinal: next.remove("Ordinal").map(|x| x.parse().unwrap()),
                size_of_data: u32::from_str_radix(&next.remove("SizeOfData").unwrap(), 16).unwrap(),
                symbol_name: next.remove("Symbol name").unwrap(),
                time_date_stamp: next.remove("TimeDateStamp").unwrap(),
                data_type: next.remove("Type").unwrap().parse().unwrap(),
            };
            assert!(next.is_empty());
            exports.push(export);
        }
    }
    exports.sort_by(|a, b| (&a.dll, &a.name).cmp(&(&b.dll, &b.name)));
    let mut last_dll = String::new();
    for export in exports {
        if export.data_type != Type::Code {
            println!("Skipping non-code {:?}", export);
            continue
        }
        if export.dll != last_dll {
            writeln!(&mut fout, "LIBRARY {}", export.dll).unwrap();
            last_dll = export.dll.clone();
            writeln!(&mut fout, "EXPORTS").unwrap();
        }
        if export.symbol_name.contains("@@") {
            println!("Skipping C++ {:?}", export.name);
            continue
        }
        match export.name_type {
            NameType::Undecorate | NameType::NoPrefix => {
                let symbol = sanitize(&export.symbol_name, arch);
                writeln!(&mut fout, "{}", symbol).unwrap();
            },
            NameType::Name => {
                writeln!(&mut fout, "{}", export.symbol_name).unwrap();
            },
            NameType::Ordinal => {
                let symbol = sanitize(&export.symbol_name, arch);
                writeln!(&mut fout, "{} @{}", symbol, export.ordinal.unwrap()).unwrap();
            },
        }
    }
    drop(fout);
    let dlltool = match arch {
        Machine::X64 => DLLTOOL64,
        Machine::X86 => DLLTOOL32,
    };
    Command::new(dlltool).arg("-d").arg(&pdef).arg("-l").arg(&plibgnu).arg("-k").output().unwrap();
}
fn sanitize(symbol: &str, arch: Machine) -> &str {
    if arch != Machine::X86 { symbol }
    else if &symbol[0..1] == "_" { &symbol[1..] }
    else { symbol }
}
fn main() {
    for arg in args().skip(1) {
        export(&arg, Machine::X64);
        export(&arg, Machine::X86);
    }
}
