#![allow(clippy::as_conversions, clippy::unwrap_used)]

use std::cell::{Ref, RefCell};
use std::rc::Rc;

use super::*;

struct SlowCursor<T: AsRef<[u8]>> {
    slice: T,
    pos: usize,
}

impl<T: AsRef<[u8]>> Read for SlowCursor<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        if let Some(&byte) = self.slice.as_ref().get(self.pos) {
            buf[0] = byte;
            self.pos += 1;
            Ok(1)
        } else {
            Ok(0)
        }
    }
}

impl<T: AsRef<[u8]>> Seek for SlowCursor<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        // code copied from <std::io::Cursor as Seek>

        match pos {
            SeekFrom::Start(n) => {
                self.pos = n as usize;
            }
            SeekFrom::End(n) => {
                if n > 0 {
                    return Err(Error::new(ErrorKind::UnexpectedEof, "seeking behind EOF"));
                }
                self.pos -= (-n) as usize;
            }
            SeekFrom::Current(n) => {
                self.pos = ((self.pos as isize) + (n as isize))
                    .try_into()
                    .map_err(|_| Error::new(ErrorKind::Other, "seeking before start of file"))?;
            }
        }

        if self.pos > self.slice.as_ref().len() {
            return Err(Error::new(ErrorKind::UnexpectedEof, "seeking behind EOF"));
        }

        Ok(self.pos as u64)
    }
}

#[derive(Clone)]
struct RefWrite(Rc<RefCell<Vec<u8>>>);

impl Write for RefWrite {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.borrow_mut().flush()
    }
}

impl RefWrite {
    fn vec(&self) -> Ref<'_, Vec<u8>> {
        self.0.borrow()
    }
}

#[test]
fn test() {
    let cursor = SlowCursor {
        slice: b"abcdefghijklmnop",
        pos: 0,
    };

    let output = RefWrite(Rc::new(RefCell::new(Vec::new())));
    let mut tee = ShallowTees::new(cursor, output.clone());

    let offset = tee.seek(SeekFrom::Start(5)).unwrap();
    assert_eq!(offset, 5);
    assert_eq!(&output.vec()[..], b"abcde");

    let offset = tee.seek(SeekFrom::Start(6)).unwrap();
    assert_eq!(offset, 6);
    assert_eq!(&output.vec()[..], b"abcdef");

    let offset = tee.seek(SeekFrom::Start(4)).unwrap();
    assert_eq!(offset, 4);
    assert_eq!(&output.vec()[..], b"abcdef");

    let offset = tee.seek(SeekFrom::Current(-1)).unwrap();
    assert_eq!(offset, 3);
    assert_eq!(&output.vec()[..], b"abcdef");

    let offset = tee.seek(SeekFrom::Current(7)).unwrap();
    assert_eq!(offset, 10);
    assert_eq!(&output.vec()[..], b"abcdefghij");

    let offset = tee.seek(SeekFrom::Current(-1)).unwrap();
    assert_eq!(offset, 9);
    assert_eq!(&output.vec()[..], b"abcdefghij");
}
