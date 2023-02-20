use std::fs::File;
use std::io::BufRead;
use std::process;
use std::{env, io};

fn words(line: &str) -> usize {
    line.split(' ').collect::<String>().len()
}

fn read_file_lines(filename: &String) -> Result<Vec<String>, io::Error> {
    let mut lines = Vec::new();
    let file = File::open(filename)?;
    for line in io::BufReader::new(file).lines() {
        lines.push(line?);
    }

    Ok(lines)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Too few arguments.");
        process::exit(1);
    }
    let filename = &args[1];
    // Your code here :)
    let lines = read_file_lines(filename).expect("read error");
    let mut word_number = 0;
    let mut char_number = 0;

    for line in &lines {
        word_number += words(line);
        char_number += line.chars().count();
        char_number += 1; // \n
    }
    char_number -= 1; // last \n

    println!("words: {word_number}");
    println!("lines: {}", &lines.len());
    println!("characters: {char_number}");
}
