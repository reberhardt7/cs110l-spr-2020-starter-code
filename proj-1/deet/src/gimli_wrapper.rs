//! This file contains code for using gimli to extract information from the DWARF section of an
//! executable. The code is adapted from
//! https://github.com/gimli-rs/gimli/blob/master/examples/simple.rs and
//! https://github.com/gimli-rs/gimli/blob/master/examples/dwarfdump.rs.
//!
//! This code is a huge mess. Please don't read it unless you're trying to do an extension :)

use gimli;
use gimli::{UnitOffset, UnitSectionOffset};
use object::Object;
use std::borrow;
//use std::io::{BufWriter, Write};
use crate::dwarf_data::{File, Function, Line, Location, Type, Variable};
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::Write;
use std::{io, path};

pub fn load_file(object: &object::File, endian: gimli::RunTimeEndian) -> Result<Vec<File>, Error> {
    // Load a section and return as `Cow<[u8]>`.
    let load_section = |id: gimli::SectionId| -> Result<borrow::Cow<[u8]>, gimli::Error> {
        Ok(object
            .section_data_by_name(id.name())
            .unwrap_or(borrow::Cow::Borrowed(&[][..])))
    };
    // Load a supplementary section. We don't have a supplementary object file,
    // so always return an empty slice.
    let load_section_sup = |_| Ok(borrow::Cow::Borrowed(&[][..]));

    // Load all of the sections.
    let dwarf_cow = gimli::Dwarf::load(&load_section, &load_section_sup)?;

    // Borrow a `Cow<[u8]>` to create an `EndianSlice`.
    let borrow_section: &dyn for<'a> Fn(
        &'a borrow::Cow<[u8]>,
    ) -> gimli::EndianSlice<'a, gimli::RunTimeEndian> =
        &|section| gimli::EndianSlice::new(&*section, endian);

    // Create `EndianSlice`s for all of the sections.
    let dwarf = dwarf_cow.borrow(&borrow_section);

    // Define a mapping from type offsets to type structs
    let mut offset_to_type: HashMap<usize, Type> = HashMap::new();

    let mut compilation_units: Vec<File> = Vec::new();

    // Iterate over the compilation units.
    let mut iter = dwarf.units();
    while let Some(header) = iter.next()? {
        let unit = dwarf.unit(header)?;

        // Iterate over the Debugging Information Entries (DIEs) in the unit.
        let mut depth = 0;
        let mut entries = unit.entries();
        while let Some((delta_depth, entry)) = entries.next_dfs()? {
            depth += delta_depth;
            // Update the offset_to_type mapping for types
            // Update the variable list for formal params/variables
            match entry.tag() {
                gimli::DW_TAG_compile_unit => {
                    let name = if let Ok(Some(attr)) = entry.attr(gimli::DW_AT_name) {
                        if let Ok(DebugValue::Str(name)) = get_attr_value(&attr, &unit, &dwarf) {
                            name
                        } else {
                            "<unknown>".to_string()
                        }
                    } else {
                        "<unknown>".to_string()
                    };
                    compilation_units.push(File {
                        name,
                        global_variables: Vec::new(),
                        functions: Vec::new(),
                        lines: Vec::new(),
                    });
                }
                gimli::DW_TAG_base_type => {
                    let name = if let Ok(Some(attr)) = entry.attr(gimli::DW_AT_name) {
                        if let Ok(DebugValue::Str(name)) = get_attr_value(&attr, &unit, &dwarf) {
                            name
                        } else {
                            "<unknown>".to_string()
                        }
                    } else {
                        "<unknown>".to_string()
                    };
                    let byte_size = if let Ok(Some(attr)) = entry.attr(gimli::DW_AT_byte_size) {
                        if let Ok(DebugValue::Uint(byte_size)) =
                            get_attr_value(&attr, &unit, &dwarf)
                        {
                            byte_size
                        } else {
                            // TODO: report error?
                            0
                        }
                    } else {
                        // TODO: report error?
                        0
                    };
                    let type_offset = entry.offset().0;
                    offset_to_type
                        .insert(type_offset, Type::new(name, byte_size.try_into().unwrap()));
                }
                gimli::DW_TAG_subprogram => {
                    let mut func: Function = Default::default();
                    let mut attrs = entry.attrs();
                    while let Some(attr) = attrs.next()? {
                        let val = get_attr_value(&attr, &unit, &dwarf);
                        //println!("   {}: {:?}", attr.name(), val);
                        match attr.name() {
                            gimli::DW_AT_name => {
                                if let Ok(DebugValue::Str(name)) = val {
                                    func.name = name;
                                }
                            }
                            gimli::DW_AT_high_pc => {
                                if let Ok(DebugValue::Uint(high_pc)) = val {
                                    func.text_length = high_pc.try_into().unwrap();
                                }
                            }
                            gimli::DW_AT_low_pc => {
                                //println!("low pc {:?}", attr.value());
                                if let Ok(DebugValue::Uint(low_pc)) = val {
                                    func.address = low_pc.try_into().unwrap();
                                }
                            }
                            gimli::DW_AT_decl_line => {
                                if let Ok(DebugValue::Uint(line_number)) = val {
                                    func.line_number = line_number.try_into().unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                    compilation_units.last_mut().unwrap().functions.push(func);
                }
                gimli::DW_TAG_formal_parameter | gimli::DW_TAG_variable => {
                    let mut name = String::new();
                    let mut entity_type: Option<Type> = None;
                    let mut location: Option<Location> = None;
                    let mut line_number = 0;
                    let mut attrs = entry.attrs();
                    while let Some(attr) = attrs.next()? {
                        let val = get_attr_value(&attr, &unit, &dwarf);
                        //println!("   {}: {:?}", attr.name(), val);
                        match attr.name() {
                            gimli::DW_AT_name => {
                                if let Ok(DebugValue::Str(attr_name)) = val {
                                    name = attr_name;
                                }
                            }
                            gimli::DW_AT_type => {
                                if let Ok(DebugValue::Size(offset)) = val {
                                    if let Some(dtype) = offset_to_type.get(&offset).clone() {
                                        entity_type = Some(dtype.clone());
                                    }
                                }
                            }
                            gimli::DW_AT_location => {
                                if let Some(loc) = get_location(&attr, &unit) {
                                    location = Some(loc);
                                }
                            }
                            gimli::DW_AT_decl_line => {
                                if let Ok(DebugValue::Uint(num)) = val {
                                    line_number = num;
                                }
                            }
                            _ => {}
                        }
                    }
                    if entity_type.is_some() && location.is_some() {
                        let var = Variable {
                            name,
                            entity_type: entity_type.unwrap(),
                            location: location.unwrap(),
                            line_number: line_number.try_into().unwrap(),
                        };
                        if depth == 1 {
                            compilation_units
                                .last_mut()
                                .unwrap()
                                .global_variables
                                .push(var);
                        } else if depth > 1 {
                            compilation_units
                                .last_mut()
                                .unwrap()
                                .functions
                                .last_mut()
                                .unwrap()
                                .variables
                                .push(var);
                        }
                    }
                }
                // NOTE: :You may consider supporting other types by extending this
                // match statement
                _ => {}
            }
        }

        // Get line numbers
        if let Some(program) = unit.line_program.clone() {
            // Iterate over the line program rows.
            let mut rows = program.rows();
            while let Some((header, row)) = rows.next_row()? {
                if !row.end_sequence() {
                    // Determine the path. Real applications should cache this for performance.
                    let mut path = path::PathBuf::new();
                    if let Some(file) = row.file(header) {
                        if let Some(dir) = file.directory(header) {
                            path.push(dwarf.attr_string(&unit, dir)?.to_string_lossy().as_ref());
                        }
                        path.push(
                            dwarf
                                .attr_string(&unit, file.path_name())?
                                .to_string_lossy()
                                .as_ref(),
                        );
                    }

                    // Get the File
                    let file = compilation_units
                        .iter_mut()
                        .find(|f| f.name == path.as_os_str().to_str().unwrap());

                    // Determine line/column. DWARF line/column is never 0, so we use that
                    // but other applications may want to display this differently.
                    let line = row.line().unwrap_or(0);

                    if let Some(file) = file {
                        file.lines.push(Line {
                            file: file.name.clone(),
                            number: line.try_into().unwrap(),
                            address: row.address().try_into().unwrap(),
                        });
                    }
                }
            }
        }
    }
    Ok(compilation_units)
}

#[derive(Debug, Clone)]
pub enum DebugValue {
    Str(String),
    Uint(u64),
    Int(i64),
    Size(usize),
    NoVal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    GimliError(gimli::Error),
    Addr2lineError(addr2line::gimli::Error),
    ObjectError(String),
    IoError,
}

impl From<gimli::Error> for Error {
    fn from(err: gimli::Error) -> Self {
        Error::GimliError(err)
    }
}

impl From<addr2line::gimli::Error> for Error {
    fn from(err: addr2line::gimli::Error) -> Self {
        Error::Addr2lineError(err)
    }
}

impl From<io::Error> for Error {
    fn from(_: io::Error) -> Self {
        Error::IoError
    }
}

impl From<std::fmt::Error> for Error {
    fn from(_: std::fmt::Error) -> Self {
        Error::IoError
    }
}

impl<'input, Endian> Reader for gimli::EndianSlice<'input, Endian> where
    Endian: gimli::Endianity + Send + Sync
{
}

trait Reader: gimli::Reader<Offset = usize> + Send + Sync {}

fn get_location<R: Reader>(attr: &gimli::Attribute<R>, unit: &gimli::Unit<R>) -> Option<Location> {
    if let gimli::AttributeValue::Exprloc(ref data) = attr.value() {
        let encoding = unit.encoding();
        let mut pc = data.0.clone();
        if pc.len() > 0 {
            if let Ok(op) = gimli::Operation::parse(&mut pc, encoding) {
                match op {
                    gimli::Operation::FrameOffset { offset } => {
                        return Some(Location::FramePointerOffset(offset.try_into().unwrap()));
                    }
                    gimli::Operation::Address { address } => {
                        return Some(Location::Address(address.try_into().unwrap()));
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

// based on dwarf_dump.rs
fn get_attr_value<R: Reader>(
    attr: &gimli::Attribute<R>,
    unit: &gimli::Unit<R>,
    dwarf: &gimli::Dwarf<R>,
) -> Result<DebugValue, Error> {
    let value = attr.value();
    // TODO: get rid of w eventually
    let mut buf = String::new();
    let w = &mut buf;
    match value {
        gimli::AttributeValue::Exprloc(ref data) => {
            dump_exprloc(w, unit.encoding(), data)?;
            Ok(DebugValue::Str(w.to_string()))
        }
        gimli::AttributeValue::UnitRef(offset) => {
            match offset.to_unit_section_offset(unit) {
                UnitSectionOffset::DebugInfoOffset(goff) => {
                    Ok(DebugValue::Size(goff.0))
                }
                UnitSectionOffset::DebugTypesOffset(goff) => {
                    Ok(DebugValue::Size(goff.0))
                }
            }
        }
        gimli::AttributeValue::DebugStrRef(offset) => {
            if let Ok(s) = dwarf.debug_str.get_str(offset) {
                Ok(DebugValue::Str(format!("{}", s.to_string_lossy()?)))
            } else {
                Ok(DebugValue::Str(format!("<.debug_str+0x{:08x}>", offset.0)))
            }
        }
        gimli::AttributeValue::Sdata(data) => Ok(DebugValue::Int(data)),
        gimli::AttributeValue::Addr(data) => Ok(DebugValue::Uint(data)),
        gimli::AttributeValue::Udata(data) => Ok(DebugValue::Uint(data)),

        gimli::AttributeValue::String(s) => {
            Ok(DebugValue::Str(format!("{}", s.to_string_lossy()?)))
        }
        gimli::AttributeValue::FileIndex(value) => {
            write!(w, "0x{:08x}", value)?;
            dump_file_index(w, value, unit, dwarf)?;
            Ok(DebugValue::Str(w.to_string()))
        }
        _ => {
            Ok(DebugValue::NoVal)
        }
    }
}

fn dump_file_index<R: Reader, W: Write>(
    w: &mut W,
    file: u64,
    unit: &gimli::Unit<R>,
    dwarf: &gimli::Dwarf<R>,
) -> Result<(), Error> {
    if file == 0 {
        return Ok(());
    }
    let header = match unit.line_program {
        Some(ref program) => program.header(),
        None => return Ok(()),
    };
    let file = match header.file(file) {
        Some(header) => header,
        None => {
            writeln!(w, "Unable to get header for file {}", file)?;
            return Ok(());
        }
    };
    write!(w, " ")?;
    if let Some(directory) = file.directory(header) {
        let directory = dwarf.attr_string(unit, directory)?;
        let directory = directory.to_string_lossy()?;
        if !directory.starts_with('/') {
            if let Some(ref comp_dir) = unit.comp_dir {
                write!(w, "{}/", comp_dir.to_string_lossy()?,)?;
            }
        }
        write!(w, "{}/", directory)?;
    }
    write!(
        w,
        "{}",
        dwarf
            .attr_string(unit, file.path_name())?
            .to_string_lossy()?
    )?;
    Ok(())
}

fn dump_exprloc<R: Reader, W: Write>(
    w: &mut W,
    encoding: gimli::Encoding,
    data: &gimli::Expression<R>,
) -> Result<(), Error> {
    let mut pc = data.0.clone();
    let mut space = false;
    while pc.len() != 0 {
        let mut op_pc = pc.clone();
        let dwop = gimli::DwOp(op_pc.read_u8()?);
        match gimli::Operation::parse(&mut pc, encoding) {
            Ok(op) => {
                if space {
                    write!(w, " ")?;
                } else {
                    space = true;
                }
                dump_op(w, encoding, dwop, op)?;
            }
            Err(gimli::Error::InvalidExpression(op)) => {
                writeln!(w, "WARNING: unsupported operation 0x{:02x}", op.0)?;
                return Ok(());
            }
            Err(gimli::Error::UnsupportedRegister(register)) => {
                writeln!(w, "WARNING: unsupported register {}", register)?;
                return Ok(());
            }
            Err(gimli::Error::UnexpectedEof(_)) => {
                writeln!(w, "WARNING: truncated or malformed expression")?;
                return Ok(());
            }
            Err(e) => {
                writeln!(w, "WARNING: unexpected operation parse error: {}", e)?;
                return Ok(());
            }
        }
    }
    Ok(())
}

fn dump_op<R: Reader, W: Write>(
    w: &mut W,
    encoding: gimli::Encoding,
    dwop: gimli::DwOp,
    op: gimli::Operation<R>,
) -> Result<(), Error> {
    write!(w, "{}", dwop)?;
    match op {
        gimli::Operation::Deref {
            base_type, size, ..
        } => {
            if dwop == gimli::DW_OP_deref_size || dwop == gimli::DW_OP_xderef_size {
                write!(w, " {}", size)?;
            }
            if base_type != UnitOffset(0) {
                write!(w, " type 0x{:08x}", base_type.0)?;
            }
        }
        gimli::Operation::Pick { index } => {
            if dwop == gimli::DW_OP_pick {
                write!(w, " {}", index)?;
            }
        }
        gimli::Operation::PlusConstant { value } => {
            write!(w, " {}", value as i64)?;
        }
        gimli::Operation::Bra { target } => {
            write!(w, " {}", target)?;
        }
        gimli::Operation::Skip { target } => {
            write!(w, " {}", target)?;
        }
        gimli::Operation::SignedConstant { value } => match dwop {
            gimli::DW_OP_const1s
            | gimli::DW_OP_const2s
            | gimli::DW_OP_const4s
            | gimli::DW_OP_const8s
            | gimli::DW_OP_consts => {
                write!(w, " {}", value)?;
            }
            _ => {}
        },
        gimli::Operation::UnsignedConstant { value } => match dwop {
            gimli::DW_OP_const1u
            | gimli::DW_OP_const2u
            | gimli::DW_OP_const4u
            | gimli::DW_OP_const8u
            | gimli::DW_OP_constu => {
                write!(w, " {}", value)?;
            }
            _ => {
                // These have the value encoded in the operation, eg DW_OP_lit0.
            }
        },
        gimli::Operation::Register { register } => {
            if dwop == gimli::DW_OP_regx {
                write!(w, " {}", register.0)?;
            }
        }
        gimli::Operation::RegisterOffset {
            register,
            offset,
            base_type,
        } => {
            if dwop >= gimli::DW_OP_breg0 && dwop <= gimli::DW_OP_breg31 {
                write!(w, "{:+}", offset)?;
            } else {
                write!(w, " {}", register.0)?;
                if offset != 0 {
                    write!(w, "{:+}", offset)?;
                }
                if base_type != UnitOffset(0) {
                    write!(w, " type 0x{:08x}", base_type.0)?;
                }
            }
        }
        gimli::Operation::FrameOffset { offset } => {
            write!(w, " {}", offset)?;
        }
        gimli::Operation::Call { offset } => match offset {
            gimli::DieReference::UnitRef(gimli::UnitOffset(offset)) => {
                write!(w, " 0x{:08x}", offset)?;
            }
            gimli::DieReference::DebugInfoRef(gimli::DebugInfoOffset(offset)) => {
                write!(w, " 0x{:08x}", offset)?;
            }
        },
        gimli::Operation::Piece {
            size_in_bits,
            bit_offset: None,
        } => {
            write!(w, " {}", size_in_bits / 8)?;
        }
        gimli::Operation::Piece {
            size_in_bits,
            bit_offset: Some(bit_offset),
        } => {
            write!(w, " 0x{:08x} offset 0x{:08x}", size_in_bits, bit_offset)?;
        }
        gimli::Operation::ImplicitValue { data } => {
            let data = data.to_slice()?;
            write!(w, " 0x{:08x} contents 0x", data.len())?;
            for byte in data.iter() {
                write!(w, "{:02x}", byte)?;
            }
        }
        gimli::Operation::ImplicitPointer { value, byte_offset } => {
            write!(w, " 0x{:08x} {}", value.0, byte_offset)?;
        }
        gimli::Operation::EntryValue { expression } => {
            write!(w, "(")?;
            dump_exprloc(w, encoding, &gimli::Expression(expression))?;
            write!(w, ")")?;
        }
        gimli::Operation::ParameterRef { offset } => {
            write!(w, " 0x{:08x}", offset.0)?;
        }
        gimli::Operation::Address { address } => {
            write!(w, " 0x{:08x}", address)?;
        }
        gimli::Operation::AddressIndex { index } => {
            write!(w, " 0x{:08x}", index.0)?;
        }
        gimli::Operation::ConstantIndex { index } => {
            write!(w, " 0x{:08x}", index.0)?;
        }
        gimli::Operation::TypedLiteral { base_type, value } => {
            write!(w, " type 0x{:08x} contents 0x", base_type.0)?;
            for byte in value.to_slice()?.iter() {
                write!(w, "{:02x}", byte)?;
            }
        }
        gimli::Operation::Convert { base_type } => {
            write!(w, " type 0x{:08x}", base_type.0)?;
        }
        gimli::Operation::Reinterpret { base_type } => {
            write!(w, " type 0x{:08x}", base_type.0)?;
        }
        gimli::Operation::Drop
        | gimli::Operation::Swap
        | gimli::Operation::Rot
        | gimli::Operation::Abs
        | gimli::Operation::And
        | gimli::Operation::Div
        | gimli::Operation::Minus
        | gimli::Operation::Mod
        | gimli::Operation::Mul
        | gimli::Operation::Neg
        | gimli::Operation::Not
        | gimli::Operation::Or
        | gimli::Operation::Plus
        | gimli::Operation::Shl
        | gimli::Operation::Shr
        | gimli::Operation::Shra
        | gimli::Operation::Xor
        | gimli::Operation::Eq
        | gimli::Operation::Ge
        | gimli::Operation::Gt
        | gimli::Operation::Le
        | gimli::Operation::Lt
        | gimli::Operation::Ne
        | gimli::Operation::Nop
        | gimli::Operation::PushObjectAddress
        | gimli::Operation::TLS
        | gimli::Operation::CallFrameCFA
        | gimli::Operation::StackValue => {}
    };
    Ok(())
}
