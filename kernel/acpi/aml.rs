use collections::string::String;

use core::mem::size_of;
use core::num::Zero;
use core::ops::{BitOrAssign, ShlAssign};

const ZERO_OP: u8 = 0x00;
const ONE_OP: u8 = 0x01;
const NAME_OP: u8 = 0x08;
const BYTE_PREFIX: u8 = 0x0A;
const WORD_PREFIX: u8 = 0x0B;
const DWORD_PREFIX: u8 = 0x0C;
const STRING_PREFIX: u8 = 0x0D;
const QWORD_PREFIX: u8 = 0x0E;
const SCOPE_OP: u8 = 0x10;
const BUFFER_OP: u8 = 0x11;
const PACKAGE_OP: u8 = 0x12;
const METHOD_OP: u8 = 0x14;
const DUAL_NAME_PREFIX: u8 = 0x2E;
const MULTI_NAME_PREFIX: u8 = 0x2F;
const EXT_OP_PREFIX: u8 = 0x5B;
const ROOT_PREFIX: u8 = 0x5C;
const PARENT_PREFIX: u8 = 0x5E;

// EXT
const MUTEX_OP: u8 = 0x01;
const OP_REGION_OP: u8 = 0x80;
const FIELD_OP: u8 = 0x81;
const DEVICE_OP: u8 = 0x82;
const PROCESSOR_OP: u8 = 0x83;

pub fn parse_string(bytes: &[u8], i: &mut usize) -> String {
    let mut string = String::new();

    while *i < bytes.len() {
        let c = bytes[*i];
        if (c >= 0x30 && c <= 0x39) || (c >= 0x41 && c <= 0x5A) || c == 0x5F ||
           c == ROOT_PREFIX || c == PARENT_PREFIX {
            string.push(c as char);
        } else {
            break;
        }

        *i += 1;
    }

    string
}

// This one function required three different unstable features and four trait requirements. Why is generic math so hard?
pub fn parse_num<T: BitOrAssign + From<u8> + ShlAssign<usize> + Zero>(bytes: &[u8],
                                                                      i: &mut usize)
                                                                      -> T {
    let mut num: T = T::zero();

    let mut shift = 0;
    while *i < bytes.len() && shift < size_of::<T>() * 8 {
        let mut b = T::from(bytes[*i]);
        b <<= shift;
        num |= b;
        shift += 8;
        *i += 1;
    }

    num
}

pub fn parse_length(bytes: &[u8], i: &mut usize) -> usize {
    let mut length = 0;

    if *i < bytes.len() {
        let b = bytes[*i] as usize;

        let mut follow = (b & 0b11000000) >> 6;
        if follow == 0 {
            length += b & 0b111111;
        } else {
            length += b & 0b1111;
        }

        *i += 1;

        let mut shift = 4;
        while *i < bytes.len() && follow > 0 {
            length += (bytes[*i] as usize) << shift;

            shift += 8;
            follow -= 1;
            *i += 1;
        }
    }

    length
}


pub fn parse_name(bytes: &[u8], i: &mut usize) -> String {
    let mut name = String::new();

    let mut count = 0;
    while *i < bytes.len() {
        match bytes[*i] {
            ZERO_OP => {
                *i += 1;

                count = 0;
                break;
            }
            DUAL_NAME_PREFIX => {
                *i += 1;

                count = 2;
                break;
            }
            MULTI_NAME_PREFIX => {
                *i += 1;

                if *i < bytes.len() {
                    count = bytes[*i];
                    *i += 1;
                }

                break;
            }
            ROOT_PREFIX => {
                *i += 1;

                name.push('\\');
            }
            PARENT_PREFIX => {
                *i += 1;

                name.push('^');
            }
            _ => {
                count = 1;
                break;
            }
        };
    }

    while count > 0 {
        if !name.is_empty() {
            name.push('.');
        }

        let end = *i + 4;
        let mut leading = true;
        while *i < bytes.len() && *i < end {
            let c = bytes[*i];
            if (c >= 0x30 && c <= 0x39) || (c >= 0x41 && c <= 0x5A) {
                leading = false;
                name.push(c as char);
            } else if c == 0x5F {
                if leading {
                    name.push('_');
                }
            } else {
                debugln!("parse_name: unknown: {:02X}", c);
                break;
            }

            *i += 1;
        }

        *i = end;

        count -= 1;
    }

    name
}


pub fn parse_int(bytes: &[u8], i: &mut usize) -> u64 {
    if *i < bytes.len() {
        let b = bytes[*i];
        *i += 1;

        match b {
            ZERO_OP => return 0,
            ONE_OP => return 1,
            BYTE_PREFIX => return parse_num::<u8>(bytes, i) as u64,
            WORD_PREFIX => return parse_num::<u16>(bytes, i) as u64,
            DWORD_PREFIX => return parse_num::<u32>(bytes, i) as u64,
            QWORD_PREFIX => return parse_num::<u64>(bytes, i),
            _ => debugln!("parse_int: unknown: {:02X}", b),
        }
    }

    return 0;
}

pub fn parse_package(bytes: &[u8], i: &mut usize) {

    let end = *i + parse_length(bytes, i);
    let elements = parse_num::<u8>(bytes, i);

    debugln!("    Package ({})", elements);
    debugln!("    {{");
    while *i < bytes.len() && *i < end {
        let op = bytes[*i];
        *i += 1;

        match op {
            ZERO_OP => {
                debugln!("        Zero");
            }
            ONE_OP => {
                debugln!("        One");
            }
            BYTE_PREFIX => {
                debugln!("        {:02X}", parse_num::<u8>(bytes, i));
            }
            WORD_PREFIX => {
                debugln!("        {:04X}", parse_num::<u16>(bytes, i));
            }
            DWORD_PREFIX => {
                debugln!("        {:08X}", parse_num::<u32>(bytes, i));
            }
            QWORD_PREFIX => {
                debugln!("        {:016X}", parse_num::<u64>(bytes, i));
            }
            PACKAGE_OP => {
                parse_package(bytes, i);
            }
            _ => {
                *i -= 1;
                debugln!("        {}", parse_name(bytes, i));
                // debugln!("        parse_package: unknown: {:02X}", op);
            }
        }
    }
    debugln!("    }}");

    *i = end;
}

pub fn parse_device(bytes: &[u8], i: &mut usize) {
    let end = *i + parse_length(bytes, i);
    let name = parse_name(bytes, i);

    debugln!("    Device ({})", name);
    debugln!("    {{");
    while *i < bytes.len() && *i < end {
        let op = bytes[*i];
        *i += 1;

        match op {
            ZERO_OP => {
                debugln!("        Zero");
            }
            ONE_OP => {
                debugln!("        One");
            }
            BYTE_PREFIX => {
                debugln!("        {:02X}", parse_num::<u8>(bytes, i));
            }
            WORD_PREFIX => {
                debugln!("        {:04X}", parse_num::<u16>(bytes, i));
            }
            DWORD_PREFIX => {
                debugln!("        {:08X}", parse_num::<u32>(bytes, i));
            }
            STRING_PREFIX => {
                debugln!("        {}", parse_string(bytes, i));
            }
            QWORD_PREFIX => {
                debugln!("        {:016X}", parse_num::<u64>(bytes, i));
            }
            NAME_OP => {
                debugln!("        Name({})", parse_string(bytes, i));
            }
            METHOD_OP => {
                let end = *i + parse_length(bytes, i);
                let name = parse_name(bytes, i);
                let flags = parse_num::<u8>(bytes, i);

                debugln!("        Method ({}, {})", name, flags);
                debugln!("        {{");
                debugln!("        }}");

                *i = end;
            }
            BUFFER_OP => {
                let end = *i + parse_length(bytes, i);

                let count = parse_int(bytes, i);

                debugln!("        Buffer ({})", count);

                *i = end;
            }
            PACKAGE_OP => {
                parse_package(bytes, i);
            }
            EXT_OP_PREFIX => {
                if *i < bytes.len() {
                    let ext_op = bytes[*i];
                    *i += 1;

                    match ext_op {
                        OP_REGION_OP => {
                            let name = parse_name(bytes, i);
                            let space = parse_num::<u8>(bytes, i);
                            let offset = parse_int(bytes, i);
                            let size = parse_int(bytes, i);

                            debugln!("        OperationRegion ({}, {}, {}, {})",
                                     name,
                                     space,
                                     offset,
                                     size);
                        }
                        FIELD_OP => {
                            let end = *i + parse_length(bytes, i);

                            let name = parse_name(bytes, i);
                            let flags = parse_num::<u8>(bytes, i);

                            debugln!("        Field ({}, {})", name, flags);
                            debugln!("        {{");
                            while *i < bytes.len() && *i < end {
                                let name = parse_name(bytes, i);
                                let length = parse_length(bytes, i);

                                debugln!("            {}, {}", name, length);
                            }
                            debugln!("        }}");

                            *i = end;
                        }
                        _ => debugln!("        Unknown EXT: {:02X}", ext_op),
                    }
                }
            }
            _ => {
                debugln!("        parse_device: unknown: {:02X}", op);
                break;
            }
        }
    }
    debugln!("    }}");

    *i = end;
}

pub fn parse_scope(bytes: &[u8], i: &mut usize) {
    let end = *i + parse_length(bytes, i);
    let name = parse_name(bytes, i);

    debugln!("Scope ({})", name);
    debugln!("{{");
    while *i < bytes.len() && *i < end {
        let op = bytes[*i];
        *i += 1;

        match op {
            ZERO_OP => {
                debugln!("    Zero");
            }
            ONE_OP => {
                debugln!("    One");
            }
            BYTE_PREFIX => {
                debugln!("    {:02X}", parse_num::<u8>(bytes, i));
            }
            WORD_PREFIX => {
                debugln!("    {:04X}", parse_num::<u16>(bytes, i));
            }
            DWORD_PREFIX => {
                debugln!("    {:08X}", parse_num::<u32>(bytes, i));
            }
            STRING_PREFIX => {
                debugln!("    {}", parse_string(bytes, i));
            }
            QWORD_PREFIX => {
                debugln!("    {:016X}", parse_num::<u64>(bytes, i));
            }
            SCOPE_OP => {
                parse_scope(bytes, i);
            }
            NAME_OP => {
                debugln!("    Name({})", parse_string(bytes, i));
            }
            METHOD_OP => {
                let end = *i + parse_length(bytes, i);
                let name = parse_name(bytes, i);
                let flags = parse_num::<u8>(bytes, i);

                debugln!("    Method ({}, {})", name, flags);
                debugln!("    {{");
                debugln!("    }}");

                *i = end;
            }
            BUFFER_OP => {
                let end = *i + parse_length(bytes, i);

                let count = parse_int(bytes, i);

                debugln!("    Buffer ({})", count);

                *i = end;
            }
            PACKAGE_OP => {
                parse_package(bytes, i);
            }
            EXT_OP_PREFIX => {
                if *i < bytes.len() {
                    let ext_op = bytes[*i];
                    *i += 1;

                    match ext_op {
                        MUTEX_OP => {
                            let name = parse_name(bytes, i);
                            let flags = parse_num::<u8>(bytes, i);

                            debugln!("    Mutex ({}, {})", name, flags);
                        }
                        OP_REGION_OP => {
                            let name = parse_name(bytes, i);
                            let space = parse_num::<u8>(bytes, i);
                            let offset = parse_int(bytes, i);
                            let size = parse_int(bytes, i);

                            debugln!("    OperationRegion ({}, {}, {}, {})",
                                     name,
                                     space,
                                     offset,
                                     size);
                        }
                        FIELD_OP => {
                            let end = *i + parse_length(bytes, i);

                            let name = parse_name(bytes, i);
                            let flags = parse_num::<u8>(bytes, i);

                            debugln!("    Field ({}, {})", name, flags);
                            debugln!("    {{");
                            while *i < bytes.len() && *i < end {
                                let name = parse_name(bytes, i);
                                let length = parse_length(bytes, i);

                                debugln!("        {}, {}", name, length);
                            }
                            debugln!("    }}");

                            *i = end;
                        }
                        DEVICE_OP => {
                            parse_device(bytes, i);
                        }
                        PROCESSOR_OP => {
                            let end = *i + parse_length(bytes, i);

                            let name = parse_name(bytes, i);
                            // let id = parse_num::<u8>(bytes, i);
                            // let blk = parse_num::<u32>(bytes, i);
                            // let blklen = parse_num::<u8>(bytes, i);
                            //

                            debugln!("    Processor ({})", name);

                            *i = end;
                        }
                        _ => debugln!("    Unknown EXT: {:02X}", ext_op),
                    }
                }
            }
            _ => {
                debugln!("    parse_scope: unknown: {:02X}", op);
                break;
            }
        }
    }
    debugln!("}}");

    *i = end;
}

pub fn parse(bytes: &[u8]) {
    let mut i = 0;
    while i < bytes.len() {
        let op = bytes[i];
        i += 1;

        match op {
            SCOPE_OP => {
                parse_scope(bytes, &mut i);
            }
            _ => {
                debugln!("parse: unknown: {:02X}", op);
                break;
            }
        }
    }
}
