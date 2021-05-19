use std::path::Path;

pub fn ls(path: &str) -> Result<Vec<u8>, std::io::Error> {
    let mut buff = vec![];
    std::fs::read_dir(clear_path(path))?.for_each(|now| {
        if now.is_err() {
            return;
        }
        let now = now.unwrap();
        let str = format!("{}\r\n", now.path().to_str().unwrap());
        buff.extend(str.as_bytes().iter());
    });
    Ok(buff)
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
