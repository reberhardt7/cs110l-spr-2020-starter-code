use crate::gimli_wrapper;
use addr2line::Context;
use object::Object;
use std::convert::TryInto;
use std::{fmt, fs};

#[derive(Debug)]
pub enum Error {
    ErrorOpeningFile,
    DwarfFormatError(gimli_wrapper::Error),
}

pub struct DwarfData {
    files: Vec<File>,
    addr2line: Context<addr2line::gimli::EndianRcSlice<addr2line::gimli::RunTimeEndian>>,
}

impl fmt::Debug for DwarfData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DwarfData {{files: {:?}}}", self.files)
    }
}

impl From<gimli_wrapper::Error> for Error {
    fn from(err: gimli_wrapper::Error) -> Self {
        Error::DwarfFormatError(err)
    }
}

impl DwarfData {
    pub fn from_file(path: &str) -> Result<DwarfData, Error> {
        let file = fs::File::open(path).or(Err(Error::ErrorOpeningFile))?;
        let mmap = unsafe { memmap::Mmap::map(&file).or(Err(Error::ErrorOpeningFile))? };
        let object = object::File::parse(&*mmap)
            .or_else(|e| Err(gimli_wrapper::Error::ObjectError(e.to_string())))?;
        let endian = if object.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };
        Ok(DwarfData {
            files: gimli_wrapper::load_file(&object, endian)?,
            addr2line: Context::new(&object).or_else(|e| Err(gimli_wrapper::Error::from(e)))?,
        })
    }

    #[allow(dead_code)]
    fn get_target_file(&self, file: &str) -> Option<&File> {
        self.files.iter().find(|f| {
            f.name == file || (!file.contains("/") && f.name.ends_with(&format!("/{}", file)))
        })
    }

    #[allow(dead_code)]
    pub fn get_addr_for_line(&self, file: Option<&str>, line_number: usize) -> Option<usize> {
        let target_file = match file {
            Some(filename) => self.get_target_file(filename)?,
            None => self.files.get(0)?,
        };
        Some(
            target_file
                .lines
                .iter()
                .find(|line| line.number >= line_number)?
                .address,
        )
    }

    #[allow(dead_code)]
    pub fn get_addr_for_function(&self, file: Option<&str>, func_name: &str) -> Option<usize> {
        match file {
            Some(filename) => Some(
                self.get_target_file(filename)?
                    .functions
                    .iter()
                    .find(|func| func.name == func_name)?
                    .address,
            ),
            None => {
                for file in &self.files {
                    if let Some(func) = file.functions.iter().find(|func| func.name == func_name) {
                        return Some(func.address);
                    }
                }
                None
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_line_from_addr(&self, curr_addr: usize) -> Option<Line> {
        let location = self
            .addr2line
            .find_location(curr_addr.try_into().unwrap())
            .ok()??;
        Some(Line {
            file: location.file?.to_string(),
            number: location.line?.try_into().unwrap(),
            address: curr_addr,
        })
    }

    #[allow(dead_code)]
    pub fn get_function_from_addr(&self, curr_addr: usize) -> Option<String> {
        let frame = self
            .addr2line
            .find_frames(curr_addr.try_into().unwrap())
            .ok()?
            .next()
            .ok()??;
        Some(frame.function?.raw_name().ok()?.to_string())
    }

    #[allow(dead_code)]
    pub fn print(&self) {
        for file in &self.files {
            println!("------");
            println!("{}", file.name);
            println!("------");

            println!("Global variables:");
            for var in &file.global_variables {
                println!(
                    "  * {} ({}, located at {}, declared at line {})",
                    var.name, var.entity_type.name, var.location, var.line_number
                );
            }

            println!("Functions:");
            for func in &file.functions {
                println!(
                    "  * {} (declared on line {}, located at {:#x}, {} bytes long)",
                    func.name, func.line_number, func.address, func.text_length
                );
                for var in &func.variables {
                    println!(
                        "    * Variable: {} ({}, located at {}, declared at line {})",
                        var.name, var.entity_type.name, var.location, var.line_number
                    );
                }
            }

            println!("Line numbers:");
            for line in &file.lines {
                println!("  * {} (at {:#x})", line.number, line.address);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Type {
    pub name: String,
    pub size: usize,
}

impl Type {
    pub fn new(name: String, size: usize) -> Self {
        Type {
            name: name,
            size: size,
        }
    }
}

#[derive(Clone)]
pub enum Location {
    Address(usize),
    FramePointerOffset(isize),
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Location::Address(addr) => write!(f, "Address({:#x})", addr),
            Location::FramePointerOffset(offset) => write!(f, "FramePointerOffset({})", offset),
        }
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// For variables and formal parameters
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub entity_type: Type,
    pub location: Location,
    pub line_number: usize, // Line number in source file
}

#[derive(Debug, Default, Clone)]
pub struct Function {
    pub name: String,
    pub address: usize,
    pub text_length: usize,
    pub line_number: usize, // Line number in source file
    pub variables: Vec<Variable>,
}

#[derive(Debug, Default, Clone)]
pub struct File {
    pub name: String,
    pub global_variables: Vec<Variable>,
    pub functions: Vec<Function>,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub file: String,
    pub number: usize,
    pub address: usize,
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.number)
    }
}


