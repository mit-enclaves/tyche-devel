use std::path::PathBuf;

use log::info;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::allocator::PAGE_SIZE;
use crate::elf_modifier::{ModifiedELF, TychePhdrTypes};
use crate::page_table_mapper::generate_page_tables;

pub const DEFAULT_STACK_SIZE: usize = 0x5000;

#[derive(Debug, Serialize, Deserialize)]
pub struct SegmentDescriptor {
    start: Option<u64>,
    size: usize,
    tpe: TychePhdrTypes,
    write: bool,
    exec: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BinaryOperation {
    SectionToSegment(String, TychePhdrTypes),
    AddSegment(SegmentDescriptor),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BinaryInstrumentation {
    /// Path to the binary.
    path: String,
    /// Extra operations to perform on the binary.
    ops: Option<Vec<BinaryOperation>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    /// User binary
    user_bin: Option<BinaryInstrumentation>,
    /// Kernel binary
    kern_bin: Option<BinaryInstrumentation>,
    /// Destination ELF file.
    output: String,
}

pub fn modify_binary(src: &PathBuf, dst: &PathBuf) {
    let data = std::fs::read(src).expect("Unable to read source file");
    info!("We read {} bytes from the file", data.len());
    let mut elf = ModifiedELF::new(&*data);

    // Move default shared buffer into its own segment.
    /*elf.split_segment_at_section(
        ".tyche_shared_default_buffer",
        TychePhdrTypes::KernelShared as u32,
        &data,
    )
    .expect("Failed to split section into segment");*/

    // Add the enclave stack as a segment.
    /*elf.append_nodata_segment(
        None,
        TychePhdrTypes::KernelStack as u32,
        object::elf::PF_R | object::elf::PF_W,
        DEFAULT_STACK_SIZE,
    );*/

    // TODO we could add a confidential heap here.
    // Or declare it as a section and move it into its own segment.

    // Create page tables.
    let (pts, nb_pages) = generate_page_tables(&*elf);
    elf.append_data_segment(
        Some(0),
        TychePhdrTypes::PageTables as u32,
        object::elf::PF_R | object::elf::PF_W,
        nb_pages * PAGE_SIZE,
        &pts,
    );

    // Let's write that thing out.
    //let mut out: Vec<u8> = Vec::with_capacity(elf.len());
    //let mut writer = object::write::elf::Writer::new(Endianness::Little, true, &mut out);
    elf.dump(dst);

    /*let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(dst)
        .expect("Unable to open dest file");
    file.write(&*out).expect("Unable to dump the content");*/
}

pub fn instrument_with_manifest(src: &PathBuf) {
    // Parse the manifest.
    let manifest: Manifest = {
        let data = std::fs::read(src).expect("Unable to read source file");
        let content = String::from_utf8(data).expect("Unable to convert data to string");
        serde_json::from_str(&content).expect("Failed to parse JSON")
    };
    instrument_binary(&manifest);
}

/// Parse singular binary instrumentation description and applies its operations.
pub fn parse_binary(binary: &BinaryInstrumentation) -> Box<ModifiedELF> {
    let data = std::fs::read(PathBuf::from(&binary.path)).expect("Unable to read the binary");
    let mut elf = ModifiedELF::new(&*data);

    // Apply all the operations.
    if let Some(operations) = &binary.ops {
        for op in operations {
            match &op {
                BinaryOperation::SectionToSegment(section, tpe) => {
                    elf.split_segment_at_section(section, *tpe as u32, &data)
                        .expect("Unable to perform the desired split operation");
                }
                BinaryOperation::AddSegment(descr) => {
                    let mut rights = object::elf::PF_R;
                    if descr.write {
                        rights |= object::elf::PF_W;
                    }
                    if descr.exec {
                        rights |= object::elf::PF_X;
                    }
                    elf.append_nodata_segment(descr.start, descr.tpe as u32, rights, descr.size);
                }
            }
        }
    }
    elf
}

pub fn instrument_binary(manifest: &Manifest) {
    // Parse the user binary if present.
    let mut user_elf = if let Some(user) = &manifest.user_bin {
        let mut bin = parse_binary(user);
        bin.mark(TychePhdrTypes::UserConfidential);
        Some(bin)
    } else {
        None
    };
    // Parse the kernel binary if present.
    let kern_elf = if let Some(kern) = &manifest.kern_bin {
        let mut bin = parse_binary(kern);
        bin.mark(TychePhdrTypes::KernelConfidential);
        Some(bin)
    } else {
        None
    };

    // Complex case.
    if let (Some(ref mut user), Some(kern)) = (&mut user_elf, &kern_elf) {
        let user = user.as_mut();
        let kern = kern.as_ref();
        if user.overlap(kern) {
            panic!("The two binaries overlap");
        }
        user.merge(kern);
        user.generate_page_tables();
        user.dump(&PathBuf::from(&manifest.output));
        return;
    }
    // Simpler cases
    if let Some(mut user) = user_elf {
        user.generate_page_tables();
        user.dump(&PathBuf::from(&manifest.output));
        return;
    }

    if let Some(mut kern) = kern_elf {
        kern.generate_page_tables();
        kern.dump(&PathBuf::from(&manifest.output));
        return;
    }

    panic!("Should never reach");
}

pub fn print_enum() {
    let op = BinaryInstrumentation {
        path: "templates/app".to_string(),
        ops: Some(vec![
            BinaryOperation::AddSegment(SegmentDescriptor {
                start: None,
                size: 0x2000,
                tpe: TychePhdrTypes::UserStack,
                write: true,
                exec: false,
            }),
            BinaryOperation::AddSegment(SegmentDescriptor {
                start: None,
                size: 0x2000,
                tpe: TychePhdrTypes::UserShared,
                write: true,
                exec: false,
            }),
        ]),
    };
    let json = serde_json::to_string(&op).unwrap();
    log::info!("The generated json {}", json);
}
