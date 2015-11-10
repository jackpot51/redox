use redox::{Box, GetSlice, String, ToString, Vec, Url};
use redox::fs::File;
use redox::io::Read;

use orbital::BmpFile;

/// A package (_REDOX content serialized)
pub struct Package {
    /// The URL
    pub url: Url,
    /// The ID of the package
    pub id: String,
    /// The name of the package
    pub name: String,
    /// The binary for the package
    pub binary: Url,
    /// The icon for the package
    pub icon: BmpFile,
    /// The accepted extensions
    pub accepts: Vec<String>,
    /// The author(s) of the package
    pub authors: Vec<String>,
    /// The description of the package
    pub descriptions: Vec<String>,
}

impl Package {
    /// Create package from URL
    pub fn from_url(url: &Url) -> Box<Self> {
        let mut package = box Package {
            url: url.clone(),
            id: String::new(),
            name: String::new(),
            binary: Url::new(),
            icon: BmpFile::default(),
            accepts: Vec::new(),
            authors: Vec::new(),
            descriptions: Vec::new(),
        };

        {
            for part in url.to_string().rsplit('/') {
                if ! part.is_empty() {
                    debugln!("{}: {}", part, url.to_string());
                    package.id = part.to_string();
                    package.binary = Url::from_string(url.to_string() + &part + ".bin");
                    break;
                }
            }
        }

        let mut info = String::new();

        if let Some(mut file) = File::open(&(url.to_string() + "_REDOX")) {
            file.read_to_string(&mut info);
        }

        for line in info.lines() {
            if line.starts_with("name=") {
                package.name = line.get_slice(Some(5), None).to_string();
            } else if line.starts_with("binary=") {
                package.binary = Url::from_string(url.to_string() + line.get_slice(Some(7), None));
            } else if line.starts_with("icon=") {
                if let Some(mut file) = File::open(line.get_slice(Some(5), None)) {
                    let mut vec: Vec<u8> = Vec::new();
                    file.read_to_end(&mut vec);
                    package.icon = BmpFile::from_data(&vec);
                }
            } else if line.starts_with("accept=") {
                package.accepts.push(line.get_slice(Some(7), None).to_string());
            } else if line.starts_with("author=") {
                package.authors.push(line.get_slice(Some(7), None).to_string());
            } else if line.starts_with("description=") {
                package.descriptions.push(line.get_slice(Some(12), None).to_string());
            } else {
                debugln!("Unknown package info: {}", line);
            }
        }

        package
    }
}
