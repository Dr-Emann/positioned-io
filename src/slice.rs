use std::cmp::min;
use std::io;

use super::{ReadAt, WriteAt, Size};
use std::ops::RangeBounds;
use std::ops::Bound;

/// A window into another `ReadAt` or `WriteAt`.
///
/// Given an existing positioned I/O, this presents a limited view of it.
///
/// # Examples
///
/// Some slices have size restrictions:
///
/// ```rust
/// # use std::io;
/// use positioned_io::{ReadAt, Slice};
///
/// # fn foo() -> io::Result<()> {
/// let a = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
/// let slice = Slice::new(&a[..], 4..8);
///
/// let mut buf = [0; 4];
/// let bytes = slice.read_at(2, &mut buf)?;
/// assert_eq!(bytes, 2);
/// assert_eq!(buf, [6, 7, 0, 0]);
/// # Ok(())
/// # }
/// # fn main() { foo().unwrap(); }
/// ```
///
/// Some slices do not:
///
/// ```rust
/// # use std::io;
/// use positioned_io::{WriteAt, Slice};
///
/// # fn foo() -> io::Result<()> {
/// let mut v = vec![0, 1, 2, 3, 4, 5];
/// let buf = [9; 3];
///
/// {
///     let mut slice = Slice::new(&mut v, 2..);
///     slice.write_all_at(3, &buf)?;
/// }
///
/// // The write goes right past the end.
/// assert_eq!(v, vec![0, 1, 2, 3, 4, 9, 9, 9]);
/// # Ok(())
/// # }
/// # fn main() { foo().unwrap(); }
/// ```
#[derive(Debug, Clone)]
pub struct Slice<I> {
    io: I,
    offset: u64,
    size: u64,
}

impl<I> Slice<I> {
    /// Create a new `Slice`.
    ///
    /// The slice will be a view of `size` bytes, starting at `offset` in `io`.
    /// If you do not pass a size, the size won't be limited.
    pub fn new(io: I, bounds: impl RangeBounds<u64>) -> Self {
        let offset = match bounds.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start.saturating_add(1),
            Bound::Unbounded => 0,
        };
        let size = match bounds.end_bound() {
            Bound::Included(&end) => if end == u64::max_value() { end.saturating_sub(offset).saturating_add(1) } else { (end + 1).saturating_sub(offset) }
            Bound::Excluded(&end) => end.saturating_sub(offset),
            Bound::Unbounded => u64::max_value(),
        };
        Slice {
            io,
            offset,
            size,
        }
    }

    /// Get the available bytes starting at some point.
    fn avail(&self, pos: u64, bytes: usize) -> usize {
        min(self.size.saturating_sub(pos), bytes as u64) as usize
    }
}
impl<I: Size> Slice<I> {
    /// Create a new `Slice` that goes to the end of `io`.
    ///
    /// Note that you can create a larger slice by passing a larger size to
    /// `new()`, but it won't do you any good for reading.
    pub fn new_to_end(io: I, offset: u64) -> io::Result<Self> {
        match io.size() {
            Ok(Some(size)) => Ok(Self::new(io, offset..size)),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "unknown base size")),
        }
    }
}

impl<I: ReadAt> ReadAt for Slice<I> {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = self.avail(pos, buf.len());
        self.io.read_at(pos + self.offset, &mut buf[..bytes])
    }
}

impl<I: WriteAt> WriteAt for Slice<I> {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        let bytes = self.avail(pos, buf.len());
        self.io.write_at(pos + self.offset, &buf[..bytes])
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl<I> Size for Slice<I> {
    fn size(&self) -> io::Result<Option<u64>> {
        if self.size == u64::max_value() {
            Ok(None)
        } else {
            Ok(Some(self.size))
        }
    }
}
