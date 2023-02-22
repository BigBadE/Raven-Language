use std::fs;
use std::fs::FileType;
use std::path::PathBuf;
use parser::FileStructure;

pub struct FileStructureImpl {
    root: PathBuf
}

impl FileStructureImpl {
    pub fn new(root: PathBuf) -> Self {
        return Self {
            root
        }
    }

    fn get_files_recursive(&self, path: PathBuf, vec: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                vec.push(entry.path());
            } else {
                self.get_files_recursive(entry.path(), vec);
            }
        }
    }
}

impl FileStructure for FileStructureImpl {
    fn get_files(&self) -> Vec<PathBuf> {
        let mut output = Vec::new();
        self.get_files_recursive(self.root.join("src"), &mut output);
        return output;
    }

    fn get_root(&self) -> PathBuf {
        return self.root.join("src").clone();
    }
}