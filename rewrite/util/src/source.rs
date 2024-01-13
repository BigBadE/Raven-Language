use std::fs;
use std::path::PathBuf;

pub trait SourceSet {
    fn get_files(&self) -> Vec<Box<dyn SourceFile>>;
}

pub trait SourceFile {
    fn read(&self) -> Vec<u8>;
}

pub struct FileSourceSet {
    pub path: PathBuf,
}

impl FileSourceSet {
    pub fn new(path: PathBuf) -> Self {
        return FileSourceSet { path };
    }
    pub fn read_recursive(&self, vector: &mut Vec<Box<dyn SourceFile>>, path: &PathBuf) {
        let file = fs::metadata(path).unwrap();
        if file.is_dir() {
            for file in fs::read_dir(path).unwrap() {
                self.read_recursive(vector, &file.unwrap().path());
            }
        } else {
            vector.push(Box::new(FileSourceSet::new(path.clone())));
        }
    }
}

impl SourceSet for FileSourceSet {
    fn get_files(&self) -> Vec<Box<dyn SourceFile>> {
        let mut output = vec![];
        self.read_recursive(&mut output, &self.path);
        return output;
    }
}

impl SourceFile for FileSourceSet {
    fn read(&self) -> Vec<u8> {
        return fs::read(&self.path).unwrap();
    }
}
