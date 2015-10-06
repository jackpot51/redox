use alloc::boxed::Box;

use core::cmp::{min, max};
use core::mem::size_of;
use core::ops::DerefMut;
use core::ptr;

use common::event::*;
use common::string::*;
use common::resource::*;

use graphics::display::*;
use graphics::point::*;
use graphics::size::*;
use graphics::window::*;

use programs::common::*;

use syscall::call::sys_yield;

pub struct WindowScheme;

pub struct WindowResource {
    pub window: Box<Window>,
    pub seek: usize,
}

impl Resource for WindowResource {
     //Required functions
    /// Return the url of this resource
    fn url(&self) -> URL {
        return URL::from_string(&("window://".to_string() + "/" + self.window.point.x + "/" +
                                  self.window.point.y + "/" +
                                  self.window.size.width + "/" +
                                  self.window.size.height +
                                  "/" + &self.window.title));
    }

    /// Return the type of this resource
    fn stat(&self) -> ResourceType {
        return ResourceType::File;
    }

    /// Read data to buffer
    fn read(&mut self, buf: &mut [u8]) -> Option<usize> {
        /* Reading window contents, might be necessary?
        let content = &mut self.window.content;

        let size = min(content.size - self.seek, buf.len());
        unsafe {
            Display::copy_run(content.offscreen + self.seek, buf.as_ptr() as usize, size);
        }
        self.seek += size;

        return Option::Some(size);
        */

        //Read events from window
        let mut i = 0;
        while buf.len() - i >= size_of::<Event>() {
            match self.window.poll() {
                Option::Some(event) => {
                    unsafe { ptr::write(buf.as_ptr().offset(i as isize) as *mut Event, event) };
                    i += size_of::<Event>();
                }
                Option::None => sys_yield(),
            }
        }

        return Option::Some(i);
    }

    /// Write to resource
    fn write(&mut self, buf: &[u8]) -> Option<usize> {
        let content = &mut self.window.content;

        let size = min(content.size - self.seek, buf.len());
        unsafe {
            Display::copy_run(buf.as_ptr() as usize,
                              content.offscreen + self.seek,
                              size);
        }
        self.seek += size;

        return Option::Some(size);
    }

    /// Seek
    fn seek(&mut self, pos: ResourceSeek) -> Option<usize> {
        let end = self.window.content.size;

        self.seek = match pos {
            ResourceSeek::Start(offset) => min(end, max(0, offset)),
            ResourceSeek::Current(offset) => min(end, max(0, self.seek as isize + offset) as usize),
            ResourceSeek::End(offset) => min(end, max(0, end as isize + offset) as usize),
        };

        return Option::Some(self.seek);
    }

    /// Sync the resource, should flip
    fn sync(&mut self) -> bool {
        self.window.redraw();
        return true;
    }
}

impl SessionItem for WindowScheme {
    fn scheme(&self) -> String {
        return "window".to_string();
    }

    fn open(&mut self, url: &URL) -> Box<Resource> {
        //window://host/path/path/path is the path type we're working with.
        let url_path = url.path_parts();
        let pointx = match url_path.get(0) {
            Some(x) => x.to_num_signed(),
            None => 0,
        };
        let pointy = match url_path.get(1) {
            Some(y) => y.to_num_signed(),
            None => 0,
        };
        let size_width = match url_path.get(2) {
            Some(w) => w.to_num(),
            None => 100,
        };
        let size_height = match url_path.get(3) {
            Some(h) => h.to_num(),
            None => 100,
        };

        let mut title = match url_path.get(4) {
            Some(t) => t.clone(),
            None => String::new(),
        };
        for i in 5..url_path.len() {
            if let Some(t) = url_path.get(i) {
                title = title + "/" + t;
            }
        }

        let mut p: Point = Point::new(pointx, pointy);
        let mut s: Size = Size::new(size_width, size_height);

        return box WindowResource {
            window: Window::new(p, s, title),
            seek: 0,
        };
    }
}
