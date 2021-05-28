use std::{
    io::{self, Error, ErrorKind},
    path::Path,
};

use std::fs;

use std::path::PathBuf;

pub fn ls(path: &str) -> Result<Vec<u8>, std::io::Error> {
    let mut buff = vec![];
    std::fs::read_dir(clear_path(path))?.for_each(|now| {
        if now.is_err() {
            return;
        }
        let now = now.unwrap();
        let p = PathBuf::from(now.path());
        let c = p.components();
        let end_path = c.skip(2).collect::<PathBuf>();
        let str = format!("{}\r\n", end_path.to_str().unwrap());
        buff.extend(str.as_bytes().iter());
    });
    Ok(buff)
}

pub fn copy_dir<U: AsRef<Path>, V: AsRef<Path>>(from: U, to: V) -> Result<(), std::io::Error> {
    let mut stack = Vec::new();
    stack.push(PathBuf::from(from.as_ref()));

    let output_root = PathBuf::from(to.as_ref());
    let input_root = PathBuf::from(from.as_ref()).components().count();

    while let Some(working_path) = stack.pop() {
        // Generate a relative path
        let src: PathBuf = working_path.components().skip(input_root).collect();

        // Create a destination if missing
        let dest = if src.components().count() == 0 {
            output_root.clone()
        } else {
            output_root.join(&src)
        };
        if fs::metadata(&dest).is_err() {
            fs::create_dir_all(&dest)?;
        }

        for entry in fs::read_dir(working_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                match path.file_name() {
                    Some(filename) => {
                        let dest_path = dest.join(filename);
                        fs::copy(&path, &dest_path)?;
                    }
                    None => {}
                }
            }
        }
    }

    Ok(())
}

pub fn rename<P: AsRef<Path>>(from: P, to: P) -> io::Result<()> {
    let original_from = from.as_ref().clone();
    let original_to = to.as_ref().clone();
    let mut path_from = from.as_ref().to_path_buf();
    let mut path_to = to.as_ref().to_path_buf();
    path_from.pop();
    path_to.pop();
    if path_from != path_to {
        if let Ok(_) = fs::metadata(original_to) {
            return Err(Error::from(ErrorKind::AlreadyExists));
        }
        let metadata = fs::metadata(original_from)?;
        if metadata.is_dir() {
            fs::create_dir(original_to)?;
            copy_dir(original_from, original_to)?;
            fs::remove_dir_all(original_from)
        } else {
            fs::File::create(original_to)?;
            fs::copy(original_from, original_to).map(|_| ())?;
            fs::remove_file(original_from)
        }
    } else {
        fs::rename(original_from, original_to)
    }
}

fn clear_path(path: &str) -> String {
    let p = Path::new("./").canonicalize().unwrap();
    let path = path.replacen(p.to_str().unwrap(), ".", 1);
    path
}

#[cfg(test)]
mod test {
    use super::clear_path;
    use std::path::Path;
    #[test]
    fn testing_clear_path() {
        let p = Path::new("./root/user_01").canonicalize().unwrap();
        assert_eq!(clear_path(p.to_str().unwrap()), "./root/user_01");
    }
}
