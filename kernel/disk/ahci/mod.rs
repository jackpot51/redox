use alloc::boxed::Box;

use collections::vec::Vec;

use disk::Disk;

use drivers::pci::config::PciConfig;

use schemes::Result;

use self::hba::{HbaMem, HbaPort, HbaPortType};

pub mod fis;
pub mod hba;

pub struct Ahci;

impl Ahci {
    pub fn disks(mut pci: PciConfig) -> Vec<Box<Disk>> {
        let base = unsafe { (pci.read(0x24) & 0xFFFFFFF0) as usize };
        let irq = unsafe { (pci.read(0x3C) & 0xF) as u8 };

        debugln!("AHCI on: {:X} IRQ: {:X}", base as usize, irq);

        let pi = unsafe { &mut *(base as *mut HbaMem) }.pi.read();
        let ret: Vec<Box<Disk>> = (0..32)
                                      .filter(|&i| pi & 1 << i as i32 == 1 << i as i32)
                                      .filter_map(|i| {
                                          let mut disk = box AhciDisk::new(base, i);
                                          let port_type = disk.port.probe();
                                          debugln!("Port {}: {:?}", i, port_type);
                                          match port_type {
                                              HbaPortType::SATA => {
                                                  disk.port.init();
                                                  Some(disk as Box<Disk>)
                                              }
                                              _ => None,
                                          }
                                      })
                                      .collect();

        ret
    }
}

pub struct AhciDisk {
    port: &'static mut HbaPort,
}

impl AhciDisk {
    fn new(base: usize, port_index: usize) -> Self {
        AhciDisk { port: &mut unsafe { &mut *(base as *mut HbaMem) }.ports[port_index] }
    }
}

impl Disk for AhciDisk {
    fn read(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        self.port.ata_dma(block, buffer.len() / 512, buffer.as_ptr() as usize, false)
    }

    fn write(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        self.port.ata_dma(block, buffer.len() / 512, buffer.as_ptr() as usize, true)
    }
}
