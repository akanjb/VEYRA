//! Opening the JSON documents of a data export, from the ZIP archive the provider
//! sends or from a document the user already extracted, and reading their top-level
//! array one element at a time so memory stays bounded by the largest element.

use std::fmt;
use std::io::{BufReader, Read, SeekFrom};

use serde::de::{self, Deserializer as _, SeqAccess, Visitor};
use serde_json::value::RawValue;
use veyra_domain::import::{ImportError, ReadSeek};

/// The first bytes of a ZIP archive: a local file header or an empty archive's end.
const ZIP_MAGIC: [&[u8; 4]; 2] = [b"PK\x03\x04", b"PK\x05\x06"];

fn unreadable(e: impl fmt::Display) -> ImportError {
    ImportError::Unreadable(e.to_string())
}

/// Calls `read` with each document in `file` whose file name `wanted` accepts, in name
/// order. A file that is not a ZIP archive is itself the only document, whatever its
/// name. Returns whether any document was read.
///
/// # Errors
/// [`ImportError::Unreadable`] when the archive is damaged, or the error `read`
/// returned.
pub fn each_document(
    file: &mut dyn ReadSeek,
    wanted: impl Fn(&str) -> bool,
    read: &mut dyn FnMut(&mut dyn Read) -> Result<(), ImportError>,
) -> Result<bool, ImportError> {
    file.seek(SeekFrom::Start(0)).map_err(unreadable)?;
    let mut magic = [0; 4];
    let is_zip = match file.read_exact(&mut magic) {
        Ok(()) => ZIP_MAGIC.iter().any(|m| **m == magic),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => false,
        Err(e) => return Err(unreadable(e)),
    };
    file.seek(SeekFrom::Start(0)).map_err(unreadable)?;
    if !is_zip {
        read(&mut BufReader::new(file))?;
        return Ok(true);
    }
    let mut archive = zip::ZipArchive::new(file).map_err(unreadable)?;
    let mut names: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .filter(|name| wanted(name.rsplit('/').next().unwrap_or(name)))
        .map(String::from)
        .collect();
    names.sort();
    for name in &names {
        let entry = archive.by_name(name).map_err(unreadable)?;
        read(&mut BufReader::new(entry))?;
    }
    Ok(!names.is_empty())
}

/// Calls `each` with every element of the JSON array `reader` holds. Stops at the first
/// error `each` returns.
///
/// # Errors
/// The error `each` returned; [`ImportError::NotRecognized`] when the document is not
/// a JSON array; [`ImportError::Unreadable`] when it ends early or is malformed.
pub fn each_element(
    reader: &mut dyn Read,
    each: &mut dyn FnMut(&RawValue) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    let mut visitor = Elements {
        each,
        stopped: None,
        started: false,
    };
    let mut parser = serde_json::Deserializer::from_reader(reader);
    let outcome = (&mut parser).deserialize_seq(&mut visitor);
    if let Some(stopped) = visitor.stopped {
        return Err(stopped);
    }
    outcome.map_err(|e| {
        if visitor.started {
            unreadable(e)
        } else {
            ImportError::NotRecognized
        }
    })?;
    parser.end().map_err(unreadable)
}

struct Elements<'a> {
    each: &'a mut dyn FnMut(&RawValue) -> Result<(), ImportError>,
    /// The error `each` stopped with, kept whole across serde's error type.
    stopped: Option<ImportError>,
    /// The document opened as an array.
    started: bool,
}

impl<'de> Visitor<'de> for &mut Elements<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.started = true;
        while let Some(element) = seq.next_element::<Box<RawValue>>()? {
            if let Err(e) = (self.each)(&element) {
                self.stopped = Some(e);
                return Err(de::Error::custom("stopped"));
            }
        }
        Ok(())
    }
}
