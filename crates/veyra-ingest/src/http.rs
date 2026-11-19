//! The small part of HTTP/1.1 the local receiver needs (ADR-038): one request per
//! connection, a body framed by `Content-Length` or chunked transfer coding, and
//! strict size limits. Anything else is refused with a status code.

use std::io::{self, BufRead, Read, Write};

/// Longest request line plus headers accepted.
const MAX_HEAD: u64 = 16 * 1024;
/// Largest body accepted. Tools export in small batches, far below this.
pub const MAX_BODY: u64 = 8 * 1024 * 1024;

/// A parsed request. Header names are lower case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// The path, without any query.
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The first header named `name` (lower case).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// A response status with a short plain-text reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub code: u16,
    pub message: &'static str,
}

impl Status {
    pub const OK: Self = Self::new(200, "");
    pub const BAD_REQUEST: Self = Self::new(400, "malformed request");
    pub const LENGTH_REQUIRED: Self = Self::new(411, "the body length is required");
    pub const TOO_LARGE: Self = Self::new(413, "the body is too large");
    pub const HEAD_TOO_LARGE: Self = Self::new(431, "the request headers are too large");
    pub const NOT_IMPLEMENTED: Self = Self::new(501, "unsupported transfer coding");

    #[must_use]
    pub const fn new(code: u16, message: &'static str) -> Self {
        Self { code, message }
    }

    const fn reason(self) -> &'static str {
        match self.code {
            200 => "OK",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            411 => "Length Required",
            413 => "Content Too Large",
            415 => "Unsupported Media Type",
            431 => "Request Header Fields Too Large",
            501 => "Not Implemented",
            _ => "Service Unavailable",
        }
    }
}

/// Reads one request.
///
/// # Errors
/// The status to answer with when the request is malformed, too large or uses a
/// framing this reader does not support; [`Status::BAD_REQUEST`] on read failures.
pub fn read_request(reader: &mut impl BufRead) -> Result<Request, Status> {
    let mut head = reader.take(MAX_HEAD + 1);
    let request_line = read_line(&mut head)?;
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Status::BAD_REQUEST);
    };
    if !version.starts_with("HTTP/1.") {
        return Err(Status::BAD_REQUEST);
    }
    let mut headers = Vec::new();
    loop {
        let line = read_line(&mut head)?;
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or(Status::BAD_REQUEST)?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
    let reader = head.into_inner();
    let mut request = Request {
        method: method.to_owned(),
        path: target.split('?').next().unwrap_or_default().to_owned(),
        headers,
        body: Vec::new(),
    };
    request.body = match (
        request.header("transfer-encoding"),
        request.header("content-length"),
    ) {
        (Some(coding), _) if coding.eq_ignore_ascii_case("chunked") => read_chunked(reader)?,
        (Some(_), _) => return Err(Status::NOT_IMPLEMENTED),
        (None, Some(length)) => {
            let length: u64 = length.parse().map_err(|_| Status::BAD_REQUEST)?;
            read_exact(reader, length)?
        }
        (None, None) if request.method == "POST" => return Err(Status::LENGTH_REQUIRED),
        (None, None) => Vec::new(),
    };
    Ok(request)
}

/// One CRLF- or LF-terminated line, without its terminator.
fn read_line(reader: &mut impl BufRead) -> Result<String, Status> {
    let mut line = Vec::new();
    reader
        .read_until(b'\n', &mut line)
        .map_err(|_| Status::BAD_REQUEST)?;
    if line.last() != Some(&b'\n') {
        // Cut off by the size limit, or the connection closed mid-line.
        return Err(if line.is_empty() {
            Status::BAD_REQUEST
        } else {
            Status::HEAD_TOO_LARGE
        });
    }
    line.pop();
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(line).map_err(|_| Status::BAD_REQUEST)
}

fn read_exact(reader: &mut impl Read, length: u64) -> Result<Vec<u8>, Status> {
    if length > MAX_BODY {
        return Err(Status::TOO_LARGE);
    }
    let mut body = vec![0; usize::try_from(length).map_err(|_| Status::TOO_LARGE)?];
    reader
        .read_exact(&mut body)
        .map_err(|_| Status::BAD_REQUEST)?;
    Ok(body)
}

fn read_chunked(reader: &mut impl BufRead) -> Result<Vec<u8>, Status> {
    let mut body = Vec::new();
    loop {
        let mut line_reader = reader.by_ref().take(MAX_HEAD);
        let line = read_line(&mut line_reader)?;
        let size = line.split(';').next().unwrap_or_default().trim();
        let size = u64::from_str_radix(size, 16).map_err(|_| Status::BAD_REQUEST)?;
        if size == 0 {
            // Trailer fields, if any, end with an empty line.
            while !read_line(&mut reader.by_ref().take(MAX_HEAD))?.is_empty() {}
            return Ok(body);
        }
        if u64::try_from(body.len()).unwrap_or(u64::MAX) + size > MAX_BODY {
            return Err(Status::TOO_LARGE);
        }
        body.extend(read_exact(reader, size)?);
        if !read_line(&mut reader.by_ref().take(2))?.is_empty() {
            return Err(Status::BAD_REQUEST);
        }
    }
}

/// Writes the response and asks the client to close the connection. A successful
/// response carries the empty OTLP JSON response, `{}`.
///
/// # Errors
/// The write error.
pub fn write_response(writer: &mut impl Write, status: Status) -> io::Result<()> {
    let (content_type, body) = if status.code == 200 {
        ("application/json", "{}")
    } else {
        ("text/plain; charset=utf-8", status.message)
    };
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        status.code,
        status.reason(),
        body.len(),
    )?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

    use std::io::Cursor;

    use super::{MAX_BODY, Status, read_request, write_response};

    fn read(raw: &str) -> Result<super::Request, Status> {
        read_request(&mut Cursor::new(raw.as_bytes().to_vec()))
    }

    #[test]
    fn reads_a_request_with_a_content_length_body() {
        let request = read(
            "POST /v1/logs?x=1 HTTP/1.1\r\nHost: 127.0.0.1:4318\r\n\
             Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
        )
        .unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/logs");
        assert_eq!(request.header("host"), Some("127.0.0.1:4318"));
        assert_eq!(request.body, b"{}");
    }

    #[test]
    fn reads_a_chunked_body() {
        let request = read(
            "POST /v1/logs HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n\
             3\r\nabc\r\n2;ext=1\r\nde\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(request.body, b"abcde");
    }

    #[test]
    fn refuses_what_it_does_not_support() {
        assert_eq!(
            read("POST / HTTP/1.1\r\n\r\n"),
            Err(Status::LENGTH_REQUIRED)
        );
        assert_eq!(
            read("POST / HTTP/1.1\r\nTransfer-Encoding: gzip\r\n\r\n"),
            Err(Status::NOT_IMPLEMENTED)
        );
        assert_eq!(
            read(&format!(
                "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_BODY + 1
            )),
            Err(Status::TOO_LARGE)
        );
        assert_eq!(
            read(&format!(
                "GET / HTTP/1.1\r\nX: {}\r\n\r\n",
                "a".repeat(20_000)
            )),
            Err(Status::HEAD_TOO_LARGE)
        );
        assert_eq!(read("nonsense\r\n\r\n"), Err(Status::BAD_REQUEST));
        assert_eq!(
            read("POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nshort"),
            Err(Status::BAD_REQUEST)
        );
    }

    #[test]
    fn writes_the_empty_otlp_response() {
        let mut out = Vec::new();
        write_response(&mut out, Status::OK).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.ends_with("Content-Length: 2\r\nConnection: close\r\n\r\n{}"));
    }
}
