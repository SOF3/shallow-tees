#![warn(
    unused_results,
    unused_qualifications,
    variant_size_differences,
    clippy::checked_conversions,
    clippy::needless_borrow,
    clippy::shadow_unrelated,
    clippy::wrong_pub_self_convention
)]
#![deny(
    anonymous_parameters,
    bare_trait_objects,
    clippy::as_conversions,
    clippy::clone_on_ref_ptr,
    clippy::float_cmp_const,
    clippy::if_not_else,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]
#![cfg_attr(
    debug_assertions,
    allow(
        dead_code,
        unused_imports,
        unused_variables,
        unreachable_code,
        unused_qualifications,
    )
)]
#![cfg_attr(not(debug_assertions), deny(warnings, missing_docs, clippy::dbg_macro))]

//! A `tee` implementation with Seek support through counting the max read.

use std::convert::{TryFrom, TryInto};
use std::io::{self, Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};

/// A tee-reader implementation that implements Seek only on the shallow level,
/// i.e. the Write still receives all the bytes skipped in the seek.
pub struct ShallowTees<R: Read + Seek, W: Write> {
    read: R,
    write: W,
    /// current offset of R
    cur: u64,
    /// maximum offset read from R
    max: u64,
}

impl<R: Read + Seek, W: Write> ShallowTees<R, W> {
    /// Creates a ShallowTees
    pub fn new(read: R, write: W) -> Self {
        Self {
            read,
            write,
            cur: 0,
            max: 0,
        }
    }
}

impl<R: Read + Seek, W: Write> Read for ShallowTees<R, W> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        debug_assert!(
            self.cur <= self.max,
            "ShallowTees started at invalid state: {} > {}",
            self.cur,
            self.max
        );
        let size = self.read.read(buf)?;
        if size == 0 {
            return Ok(0);
        }
        self.cur += u64::try_from(size).expect("usize <= u64");
        if self.max < self.cur {
            let delta = usize::try_from(self.cur - self.max).expect("0 < delta <= size < usize");
            debug_assert!(delta <= size, "self.cur <= self.max but delta > size");
            self.write.write_all(
                buf.get((size - delta)..size)
                    .expect("delta <= size <= buf.len()"),
            )?;
        }
        Ok(size)
    }
}

impl<R: Read + Seek, W: Write> Seek for ShallowTees<R, W> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let dest: u64 = match pos {
            SeekFrom::Start(pos) => pos,
            SeekFrom::Current(pos) => {
                let cur: i64 = self.cur.try_into().map_err(|_| err_ui64())?;
                u64::try_from(cur + pos).map_err(|_| err_iu64())?
            }
            SeekFrom::End(_) => panic!("SeekFrom::End() is not supported"),
        };

        if dest > self.max {
            let _ = self.read.seek(SeekFrom::Start(self.max))?;
            self.cur = self.max;
            let size = dest - self.max;
            let written = io::copy(&mut (&mut self.read).take(size), &mut self.write)?;
            self.cur += written;
            if self.cur != dest {
                return Err(Error::new(ErrorKind::UnexpectedEof, "seek behind EOF"));
            }
            self.max = dest;
        } else {
            let _ = self.read.seek(SeekFrom::Start(dest))?;
            self.cur = dest;
        }

        Ok(self.cur)
    }
}

fn err_ui64() -> Error {
    Error::new(ErrorKind::Other, "offset change is greater than 2^63")
}

fn err_iu64() -> Error {
    Error::new(ErrorKind::Other, "resultant offset is negative")
}

#[cfg(test)]
mod tests;
