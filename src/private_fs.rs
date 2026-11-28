use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> io::Result<Self> {
        Self::in_dir(&std::env::temp_dir())
    }

    fn in_dir(parent: &Path) -> io::Result<Self> {
        for _ in 0..16 {
            let path = parent.join(format!("khana-{}", crate::app::new_token()));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "cannot create private scratch directory"))
    }

    pub fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        scrub_dir_files(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Zero-overwrites a regular file and syncs before deleting to limit forensic remnants.
pub fn wipe_and_remove(path: &Path) {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_file() && meta.len() > 0 {
            if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(path) {
                let buf = [0u8; 4096];
                let mut left = meta.len();
                while left > 0 {
                    let take = (left as usize).min(buf.len());
                    if file.write_all(&buf[..take]).is_err() {
                        break;
                    }
                    left -= take as u64;
                }
                let _ = file.sync_all();
            }
        }
    }
    let _ = std::fs::remove_file(path);
}

fn scrub_dir_files(root: &Path) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if let Ok(meta) = std::fs::symlink_metadata(&p) {
            if meta.file_type().is_dir() {
                scrub_dir_files(&p);
            } else if meta.file_type().is_file() {
                wipe_and_remove(&p);
            }
        }
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    let tmp = TempDir::in_dir(parent)?;
    let source = tmp.join("data");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&source)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(source, path)
}

