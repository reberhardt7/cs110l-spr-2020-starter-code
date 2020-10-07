use std::cmp::min;
use std::io::{Read, Write};
use std::net::TcpStream;

const MAX_HEADERS_SIZE: usize = 8000;
const MAX_BODY_SIZE: usize = 10000000;
const MAX_NUM_HEADERS: usize = 32;

#[derive(Debug)]
pub enum Error {
    /// Client hung up before sending a complete request. IncompleteRequest contains the number of
    /// bytes that were successfully read before the client hung up
    IncompleteRequest(usize),
    /// Client sent an invalid HTTP request. httparse::Error contains more details
    MalformedRequest(httparse::Error),
    /// The Content-Length header is present, but does not contain a valid numeric value
    InvalidContentLength,
    /// The Content-Length header does not match the size of the request body that was sent
    ContentLengthMismatch,
    /// The request body is bigger than MAX_BODY_SIZE
    RequestBodyTooLarge,
    /// Encountered an I/O error when reading/writing a TcpStream
    ConnectionError(std::io::Error),
}

/// Extracts the Content-Length header value from the provided request. Returns Ok(Some(usize)) if
/// the Content-Length is present and valid, Ok(None) if Content-Length is not present, or
/// Err(Error) if Content-Length is present but invalid.
///
/// You won't need to touch this function.
fn get_content_length(request: &http::Request<Vec<u8>>) -> Result<Option<usize>, Error> {
    // Look for content-length header
    if let Some(header_value) = request.headers().get("content-length") {
        // If it exists, parse it as a usize (or return InvalidContentLength if it can't be parsed as such)
        Ok(Some(
            header_value
                .to_str()
                .or(Err(Error::InvalidContentLength))?
                .parse::<usize>()
                .or(Err(Error::InvalidContentLength))?,
        ))
    } else {
        // If it doesn't exist, return None
        Ok(None)
    }
}

/// This function appends to a header value (adding a new header if the header is not already
/// present). This is used to add the client's IP address to the end of the X-Forwarded-For list,
/// or to add a new X-Forwarded-For header if one is not already present.
///
/// You won't need to touch this function.
pub fn extend_header_value(
    request: &mut http::Request<Vec<u8>>,
    name: &'static str,
    extend_value: &str,
) {
    let new_value = match request.headers().get(name) {
        Some(existing_value) => {
            [existing_value.as_bytes(), b", ", extend_value.as_bytes()].concat()
        }
        None => extend_value.as_bytes().to_owned(),
    };
    request
        .headers_mut()
        .insert(name, http::HeaderValue::from_bytes(&new_value).unwrap());
}

/// Attempts to parse the data in the supplied buffer as an HTTP request. Returns one of the
/// following:
///
/// * If there is a complete and valid request in the buffer, returns Ok(Some(http::Request))
/// * If there is an incomplete but valid-so-far request in the buffer, returns Ok(None)
/// * If there is data in the buffer that is definitely not a valid HTTP request, returns Err(Error)
///
/// You won't need to touch this function.
fn parse_request(buffer: &[u8]) -> Result<Option<(http::Request<Vec<u8>>, usize)>, Error> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_NUM_HEADERS];
    let mut req = httparse::Request::new(&mut headers);
    let res = req.parse(buffer).or_else(|err| Err(Error::MalformedRequest(err)))?;

    if let httparse::Status::Complete(len) = res {
        let mut request = http::Request::builder()
            .method(req.method.unwrap())
            .uri(req.path.unwrap())
            .version(http::Version::HTTP_11);
        for header in req.headers {
            request = request.header(header.name, header.value);
        }
        let request = request.body(Vec::new()).unwrap();
        Ok(Some((request, len)))
    } else {
        Ok(None)
    }
}

/// Reads an HTTP request from the provided stream, waiting until a complete set of headers is sent.
/// This function only reads the request line and headers; the read_body function can subsequently
/// be called in order to read the request body (for a POST request).
///
/// Returns Ok(http::Request) if a valid request is received, or Error if not.
///
/// You will need to modify this function in Milestone 2.
fn read_headers(stream: &mut TcpStream) -> Result<http::Request<Vec<u8>>, Error> {
    // Try reading the headers from the request. We may not receive all the headers in one shot
    // (e.g. we might receive the first few bytes of a request, and then the rest follows later).
    // Try parsing repeatedly until we read a valid HTTP request
    let mut request_buffer = [0_u8; MAX_HEADERS_SIZE];
    let mut bytes_read = 0;
    loop {
        // Read bytes from the connection into the buffer, starting at position bytes_read
        let new_bytes = stream
            .read(&mut request_buffer[bytes_read..])
            .or_else(|err| Err(Error::ConnectionError(err)))?;
        if new_bytes == 0 {
            // We didn't manage to read a complete request
            return Err(Error::IncompleteRequest(bytes_read));
        }
        bytes_read += new_bytes;

        // See if we've read a valid request so far
        if let Some((mut request, headers_len)) = parse_request(&request_buffer[..bytes_read])? {
            // We've read a complete set of headers. However, if this was a POST request, a request
            // body might have been included as well, and we might have read part of the body out of
            // the stream into header_buffer. We need to add those bytes to the Request body so that
            // we don't lose them
            request
                .body_mut()
                .extend_from_slice(&request_buffer[headers_len..bytes_read]);
            return Ok(request);
        }
    }
}

/// This function reads the body for a request from the stream. The client only sends a body if the
/// Content-Length header is present; this function reads that number of bytes from the stream. It
/// returns Ok(()) if successful, or Err(Error) if Content-Length bytes couldn't be read.
///
/// You will need to modify this function in Milestone 2.
fn read_body(
    stream: &mut TcpStream,
    request: &mut http::Request<Vec<u8>>,
    content_length: usize,
) -> Result<(), Error> {
    // Keep reading data until we read the full body length, or until we hit an error.
    while request.body().len() < content_length {
        // Read up to 512 bytes at a time. (If the client only sent a small body, then only allocate
        // space to read that body.)
        let mut buffer = vec![0_u8; min(512, content_length)];
        let bytes_read = stream.read(&mut buffer).or_else(|err| Err(Error::ConnectionError(err)))?;

        // Make sure the client is still sending us bytes
        if bytes_read == 0 {
            log::debug!(
                "Client hung up after sending a body of length {}, even though it said the content \
                length is {}",
                request.body().len(),
                content_length
            );
            return Err(Error::ContentLengthMismatch);
        }

        // Make sure the client didn't send us *too many* bytes
        if request.body().len() + bytes_read > content_length {
            log::debug!(
                "Client sent more bytes than we expected based on the given content length!"
            );
            return Err(Error::ContentLengthMismatch);
        }

        // Store the received bytes in the request body
        request.body_mut().extend_from_slice(&buffer[..bytes_read]);
    }
    Ok(())
}

/// This function reads and returns an HTTP request from a stream, returning an Error if the client
/// closes the connection prematurely or sends an invalid request.
///
/// You will need to modify this function in Milestone 2.
pub fn read_from_stream(stream: &mut TcpStream) -> Result<http::Request<Vec<u8>>, Error> {
    // Read headers
    let mut request = read_headers(stream)?;
    // Read body if the client supplied the Content-Length header (which it does for POST requests)
    if let Some(content_length) = get_content_length(&request)? {
        if content_length > MAX_BODY_SIZE {
            return Err(Error::RequestBodyTooLarge);
        } else {
            read_body(stream, &mut request, content_length)?;
        }
    }
    Ok(request)
}

/// This function serializes a request to bytes and writes those bytes to the provided stream.
///
/// You will need to modify this function in Milestone 2.
pub fn write_to_stream(
    request: &http::Request<Vec<u8>>,
    stream: &mut TcpStream,
) -> Result<(), std::io::Error> {
    stream.write(&format_request_line(request).into_bytes())?;
    stream.write(&['\r' as u8, '\n' as u8])?; // \r\n
    for (header_name, header_value) in request.headers() {
        stream.write(&format!("{}: ", header_name).as_bytes())?;
        stream.write(header_value.as_bytes())?;
        stream.write(&['\r' as u8, '\n' as u8])?; // \r\n
    }
    stream.write(&['\r' as u8, '\n' as u8])?;
    if request.body().len() > 0 {
        stream.write(request.body())?;
    }
    Ok(())
}

pub fn format_request_line(request: &http::Request<Vec<u8>>) -> String {
    format!("{} {} {:?}", request.method(), request.uri(), request.version())
}
