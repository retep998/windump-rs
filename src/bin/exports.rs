extern crate regex;

use regex::Regex;
use std::collections::HashMap;
use std::env::args;
use std::fs::{read_dir, remove_file, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::{Command, Stdio};
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

pub const DUMPBIN: &'static str = r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Tools\MSVC\14.27.29110\bin\Hostx64\x64\dumpbin.exe";
pub const SDKBASE: &'static str = r"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.19041.0\um";
pub const WINBASE: &'static str = r"E:\Code\winapi-rs";
pub const AR64: &'static str = r"D:\Software\mingw64\x86_64-w64-mingw32\bin\ar.exe";
pub const AR32: &'static str = r"D:\Software\mingw32\i686-w64-mingw32\ar.exe";
pub const DLLTOOL64: &'static str = r"D:\Software\mingw64\x86_64-w64-mingw32\bin\dlltool.exe";
pub const DLLTOOL32: &'static str = r"D:\Software\mingw32\i686-w64-mingw32\bin\dlltool.exe";
pub const SDK64: &'static str = r"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.19041.0\um\x64";
pub const SDK32: &'static str = r"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.19041.0\um\x86";

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
impl Export {
    fn write<T>(&self, fout: &mut T, arch: Machine)
    where
        T: Write,
    {
        match self.name_type {
            NameType::Undecorate | NameType::NoPrefix => {
                let symbol = sanitize(&self.symbol_name, arch);
                writeln!(fout, "{}", symbol).unwrap();
            }
            NameType::Name => {
                writeln!(fout, "{}", self.symbol_name).unwrap();
            }
            NameType::Ordinal => {
                let symbol = sanitize(&self.symbol_name, arch);
                writeln!(fout, "{} @{}", symbol, self.ordinal.unwrap()).unwrap();
            }
        }
    }
}
fn export(name: &str, arch: Machine) {
    println!("Working on {}", name);
    let plibmsvc = Path::new(SDKBASE)
        .join(arch.msvc())
        .join(format!("{}.lib", name));
    if !plibmsvc.exists() {
        println!("Library does not exist!");
        return;
    }
    let reg = Regex::new("^  ([a-zA-Z][a-zA-Z ]*?) *: (.*)$").unwrap();
    let cin = Command::new(DUMPBIN)
        .arg("/HEADERS")
        .arg(&plibmsvc)
        .output()
        .unwrap();
    let input = String::from_utf8_lossy(&cin.stdout);
    let mut next: HashMap<String, String> = HashMap::new();
    let mut exports: Vec<Export> = Vec::new();
    for line in input.lines() {
        if let Some(cap) = reg.captures(line) {
            let key = cap.get(1).unwrap();
            let val = cap.get(2).unwrap();
            next.insert(key.as_str().into(), val.as_str().into());
        } else if !next.is_empty() {
            let version: u32 = next.remove("Version").unwrap().parse().unwrap();
            assert_eq!(version, 0);
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
    let mut dll_exports = HashMap::new();
    for export in exports {
        if export.data_type != Type::Code {
            // println!("Skipping non-code {:?}", export);
            continue;
        }
        if export.symbol_name.contains("@@") {
            // println!("Skipping C++ {:?}", export.symbol_name);
            continue;
        }
        dll_exports
            .entry(export.dll.clone())
            .or_insert_with(|| Vec::new())
            .push(export);
    }
    for (_, exports) in &mut dll_exports {
        exports.sort_by(|a, b| a.name.cmp(&b.name));
    }
    let dlltool = match arch {
        Machine::X64 => DLLTOOL64,
        Machine::X86 => DLLTOOL32,
    };
    if dll_exports.len() == 0 {
        return;
    } else if dll_exports.len() == 1 {
        let (dll, exports) = dll_exports.into_iter().next().unwrap();
        let pdef = Path::new(WINBASE)
            .join(arch.rust())
            .join("def")
            .join(format!("{}.def", name));
        let plibgnu = Path::new(WINBASE)
            .join(arch.rust())
            .join("lib")
            .join(format!("libwinapi_{}.a", name));
        println!("{}", pdef.display());
        let mut fout = BufWriter::new(File::create(&pdef).unwrap());
        writeln!(&mut fout, "LIBRARY {}", dll).unwrap();
        writeln!(&mut fout, "EXPORTS").unwrap();
        for export in exports {
            export.write(&mut fout, arch);
        }
        drop(fout);
        Command::new(dlltool)
            .arg("-d")
            .arg(&pdef)
            .arg("-l")
            .arg(&plibgnu)
            .arg("-k")
            .output()
            .unwrap();
    } else {
        let path = Path::new(WINBASE).join(arch.rust()).join("lib");
        let mut merged_libs = Vec::new();
        let mut mri_script = Vec::new();
        writeln!(&mut mri_script, "CREATE libwinapi_{}.a", name).unwrap();
        for (dll, exports) in dll_exports {
            let stem = Path::new(&dll)
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_lowercase();
            let pdef = Path::new(WINBASE)
                .join(arch.rust())
                .join("def")
                .join(format!("{}-{}.def", name, stem));
            let psublib = Path::new(WINBASE)
                .join(arch.rust())
                .join("lib")
                .join(format!("libwinapi_{}-{}.a", name, stem));
            let mut fout = BufWriter::new(File::create(&pdef).unwrap());
            writeln!(&mut fout, "LIBRARY {}", dll).unwrap();
            writeln!(&mut fout, "EXPORTS").unwrap();
            for export in exports {
                export.write(&mut fout, arch);
            }
            writeln!(&mut mri_script, "ADDLIB libwinapi_{}-{}.a", name, stem).unwrap();
            drop(fout);
            merged_libs.push(psublib.clone());
            Command::new(dlltool)
                .arg("-d")
                .arg(&pdef)
                .arg("-l")
                .arg(&psublib)
                .arg("-k")
                .output()
                .unwrap();
        }
        writeln!(&mut mri_script, "SAVE").unwrap();
        writeln!(&mut mri_script, "END").unwrap();
        let ar = match arch {
            Machine::X64 => AR64,
            Machine::X86 => AR32,
        };
        let mut child = Command::new(ar)
            .arg("-M")
            .current_dir(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .ok()
            .expect("failed to spawn process");

        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&mri_script)
            .unwrap();
        let output = child.wait_with_output().unwrap();
        if !output.status.success() {
            io::stdout().write_all(&output.stdout).unwrap();
            io::stderr().write_all(&output.stderr).unwrap();
        }
        merged_libs.sort();
        merged_libs.dedup();
        for file in merged_libs {
            remove_file(file).unwrap();
        }
    }
}
fn sanitize(symbol: &str, arch: Machine) -> &str {
    if arch != Machine::X86 {
        symbol
    } else if &symbol[0..1] == "_" {
        &symbol[1..]
    } else {
        symbol
    }
}
fn main() {
    let args = args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        for entry in read_dir(SDK64).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.to_lowercase() != "lib")
                .unwrap_or(true)
            {
                continue;
            }
            export(
                &path.file_stem().unwrap().to_str().unwrap().to_lowercase(),
                Machine::X64,
            );
        }
        for entry in read_dir(SDK32).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.to_lowercase() != "lib")
                .unwrap_or(true)
            {
                continue;
            }
            export(
                &path.file_stem().unwrap().to_str().unwrap().to_lowercase(),
                Machine::X86,
            );
        }
    } else {
        for arg in args {
            export(&arg, Machine::X64);
            export(&arg, Machine::X86);
        }
    }
}
