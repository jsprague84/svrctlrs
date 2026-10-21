//! SFTP for the loopback SSH server: russh-sftp's server side over a real directory, which
//! the client sees as `/` (and as its home, `.`). Paths are resolved lexically inside that
//! directory, so `..` cannot leave it; realpath follows symlinks as OpenSSH does.
//!
//! One difference from a real server, on purpose: a file whose owner-write bit is clear
//! refuses to open for writing even when the tests run as root (CI does), so a permission
//! test means the same on every machine.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode,
};

enum Open {
    File(fs::File),
    /// A directory's entries, handed out by the first `readdir`
    Dir(Option<Vec<File>>),
}

pub struct SftpFs {
    root: PathBuf,
    handles: HashMap<String, Open>,
    next_handle: u64,
}

impl SftpFs {
    pub fn new(root: &Path) -> Self {
        Self {
            root: fs::canonicalize(root).expect("sftp root"),
            handles: HashMap::new(),
            next_handle: 0,
        }
    }

    /// The real path behind a client path, never outside the root.
    fn real(&self, path: &str) -> PathBuf {
        let mut real = self.root.clone();
        let mut depth = 0usize;
        for part in Path::new(path).components() {
            match part {
                Component::Normal(p) => {
                    real.push(p);
                    depth += 1;
                }
                Component::ParentDir if depth > 0 => {
                    real.pop();
                    depth -= 1;
                }
                _ => {}
            }
        }
        real
    }

    /// The client path of a real path inside the root.
    fn virtual_path(&self, real: &Path) -> Option<String> {
        let rest = real.strip_prefix(&self.root).ok()?;
        Some(format!("/{}", rest.to_string_lossy()))
    }

    fn handle(&mut self, open: Open) -> String {
        self.next_handle += 1;
        let h = self.next_handle.to_string();
        self.handles.insert(h.clone(), open);
        h
    }
}

fn status_of(e: &std::io::Error) -> StatusCode {
    match e.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
        std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

fn attrs_of(meta: &fs::Metadata) -> FileAttributes {
    FileAttributes {
        size: Some(meta.len()),
        uid: Some(meta.uid()),
        user: None,
        gid: Some(meta.gid()),
        group: None,
        permissions: Some(meta.mode()),
        atime: Some(meta.atime() as u32),
        mtime: Some(meta.mtime() as u32),
    }
}

impl russh_sftp::server::Handler for SftpFs {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = self.real(&filename);
        if pflags.contains(OpenFlags::WRITE) {
            if let Ok(meta) = fs::metadata(&path) {
                if meta.mode() & 0o200 == 0 {
                    return Err(StatusCode::PermissionDenied);
                }
            }
        }
        let file = fs::OpenOptions::from(pflags)
            .open(&path)
            .map_err(|e| status_of(&e))?;
        Ok(Handle {
            id,
            handle: self.handle(Open::File(file)),
        })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(ok(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let Some(Open::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| status_of(&e))?;
        let mut data = vec![0u8; len as usize];
        let n = file.read(&mut data).map_err(|e| status_of(&e))?;
        if n == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(n);
        Ok(Data { id, data })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let Some(Open::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| status_of(&e))?;
        file.write_all(&data).map_err(|e| status_of(&e))?;
        Ok(ok(id))
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let meta = fs::symlink_metadata(self.real(&path)).map_err(|e| status_of(&e))?;
        Ok(Attrs {
            id,
            attrs: attrs_of(&meta),
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let meta = fs::metadata(self.real(&path)).map_err(|e| status_of(&e))?;
        Ok(Attrs {
            id,
            attrs: attrs_of(&meta),
        })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let Some(Open::File(file)) = self.handles.get(&handle) else {
            return Err(StatusCode::Failure);
        };
        let meta = file.metadata().map_err(|e| status_of(&e))?;
        Ok(Attrs {
            id,
            attrs: attrs_of(&meta),
        })
    }

    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        if let Some(mode) = attrs.permissions {
            fs::set_permissions(self.real(&path), fs::Permissions::from_mode(mode & 0o7777))
                .map_err(|e| status_of(&e))?;
        }
        Ok(ok(id))
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let dir = self.real(&path);
        let mut files = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| status_of(&e))? {
            let entry = entry.map_err(|e| status_of(&e))?;
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| status_of(&e))?;
            files.push(File::new(
                entry.file_name().to_string_lossy().into_owned(),
                attrs_of(&meta),
            ));
        }
        Ok(Handle {
            id,
            handle: self.handle(Open::Dir(Some(files))),
        })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        match self.handles.get_mut(&handle) {
            Some(Open::Dir(files)) => match files.take() {
                Some(files) => Ok(Name { id, files }),
                None => Err(StatusCode::Eof),
            },
            _ => Err(StatusCode::Failure),
        }
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        fs::remove_file(self.real(&filename)).map_err(|e| status_of(&e))?;
        Ok(ok(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        fs::create_dir(self.real(&path)).map_err(|e| status_of(&e))?;
        Ok(ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        fs::remove_dir(self.real(&path)).map_err(|e| status_of(&e))?;
        Ok(ok(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let real = fs::canonicalize(self.real(&path)).map_err(|e| status_of(&e))?;
        let path = self.virtual_path(&real).ok_or(StatusCode::NoSuchFile)?;
        Ok(Name {
            id,
            files: vec![File::dummy(path)],
        })
    }

    /// SFTP v3: a rename never replaces an existing file
    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        let to = self.real(&newpath);
        if fs::symlink_metadata(&to).is_ok() {
            return Err(StatusCode::Failure);
        }
        fs::rename(self.real(&oldpath), to).map_err(|e| status_of(&e))?;
        Ok(ok(id))
    }

    async fn readlink(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let target = fs::read_link(self.real(&path)).map_err(|e| status_of(&e))?;
        Ok(Name {
            id,
            files: vec![File::dummy(target.to_string_lossy())],
        })
    }
}
