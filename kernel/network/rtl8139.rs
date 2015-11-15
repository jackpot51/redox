use alloc::boxed::Box;

use collections::slice;
use collections::string::ToString;
use collections::vec::Vec;

use core::ptr;

use common::{debug, memory};
use common::queue::Queue;
use schemes::{Resource, Url};
use scheduler;

use drivers::pciconfig::PciConfig;
use drivers::pio::*;

use network::common::*;
use network::scheme::*;

use schemes::KScheme;

#[repr(packed)]
struct Txd {
    pub address_port: u16,
    pub status_port: u16,
    pub buffer: usize,
}

pub struct Rtl8139 {
    pci: PciConfig,
    base: usize,
    memory_mapped: bool,
    irq: u8,
    resources: Vec<*mut NetworkResource>,
    inbound: Queue<Vec<u8>>,
    outbound: Queue<Vec<u8>>,
    txds: Vec<Txd>,
    txd_i: usize,
}

impl Rtl8139 {
    pub fn new(mut pci: PciConfig) -> Box<Self> {
        let base = unsafe { pci.read(0x10) as usize };
        let irq = unsafe { pci.read(0x3C) as u8 & 0xF };

        let mut module = box Rtl8139 {
            pci: pci,
            base: base & 0xFFFFFFF0,
            memory_mapped: base & 1 == 0,
            irq: irq,
            resources: Vec::new(),
            inbound: Queue::new(),
            outbound: Queue::new(),
            txds: Vec::new(),
            txd_i: 0,
        };

        unsafe { module.init() };

        module
    }

    unsafe fn init(&mut self) {
        debug::d("RTL8139 on: ");
        debug::dh(self.base);
        if self.memory_mapped {
            debug::d(" memory mapped");
        } else {
            debug::d(" port mapped");
        }
        debug::d(" IRQ: ");
        debug::dbh(self.irq);

        self.pci.flag(4, 4, true); // Bus mastering

        let base = self.base as u16;

        outb(base + 0x52, 0);

        outb(base + 0x37, 0x10);
        while inb(base + 0x37) & 0x10 != 0 {}

        debug::d(" MAC: ");
        let mac_low = ind(base);
        let mac_high = ind(base + 4);
        MAC_ADDR = MacAddr {
            bytes: [mac_low as u8,
                    (mac_low >> 8) as u8,
                    (mac_low >> 16) as u8,
                    (mac_low >> 24) as u8,
                    mac_high as u8,
                    (mac_high >> 8) as u8],
        };
        debug::d(&MAC_ADDR.to_string());

        let receive_buffer = memory::alloc(10240);
        outd(base + 0x30, receive_buffer as u32);

        for i in 0..4 {
            self.txds.push(Txd {
                address_port: base + 0x20 + (i as u16) * 4,
                status_port: base + 0x10 + (i as u16) * 4,
                buffer: memory::alloc(4096),
            });
        }

        outw(base + 0x3C, 5);
        debug::d(" IMR: ");
        debug::dh(inw(base + 0x3C) as usize);

        outb(base + 0x37, 0xC);
        debug::d(" CMD: ");
        debug::dbh(inb(base + 0x37));

        outd(base + 0x44,
             (1 << 7) | (1 << 4) | (1 << 3) | (1 << 2) | (1 << 1));
        debug::d(" RCR: ");
        debug::dh(ind(base + 0x44) as usize);

        outd(base + 0x40, (0b11 << 24));
        debug::d(" TCR: ");
        debug::dh(ind(base + 0x40) as usize);

        debug::dl();
    }

    unsafe fn receive_inbound(&mut self) {
        let base = self.base as u16;

        let receive_buffer = ind(base + 0x30) as usize;
        let mut capr = (inw(base + 0x38) + 16) as usize;
        let cbr = inw(base + 0x3A) as usize;

        while capr != cbr {
            let frame_addr = receive_buffer + capr + 4;
            let frame_status = ptr::read((receive_buffer + capr) as *const u16) as usize;
            let frame_len = ptr::read((receive_buffer + capr + 2) as *const u16) as usize;

            debug::d("Recv ");
            debug::dh(capr as usize);
            debug::d(" ");
            debug::dh(frame_status);
            debug::d(" ");
            debug::dh(frame_addr);
            debug::d(" ");
            debug::dh(frame_len);
            debug::dl();

            self.inbound
                .push(Vec::from(slice::from_raw_parts(frame_addr as *const u8, frame_len - 4)));

            capr = capr + frame_len + 4;
            capr = (capr + 3) & (0xFFFFFFFF - 3);
            if capr >= 8192 {
                capr -= 8192
            }

            outw(base + 0x38, (capr as u16) - 16);
        }
    }

    unsafe fn send_outbound(&mut self) {
        while let Some(bytes) = self.outbound.pop() {
            if let Some(txd) = self.txds.get(self.txd_i) {
                if bytes.len() < 4096 {
                    let mut tx_status;
                    loop {
                        tx_status = ind(txd.status_port);
                        if tx_status & (1 << 13) == (1 << 13) {
                            break;
                        }
                    }

                    debug::d("Send ");
                    debug::dh(txd.status_port as usize);
                    debug::d(" ");
                    debug::dh(tx_status as usize);
                    debug::d(" ");
                    debug::dh(txd.buffer);
                    debug::d(" ");
                    debug::dh(bytes.len() & 0xFFF);
                    debug::dl();

                    ::memcpy(txd.buffer as *mut u8, bytes.as_ptr(), bytes.len());

                    outd(txd.address_port, txd.buffer as u32);
                    outd(txd.status_port, bytes.len() as u32 & 0xFFF);

                    self.txd_i = (self.txd_i + 1) % 4;
                } else {
                    debug::dl();
                    debug::d("RTL8139: Frame too long for transmit: ");
                    debug::dd(bytes.len());
                    debug::dl();
                }
            } else {
                debug::d("RTL8139: TXD Overflow!\n");
                self.txd_i = 0;
            }
        }
    }
}

impl KScheme for Rtl8139 {
    fn scheme(&self) -> &str {
        "network"
    }

    fn open(&mut self, _: &Url, _: usize) -> Option<Box<Resource>> {
        Some(NetworkResource::new(self))
    }

    fn on_irq(&mut self, irq: u8) {
        if irq == self.irq {
            unsafe {
                let base = self.base as u16;

                let isr = inw(base + 0x3E);
                outw(base + 0x3E, isr);

                // dh(isr as usize);
                // dl();
            }

            self.sync();
        }
    }

    fn on_poll(&mut self) {
        self.sync();
    }
}

impl NetworkScheme for Rtl8139 {
    fn add(&mut self, resource: *mut NetworkResource) {
        unsafe {
            let reenable = scheduler::start_no_ints();
            self.resources.push(resource);
            scheduler::end_no_ints(reenable);
        }
    }

    fn remove(&mut self, resource: *mut NetworkResource) {
        unsafe {
            let reenable = scheduler::start_no_ints();
            let mut i = 0;
            while i < self.resources.len() {
                let mut remove = false;

                match self.resources.get(i) {
                    Some(ptr) => if *ptr == resource {
                        remove = true;
                    } else {
                        i += 1;
                    },
                    None => break,
                }

                if remove {
                    self.resources.remove(i);
                }
            }
            scheduler::end_no_ints(reenable);
        }
    }

    fn sync(&mut self) {
        unsafe {
            let reenable = scheduler::start_no_ints();

            for resource in self.resources.iter() {
                while let Some(bytes) = (**resource).outbound.pop() {
                    self.outbound.push(bytes);
                }
            }

            self.send_outbound();

            self.receive_inbound();

            while let Some(bytes) = self.inbound.pop() {
                for resource in self.resources.iter() {
                    (**resource).inbound.push(bytes.clone());
                }
            }

            scheduler::end_no_ints(reenable);
        }
    }
}
