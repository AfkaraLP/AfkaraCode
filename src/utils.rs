use std::io::{Read, Seek, SeekFrom, Write};

/// # Errors
///
/// - Old string not found in file.
/// - File cannot be opened.
pub fn edit_file(
    path: impl AsRef<str>,
    old: &str,
    new: &str,
) -> Result<&'static str, &'static str> {
    let mut file = std::fs::File::open(path.as_ref()).map_err(|_| "couldn't open file.")?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)
        .map_err(|_| "failed to read file.")?;

    if !file_content.contains(old) {
        return Err("file did not contain old string.");
    }

    let file_content = file_content.replace(old, new);

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path.as_ref())
        .map_err(|_| "failed to reopen file for writing.")?;

    file.write_all(file_content.as_bytes())
        .map_err(|_| "failed to write to file.")?;

    Ok("successfully wrote to file")
}

/// # Errors
///
/// Errors if file is read protected or does not exist.
pub fn read_file(path: String) -> Result<String, &'static str> {
    std::fs::read_to_string(path).map_err(|_| "failed reading file.")
}

/// Read with optional byte range.
/// If offset is Some, seek to that position. If length is Some, read up to length bytes.
pub fn read_file_with_range(path: String, offset: Option<u64>, length: Option<usize>) -> Result<String, &'static str> {
    use std::fs::File;

    let mut file = File::open(path).map_err(|_| "failed reading file.")?;

    if let Some(off) = offset {
        file.seek(SeekFrom::Start(off)).map_err(|_| "failed seeking in file.")?;
    }

    let mut buf = Vec::new();

    match length {
        Some(len) => {
            let mut take = file.take(len as u64);
            take.read_to_end(&mut buf).map_err(|_| "failed reading file.")?;
        }
        None => {
            file.read_to_end(&mut buf).map_err(|_| "failed reading file.")?;
        }
    }

    String::from_utf8(buf).map_err(|_| "failed reading file.")
}

/// # Errors
///
/// Can error if the directory we're trying to create the file in is write protected.
pub fn create_file(path: String, content: String) -> Result<String, &'static str> {
    std::fs::write(path, content).map_err(|_| "failed to create file")?;
    Ok("successfully created file".to_string())
}

/// # Errors
///
/// Can error if the directory cannot be read from or does not exist.
pub fn list_directory_contents(path: String) -> Result<String, &'static str> {
    use std::fmt::Write;
    Ok(std::fs::read_dir(path)
        .map_err(|_| "failed to read directory contents.")?
        .fold(String::new(), |mut acc, entry| {
            match entry {
                Ok(v) => {
                    _ = writeln!(
                        &mut acc,
                        "{}",
                        v.file_name()
                            .to_str()
                            .unwrap_or("unknown error reading this entry.")
                    );
                }

                Err(_) => acc.push_str("unknown error reading this entry.\n"),
            }
            acc
        }))
}

/// # Errors
/// Can error if the directory cannot be created (e.g., permission denied) or path is invalid.
pub fn make_directory(path: String) -> Result<String, &'static str> {
    std::fs::create_dir_all(path).map_err(|_| "failed to create directory")?;
    Ok("successfully created directory".to_string())
}
