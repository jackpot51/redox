use alloc::boxed::Box;

use core::{cmp, mem};
// use core::simd::*;

use common::memory;

use sync::Intex;

use super::FONT;
use super::color::Color;
use super::point::Point;
use super::size::Size;

/// The info of the VBE mode
#[derive(Copy, Clone)]
#[repr(packed)]
pub struct VBEModeInfo {
    attributes: u16,
    win_a: u8,
    win_b: u8,
    granularity: u16,
    winsize: u16,
    segment_a: u16,
    segment_b: u16,
    winfuncptr: u32,
    bytesperscanline: u16,
    pub xresolution: u16,
    pub yresolution: u16,
    xcharsize: u8,
    ycharsize: u8,
    numberofplanes: u8,
    bitsperpixel: u8,
    numberofbanks: u8,
    memorymodel: u8,
    banksize: u8,
    numberofimagepages: u8,
    unused: u8,
    redmasksize: u8,
    redfieldposition: u8,
    greenmasksize: u8,
    greenfieldposition: u8,
    bluemasksize: u8,
    bluefieldposition: u8,
    rsvdmasksize: u8,
    rsvdfieldposition: u8,
    directcolormodeinfo: u8,
    physbaseptr: u32,
    offscreenmemoryoffset: u32,
    offscreenmemsize: u16,
}

pub const VBEMODEINFO: *const VBEModeInfo = 0x5200 as *const VBEModeInfo;

/// A display
pub struct Display {
    pub offscreen: usize,
    pub onscreen: usize,
    pub size: usize,
    pub bytesperrow: usize,
    pub width: usize,
    pub height: usize,
    pub root: bool,
}

impl Display {
    pub unsafe fn root() -> Box<Self> {
        let mode_info = &*VBEMODEINFO;

        let ret = box Display {
            offscreen: memory::alloc(mode_info.bytesperscanline as usize *
                                     mode_info.yresolution as usize),
            onscreen: mode_info.physbaseptr as usize,
            size: mode_info.bytesperscanline as usize * mode_info.yresolution as usize,
            bytesperrow: mode_info.bytesperscanline as usize,
            width: mode_info.xresolution as usize,
            height: mode_info.yresolution as usize,
            root: true,
        };

        ret.set(Color::new(0, 0, 0));
        ret.flip();

        ret
    }

    /// Create a new display
    pub fn new(width: usize, height: usize) -> Box<Self> {
        unsafe {
            let bytesperrow = width * 4;
            let memory_size = bytesperrow * height;

            let ret = box Display {
                offscreen: memory::alloc(memory_size),
                onscreen: memory::alloc(memory_size),
                size: memory_size,
                bytesperrow: bytesperrow,
                width: width,
                height: height,
                root: false,
            };

            ret.set(Color::new(0, 0, 0));
            ret.flip();

            ret
        }
    }

    // Optimized {
    pub unsafe fn set_run(data: u32, dst: usize, len: usize) {
        let mut i = 0;
        // Only use 16 byte transfer if possible
        // if len - (dst + i) % 16 >= mem::size_of::<u32x4>() {
        // Align 16
        // while (dst + i) % 16 != 0 && len - i >= mem::size_of::<u32>() {
        // ((dst + i) as *mut u32) = data;
        // i += mem::size_of::<u32>();
        // }
        // While 16 byte transfers
        // let simd: u32x4 = u32x4(data, data, data, data);
        // while len - i >= mem::size_of::<u32x4>() {
        // ((dst + i) as *mut u32x4) = simd;
        // i += mem::size_of::<u32x4>();
        // }
        // }
        //
        // Everything after last 16 byte transfer
        while len - i >= mem::size_of::<u32>() {
            *((dst + i) as *mut u32) = data;
            i += mem::size_of::<u32>();
        }
    }

    pub unsafe fn copy_run(src: usize, dst: usize, len: usize) {
        let mut i = 0;
        // Only use 16 byte transfer if possible
        // if (src + i) % 16 == (dst + i) % 16 {
        // Align 16
        // while (dst + i) % 16 != 0 && len - i >= mem::size_of::<u32>() {
        // ((dst + i) as *mut u32) = *((src + i) as *const u32);
        // i += mem::size_of::<u32>();
        // }
        // While 16 byte transfers
        // while len - i >= mem::size_of::<u32x4>() {
        // ((dst + i) as *mut u32x4) = *((src + i) as *const u32x4);
        // i += mem::size_of::<u32x4>();
        // }
        // }
        //
        // Everything after last 16 byte transfer
        while len - i >= mem::size_of::<u32>() {
            *((dst + i) as *mut u32) = *((src + i) as *const u32);
            i += mem::size_of::<u32>();
        }
    }

    /// Set the color
    pub fn set(&self, color: Color) {
        unsafe {
            Display::set_run(color.data, self.offscreen, self.size);
        }
    }

    /// Scroll the display
    pub fn scroll(&self, rows: usize) {
        if rows > 0 && rows < self.height {
            let offset = rows * self.bytesperrow;
            unsafe {
                Display::copy_run(self.offscreen + offset, self.offscreen, self.size - offset);
                Display::set_run(0, self.offscreen + self.size - offset, offset);
            }
        }
    }

    /// Flip the display
    pub fn flip(&self) {
        unsafe {
            let _intex = Intex::static_lock();
            if self.root {
                Display::copy_run(self.offscreen, self.onscreen, self.size);
            } else {
                let self_mut: *mut Self = mem::transmute(self);
                mem::swap(&mut (*self_mut).offscreen, &mut (*self_mut).onscreen);
            }
        }
    }

    /// Draw a rectangle
    pub fn rect(&self, point: Point, size: Size, color: Color) {
        let data = color.data;
        let alpha = (color.data & 0xFF000000) >> 24;

        if alpha > 0 {
            let start_y = cmp::max(0, cmp::min(self.height as isize - 1, point.y)) as usize;
            let end_y = cmp::max(0,
                                 cmp::min(self.height as isize - 1,
                                          point.y +
                                          size.height as isize)) as usize;

            let start_x = cmp::max(0, cmp::min(self.width as isize - 1, point.x)) as usize * 4;
            let len = cmp::max(0,
                               cmp::min(self.width as isize - 1,
                                        point.x +
                                        size.width as isize)) as usize * 4 -
                      start_x;

            if alpha >= 255 {
                for y in start_y..end_y {
                    unsafe {
                        Display::set_run(data,
                                         self.offscreen + y * self.bytesperrow + start_x,
                                         len);
                    }
                }
            } else {
                let n_alpha = 255 - alpha;
                let r = (((data >> 16) & 0xFF) * alpha) >> 8;
                let g = (((data >> 8) & 0xFF) * alpha) >> 8;
                let b = ((data & 0xFF) * alpha) >> 8;
                let premul = (r << 16) | (g << 8) | b;
                for y in start_y..end_y {
                    unsafe {
                        Display::set_run_alpha(premul,
                                               n_alpha,
                                               self.offscreen + y * self.bytesperrow + start_x,
                                               len);
                    }
                }
            }
        }
    }

    /// Set the color of a pixel
    pub fn pixel(&self, point: Point, color: Color) {
        unsafe {
            if point.x >= 0 && point.x < self.width as isize && point.y >= 0 &&
               point.y < self.height as isize {
                *((self.offscreen + point.y as usize * self.bytesperrow +
                   point.x as usize * 4) as *mut u32) = color.data;
            }
        }
    }

    // TODO: SIMD to optimize
    pub unsafe fn set_run_alpha(premul: u32, n_alpha: u32, dst: usize, len: usize) {
        let mut i = 0;
        while len - i >= mem::size_of::<u32>() {
            let orig = *((dst + i) as *const u32);
            let r = (((orig >> 16) & 0xFF) * n_alpha) >> 8;
            let g = (((orig >> 8) & 0xFF) * n_alpha) >> 8;
            let b = ((orig & 0xFF) * n_alpha) >> 8;
            *((dst + i) as *mut u32) = ((r << 16) | (g << 8) | b) + premul;
            i += mem::size_of::<u32>();
        }
    }

    // TODO: SIMD to optimize
    pub unsafe fn copy_run_alpha(src: usize, dst: usize, len: usize) {
        let mut i = 0;
        while len - i >= mem::size_of::<u32>() {
            let new = *((src + i) as *const u32);
            let alpha = (new >> 24) & 0xFF;
            if alpha > 0 {
                if alpha >= 255 {
                    *((dst + i) as *mut u32) = new;
                } else {
                    let n_r = (((new >> 16) & 0xFF) * alpha) >> 8;
                    let n_g = (((new >> 8) & 0xFF) * alpha) >> 8;
                    let n_b = ((new & 0xFF) * alpha) >> 8;

                    let orig = *((dst + i) as *const u32);
                    let n_alpha = 255 - alpha;
                    let o_r = (((orig >> 16) & 0xFF) * n_alpha) >> 8;
                    let o_g = (((orig >> 8) & 0xFF) * n_alpha) >> 8;
                    let o_b = ((orig & 0xFF) * n_alpha) >> 8;

                    *((dst + i) as *mut u32) = ((o_r << 16) | (o_g << 8) | o_b) +
                                               ((n_r << 16) | (n_g << 8) | n_b);
                }
            }
            i += mem::size_of::<u32>();
        }
    }

    /// Draw a char
    pub fn char(&self, point: Point, character: char, color: Color) {
        let font_i = 16 * (character as usize);
        for row in 0..16 {
            let row_data = FONT[font_i + row];
            for col in 0..8 {
                let pixel = (row_data >> (7 - col)) & 1;
                if pixel > 0 {
                    self.pixel(Point::new(point.x + col, point.y + row as isize), color);
                }
            }
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            if self.offscreen > 0 {
                memory::unalloc(self.offscreen);
                self.offscreen = 0;
            }
            if !self.root && self.onscreen > 0 {
                memory::unalloc(self.onscreen);
                self.onscreen = 0;
            }
            self.size = 0;
            self.bytesperrow = 0;
            self.width = 0;
            self.height = 0;
            self.root = false;
        }
    }
}
