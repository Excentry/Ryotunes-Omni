use std::io::{self, Write};
use std::path::Path;

pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(contents)?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write;

    #[test]
    fn replacement_keeps_the_target_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, b"old").unwrap();

        write(&path, b"new complete value").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new complete value");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
