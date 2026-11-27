//! Small filesystem boundary for opt-in configuration and skill files.
//! Unix traverses directories with pinned handles and never follows symlinks.
use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Write},
    path::{Component, Path},
    sync::atomic::{AtomicU64, Ordering},
};
use traceveil::{Error, Result};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn unsafe_path() -> Error {
    Error::new(
        "DESTINATION_UNSAFE",
        "Use a regular file in a directory without symlinks or parent traversal.",
    )
}
fn bounded(mut file: File, limit: usize) -> Result<Vec<u8>> {
    if !file.metadata().map_err(traceveil::io_error)?.is_file() {
        return Err(unsafe_path());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(traceveil::io_error)?;
    if bytes.len() > limit {
        return Err(Error::new(
            "INPUT_TOO_LARGE",
            "A configuration or skill file exceeds its documented size limit.",
        ));
    }
    Ok(bytes)
}
#[cfg(unix)]
mod native {
    use super::*;
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };
    pub struct Directory {
        file: File,
    }
    fn name(value: &OsStr) -> Result<CString> {
        if Path::new(value).components().count() != 1
            || !matches!(
                Path::new(value).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err(unsafe_path());
        }
        CString::new(value.as_bytes()).map_err(|_| unsafe_path())
    }
    impl Directory {
        pub fn open(path: &Path, create: bool) -> Result<Option<Self>> {
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(traceveil::io_error)?
                    .join(path)
            };
            let mut file = File::open("/").map_err(traceveil::io_error)?;
            for part in path.components() {
                let value = match part {
                    Component::RootDir | Component::CurDir => continue,
                    Component::Normal(value) => name(value)?,
                    _ => return Err(unsafe_path()),
                };
                if create {
                    let status = unsafe { libc::mkdirat(file.as_raw_fd(), value.as_ptr(), 0o700) };
                    if status != 0
                        && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                    {
                        return Err(traceveil::io_error(std::io::Error::last_os_error()));
                    }
                }
                let fd = unsafe {
                    libc::openat(
                        file.as_raw_fd(),
                        value.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    let e = std::io::Error::last_os_error();
                    return if e.kind() == std::io::ErrorKind::NotFound {
                        Ok(None)
                    } else if [Some(libc::ELOOP), Some(libc::ENOTDIR)].contains(&e.raw_os_error()) {
                        Err(unsafe_path())
                    } else {
                        Err(traceveil::io_error(e))
                    };
                }
                file = unsafe { File::from_raw_fd(fd) };
            }
            Ok(Some(Self { file }))
        }
        fn metadata(&self, value: &OsStr) -> Result<bool> {
            let value = name(value)?;
            let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
            let result = unsafe {
                libc::fstatat(
                    self.file.as_raw_fd(),
                    value.as_ptr(),
                    stat.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if result != 0 {
                let e = std::io::Error::last_os_error();
                return if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(traceveil::io_error(e))
                };
            }
            let stat = unsafe { stat.assume_init() };
            if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
                return Err(unsafe_path());
            }
            Ok(true)
        }
        pub fn read(&self, value: impl AsRef<OsStr>, limit: usize) -> Result<Option<Vec<u8>>> {
            let value = value.as_ref();
            if !self.metadata(value)? {
                return Ok(None);
            }
            let value = name(value)?;
            let fd = unsafe {
                libc::openat(
                    self.file.as_raw_fd(),
                    value.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(unsafe_path());
            }
            bounded(unsafe { File::from_raw_fd(fd) }, limit).map(Some)
        }
        pub fn write(&self, value: impl AsRef<OsStr>, bytes: &[u8]) -> Result<()> {
            let value = value.as_ref();
            self.metadata(value)?;
            let target = name(value)?;
            let (temp, mut file) = loop {
                let temp = name(OsStr::new(&format!(
                    ".traceveil-{}-{}.tmp",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )))?;
                let fd = unsafe {
                    libc::openat(
                        self.file.as_raw_fd(),
                        temp.as_ptr(),
                        libc::O_WRONLY
                            | libc::O_CREAT
                            | libc::O_EXCL
                            | libc::O_NOFOLLOW
                            | libc::O_CLOEXEC,
                        0o600,
                    )
                };
                if fd >= 0 {
                    break (temp, unsafe { File::from_raw_fd(fd) });
                }
                let e = std::io::Error::last_os_error();
                if e.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(traceveil::io_error(e));
                }
            };
            let result = (|| {
                file.write_all(bytes).map_err(traceveil::io_error)?;
                file.sync_all().map_err(traceveil::io_error)?;
                if unsafe {
                    libc::renameat(
                        self.file.as_raw_fd(),
                        temp.as_ptr(),
                        self.file.as_raw_fd(),
                        target.as_ptr(),
                    )
                } != 0
                {
                    return Err(traceveil::io_error(std::io::Error::last_os_error()));
                }
                Ok(())
            })();
            if result.is_err() {
                unsafe { libc::unlinkat(self.file.as_raw_fd(), temp.as_ptr(), 0) };
            }
            result
        }
        pub fn remove_empty(path: &Path) -> Result<bool> {
            let Some(dir) = Self::open(path.parent().ok_or_else(Error::input)?, false)? else {
                return Ok(true);
            };
            let value = name(path.file_name().ok_or_else(Error::input)?)?;
            Ok(
                unsafe { libc::unlinkat(dir.file.as_raw_fd(), value.as_ptr(), libc::AT_REMOVEDIR) }
                    == 0,
            )
        }
        pub fn remove(&self, value: impl AsRef<OsStr>) -> Result<()> {
            if !self.metadata(value.as_ref())? {
                return Err(Error::new(
                    "SKILL_NOT_OWNED",
                    "The installed file is missing.",
                ));
            }
            let value = name(value.as_ref())?;
            if unsafe { libc::unlinkat(self.file.as_raw_fd(), value.as_ptr(), 0) } != 0 {
                return Err(traceveil::io_error(std::io::Error::last_os_error()));
            }
            Ok(())
        }
    }
}
#[cfg(not(unix))]
mod native {
    use super::*;
    use std::{fs, path::PathBuf};
    pub struct Directory {
        path: PathBuf,
    }
    fn check(path: &Path) -> Result<()> {
        for p in path.ancestors() {
            if let Ok(m) = fs::symlink_metadata(p) {
                if m.file_type().is_symlink() {
                    return Err(unsafe_path());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err(unsafe_path());
                    }
                }
            }
        }
        Ok(())
    }
    impl Directory {
        pub fn open(path: &Path, create: bool) -> Result<Option<Self>> {
            if path.components().any(|p| matches!(p, Component::ParentDir)) {
                return Err(unsafe_path());
            }
            check(path)?;
            if create {
                fs::create_dir_all(path).map_err(traceveil::io_error)?;
            }
            if !path.exists() {
                return Ok(None);
            }
            if !path.is_dir() {
                return Err(unsafe_path());
            }
            Ok(Some(Self { path: path.into() }))
        }
        fn target(&self, value: &OsStr) -> Result<PathBuf> {
            if Path::new(value).components().count() != 1
                || !matches!(
                    Path::new(value).components().next(),
                    Some(Component::Normal(_))
                )
            {
                return Err(unsafe_path());
            }
            check(&self.path)?;
            let p = self.path.join(value);
            check(&p)?;
            if fs::symlink_metadata(&p).is_ok_and(|m| !m.is_file()) {
                return Err(unsafe_path());
            }
            Ok(p)
        }
        pub fn read(&self, value: impl AsRef<OsStr>, limit: usize) -> Result<Option<Vec<u8>>> {
            let path = self.target(value.as_ref())?;
            match File::open(path) {
                Ok(f) => bounded(f, limit).map(Some),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(traceveil::io_error(e)),
            }
        }
        pub fn write(&self, value: impl AsRef<OsStr>, bytes: &[u8]) -> Result<()> {
            let target = self.target(value.as_ref())?;
            let (temp, mut file) = loop {
                let p = self.path.join(format!(
                    ".traceveil-{}-{}.tmp",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::OpenOptions::new().write(true).create_new(true).open(&p) {
                    Ok(f) => break (p, f),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(traceveil::io_error(e)),
                }
            };
            let result = (|| {
                file.write_all(bytes).map_err(traceveil::io_error)?;
                file.sync_all().map_err(traceveil::io_error)?;
                drop(file);
                check(&self.path)?;
                fs::rename(&temp, &target).map_err(traceveil::io_error)
            })();
            if result.is_err() {
                let _ = fs::remove_file(temp);
            }
            result
        }
        pub fn remove_empty(path: &Path) -> Result<bool> {
            check(path)?;
            Ok(fs::remove_dir(path).is_ok())
        }
        pub fn remove(&self, value: impl AsRef<OsStr>) -> Result<()> {
            fs::remove_file(self.target(value.as_ref())?).map_err(traceveil::io_error)
        }
    }
}
pub use native::Directory;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};
    #[test]
    fn pinned_directory_writes_survive_parent_rebinding_and_preserve_foreign_temporary_files() {
        let root = std::env::temp_dir().join(format!("traceveil-pinned-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let path = root.join("scope");
        fs::create_dir(&path).unwrap();
        let dir = Directory::open(&path, false).unwrap().unwrap();
        let temp = format!(
            ".traceveil-{}-{}.tmp",
            std::process::id(),
            NEXT.load(Ordering::Relaxed)
        );
        fs::write(path.join(&temp), "foreign").unwrap();
        dir.write("config.json", b"first").unwrap();
        assert_eq!(fs::read_to_string(path.join(&temp)).unwrap(), "foreign");
        let moved = root.join("moved");
        fs::rename(&path, &moved).unwrap();
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, &path).unwrap();
        dir.write("config.json", b"second").unwrap();
        assert_eq!(fs::read(moved.join("config.json")).unwrap(), b"second");
        assert!(!outside.join("config.json").exists());
        assert!(Directory::open(&path, true).is_err());
        drop(dir);
        fs::remove_dir_all(root).unwrap();
    }
}
