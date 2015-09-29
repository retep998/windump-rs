
#![feature(fs_walk)]

extern crate regex;

use regex::{Regex};
use std::collections::{HashMap};
use std::fs::{File, walk_dir};
use std::io::{Read};
use std::path::{Path};

fn main() {
    let base = Path::new(r"C:\Program Files (x86)\Windows Kits\10\Include\10.0.10240.0");
    let reg1 = Regex::new(r#"^#include\s+<(.*)>"#).unwrap();
    let reg2 = Regex::new(r#"^#include\s+"(.*)""#).unwrap();
    let mut data: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    for entry in walk_dir(base).unwrap() {
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
        for line in text.lines() {
            let cap = reg1.captures(&line).and_then(|x| x.at(1))
                .or_else(|| reg2.captures(&line).and_then(|x| x.at(1)));
            if let Some(inc) = cap {
                let inc = inc.to_lowercase();
                data.entry(name.clone()).or_insert_with(|| (Vec::new(), Vec::new()))
                    .0.push(inc.clone());
                data.entry(inc).or_insert_with(|| (Vec::new(), Vec::new()))
                    .1.push(name.clone());
            }
        }
    }
    println!("{:#?}", data);
}