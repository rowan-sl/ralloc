use std::{path::Path, io::{self, Seek}, fs::OpenOptions, ops::DerefMut};

use memmap2::{MmapOptions, MmapMut};

use super::Backing;


/// helper function for creating a memmap for this use case
///
/// # Saftey
///
/// from MmapOptions::new:
/// > Safety: All file-backed memory map constructors are marked unsafe because of the potential for Undefined Behavior (UB)
/// using the map if the underlying file is subsequently modified, in or out of process. Applications must consider the risk and
/// take appropriate precautions when using file-backed maps. Solutions such as file permissions, locks or process-private (e.g. unlinked)
/// files exist but are platform specific and limited.
///
#[must_use]
#[forbid(unsafe_op_in_unsafe_fn)]
pub unsafe fn new_map<P: AsRef<Path>>(path: P, size: usize) -> Result<MmapMut, io::Error> {
    let mut handle = OpenOptions::new().create(true).read(true).write(true).open(path)?;
    handle.seek(std::io::SeekFrom::Start(0))?;
    handle.set_len(size as u64)?;
    let map = unsafe { MmapOptions::new().len(size).map_mut(&handle)? };
    Ok(map)
}

impl Backing for MmapMut {
    fn get_mem(&mut self) -> &mut [u8] {
        <MmapMut as DerefMut>::deref_mut(self)
    }
}
