#[macro_use]
extern crate criterion;
extern crate rand;
extern crate tempfile;

extern crate positioned_io;

use criterion::{Criterion, Fun};
use std::fs::File;
use std::io::prelude::*;

use rand::{RngCore, SeedableRng};

use positioned_io::{ReadAt, Size, WriteAt};
use std::cell::RefCell;
use std::io;
use std::io::SeekFrom;
use std::sync::Mutex;

#[cfg(windows)]
mod ignored_seek {
    use positioned_io::{ReadAt, Size, WriteAt};
    use std::fs::File;
    use std::io;
    use std::os::windows::fs::FileExt;

    pub struct IgnoreSeek(pub File);

    impl Size for IgnoreSeek {
        fn size(&self) -> io::Result<Option<u64>> {
            self.0.size()
        }
    }

    impl ReadAt for IgnoreSeek {
        fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
            self.0.seek_read(buf, pos)
        }
    }

    impl WriteAt for IgnoreSeek {
        fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
            self.0.seek_write(buf, pos)
        }

        fn flush(&mut self) -> io::Result<()> {
            io::Write::flush(&mut self.0)
        }
    }
}

struct RefCellSeek(RefCell<File>);

impl Size for RefCellSeek {
    fn size(&self) -> io::Result<Option<u64>> {
        self.0.borrow().size()
    }
}

impl ReadAt for RefCellSeek {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        let mut r = self.0.borrow_mut();
        r.seek(SeekFrom::Start(pos))?;
        r.read(buf)
    }
}

impl WriteAt for RefCellSeek {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        let mut r = self.0.borrow_mut();
        r.seek(SeekFrom::Start(pos))?;
        r.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut *self.0.borrow_mut())
    }
}

struct MutexSeek(Mutex<File>);

impl Size for MutexSeek {
    fn size(&self) -> io::Result<Option<u64>> {
        self.0.lock().unwrap().size()
    }
}

impl ReadAt for MutexSeek {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        let mut r = self.0.lock().unwrap();
        r.seek(SeekFrom::Start(pos))?;
        r.read(buf)
    }
}

impl WriteAt for MutexSeek {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        let mut r = self.0.lock().unwrap();
        r.seek(SeekFrom::Start(pos))?;
        r.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut *self.0.lock().unwrap())
    }
}

fn read_at_many<R: ReadAt, Rng: rand::Rng>(r: &R, rng: &mut Rng) -> usize {
    let offset = rng.gen_range(0, 1023 * 1024);
    let mut buf: [u8; 1024] = unsafe { ::std::mem::uninitialized() };
    r.read_at(offset, &mut buf[..]).unwrap()
}

fn read_at_fun<R: ReadAt + 'static, F>(name: &str, f: F) -> Fun<()>
where
    F: FnOnce(File) -> R,
{
    let mut file = tempfile::tempfile().unwrap();

    let mut rng =
        rand::prng::XorShiftRng::from_seed([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let mut buf = [0; 1024];
    for _ in 0..1024 {
        rng.fill_bytes(&mut buf[..]);
        file.write_all(&buf[..]).unwrap();
    }
    let reader = f(file);
    Fun::new(name, move |b, _| b.iter(|| read_at_many(&reader, &mut rng)))
}

fn bench_read_at_random_seek(c: &mut Criterion) {
    let standard = read_at_fun(if cfg!(windows) { "mmap" } else { "pread" }, |file| file);
    let refcell = read_at_fun("refcell", |file| RefCellSeek(RefCell::new(file)));
    let mutex = read_at_fun("mutex", |file| MutexSeek(Mutex::new(file)));

    let mut functions = Vec::new();
    functions.push(standard);
    functions.push(refcell);
    functions.push(mutex);
    #[cfg(windows)]
    {
        let seek_ignore = read_at_fun("seek_read", ignored_seek::IgnoreSeek);
        functions.push(seek_ignore);
    }

    c.bench_functions("random read_at", functions, ());
}

criterion_group!(benches, bench_read_at_random_seek);

criterion_main!(benches);
