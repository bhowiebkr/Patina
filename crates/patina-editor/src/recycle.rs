//! Moves a project folder to the Recycle Bin of Windows, so that the user can get it back (15.10).

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};

const FO_DELETE: u32 = 3;
const FOF_SILENT: u16 = 0x4;
const FOF_NOCONFIRMATION: u16 = 0x10;
/// Moves the files to the Recycle Bin.
const FOF_ALLOWUNDO: u16 = 0x40;
const FOF_NOERRORUI: u16 = 0x400;
/// Asks the user when a file cannot go to the Recycle Bin, so no file goes without a question.
const FOF_WANTNUKEWARNING: u16 = 0x4000;

/// `SHFILEOPSTRUCTW` with the 64-bit layout. shellapi.h packs it to 1 byte only on 32-bit.
#[repr(C)]
struct FileOp {
    hwnd: *mut c_void,
    func: u32,
    from: *const u16,
    to: *const u16,
    flags: u16,
    aborted: i32,
    name_mappings: *mut c_void,
    title: *const u16,
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHFileOperationW(op: *mut FileOp) -> i32;
}

/// Moves the folder `path` to the Recycle Bin. The path must be absolute.
pub fn recycle(path: &Path) -> Result<(), String> {
    // The path list ends with two zeros.
    let from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
    let flags = FOF_SILENT | FOF_NOCONFIRMATION | FOF_ALLOWUNDO | FOF_NOERRORUI | FOF_WANTNUKEWARNING;
    let mut op = FileOp { hwnd: null_mut(), func: FO_DELETE, from: from.as_ptr(), to: null(), flags, aborted: 0, name_mappings: null_mut(), title: null() };
    // SAFETY: `op` is a valid SHFILEOPSTRUCTW, and `from` is a double-zero list that lives until the call returns.
    let code = unsafe { SHFileOperationW(&mut op) };
    if code != 0 {
        return Err(format!("Windows cannot move {} to the Recycle Bin (code {code:#x}). Close the programs that use the folder.", path.display()));
    }
    if op.aborted != 0 {
        return Err(format!("The delete of {} stopped.", path.display()));
    }
    Ok(())
}
