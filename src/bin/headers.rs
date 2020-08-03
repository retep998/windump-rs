
extern crate regex;
extern crate walkdir;

use regex::Regex;
use std::collections::{HashMap};
use std::fs::File;
use std::io::{Read};
use std::path::{Path};
use walkdir::WalkDir;

#[derive(Debug, Default)]
struct Header {
    includes: Vec<String>,
    included_by: Vec<String>,
}

fn main() {
    let base = Path::new(r"C:\Program Files (x86)\Windows Kits\10\Include\10.0.17763.0");
    let reg = Regex::new(r#"#include\s+[<"](.*)[>"]"#).unwrap();
    let reg1 = Regex::new(r#"^#include\s+<(.*)>"#).unwrap();
    let reg2 = Regex::new(r#"^#include\s+"(.*)""#).unwrap();
    let mut data: HashMap<String, Header> = HashMap::new();
    for entry in WalkDir::new(base) {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();
        if !meta.is_file() { continue }
        let path = entry.path();
        let ext = match path.extension() { Some(x) => x, None => continue };
        let ext = ext.to_str().unwrap().to_lowercase();
        if ext != "h" { continue }
        let name = match path.file_name() { Some(x) => x, None => continue };
        let name = name.to_str().unwrap().to_lowercase();
        let mut file = File::open(&path).unwrap();
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        for cap in reg.captures_iter(&text) {
            let inc = cap[1].to_lowercase();
            data.entry(name.clone()).or_default()
                .includes.push(inc.clone());
            data.entry(inc).or_default()
                .included_by.push(name.clone());
        }
    }
    println!("{:#?}", data);
}
