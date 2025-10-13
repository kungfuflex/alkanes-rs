
use std::path::{Path, PathBuf};
use std::env;

pub fn expand_tilde<P: AsRef<Path>>(path: P) -> Result<PathBuf, std::io::Error> {
    let path = path.as_ref();
    if !path.starts_with("~") {
        return Ok(path.to_path_buf());
    }
    let mut home_dir = match env::var("HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "HOME environment variable not set")),
    };
    if path == Path::new("~") {
        return Ok(home_dir);
    }
    let mut components = path.components();
    components.next(); 
    for component in components {
        home_dir.push(component);
    }
    Ok(home_dir)
}
