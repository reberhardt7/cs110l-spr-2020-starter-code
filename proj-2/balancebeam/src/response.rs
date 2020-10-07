use std::io::{Read, Write};
use std::net::TcpStream;

const MAX_HEADERS_SIZE: usize = 8000;
const MAX_BODY_SIZE: usize = 10000000;
const MAX_NUM_HEADERS: usize = 32;

#[derive(Debug)]
pub enum Error {
    /// Client hung up before sending a complete request
    IncompleteResponse,
    /// Client sent an invalid HTTP request. httparse::Error contains more details
    MalformedResponse(httparse::Error),
    /// The Content-Length header is present, but does not contain a valid numeric value
    InvalidContentLength,
    /// The Content-Length header does not match the size of the request body that was sent
    ContentLengthMismatch,
    /// The request body is bigger than MAX_BODY_SIZE
    ResponseBodyTooLarge,
    /// Encountered an I/O error when reading/writing a TcpStream
    ConnectionError(std::io::Error),
}

/// Extracts the Content-Length header value from the provided response. Returns Ok(Some(usize)) if
/// the Content-Length is present and valid, Ok(None) if Content-Length is not present, or
/// Err(Error) if Content-Length is present but invalid.
///
/// You won't need to touch this function.
fn get_content_length(response: &http::Response<Vec<u8>>) -> Result<Option<usize>, Error> {
    // Look for content-length header
    if let Some(header_value) = response.headers().get("content-length") {
        // If it exists, parse it as a usize (or return InvalidResponseFormat if it can't be parsed as such)
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

/// Attempts to parse the data in the supplied buffer as an HTTP response. Returns one of the
/// following:
///
/// * If there is a complete and valid response in the buffer, returns Ok(Some(http::Request))
/// * If there is an incomplete but valid-so-far response in the buffer, returns Ok(None)
/// * If there is data in the buffer that is definitely not a valid HTTP response, returns
///   Err(Error)
///
/// You won't need to touch this function.
fn parse_response(buffer: &[u8]) -> Result<Option<(http::Response<Vec<u8>>, usize)>, Error> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_NUM_HEADERS];
    let mut resp = httparse::Response::new(&mut headers);
    let res = resp
        .parse(buffer)
        .or_else(|err| Err(Error::MalformedResponse(err)))?;

    if let httparse::Status::Complete(len) = res {
        let mut response = http::Response::builder()
            .status(resp.code.unwrap())
            .version(http::Version::HTTP_11);
        for header in resp.headers {
            response = response.header(header.name, header.value);
        }
        let response = response.body(Vec::new()).unwrap();
        Ok(Some((response, len)))
    } else {
        Ok(None)
    }
}

/// Reads an HTTP response from the provided stream, waiting until a complete set of headers is
/// sent. This function only reads the response line and headers; the read_body function can
/// subsequently be called in order to read the response body.
///
/// Returns Ok(http::Response) if a valid response is received, or Error if not.
///
/// You will need to modify this function in Milestone 2.
fn read_headers(stream: &mut TcpStream) -> Result<http::Response<Vec<u8>>, Error> {
    // Try reading the headers from the response. We may not receive all the headers in one shot
    // (e.g. we might receive the first few bytes of a response, and then the rest follows later).
    // Try parsing repeatedly until we read a valid HTTP response
    let mut response_buffer = [0_u8; MAX_HEADERS_SIZE];
    let mut bytes_read = 0;
    loop {
        // Read bytes from the connection into the buffer, starting at position bytes_read
        let new_bytes = stream
            .read(&mut response_buffer[bytes_read..])
            .or_else(|err| Err(Error::ConnectionError(err)))?;
        if new_bytes == 0 {
            // We didn't manage to read a complete response
            return Err(Error::IncompleteResponse);
        }
        bytes_read += new_bytes;

        // See if we've read a valid response so far
        if let Some((mut response, headers_len)) = parse_response(&response_buffer[..bytes_read])? {
            // We've read a complete set of headers. We may have also read the first part of the
            // response body; take whatever is left over in the response buffer and save that as
            // the start of the response body.
            response
                .body_mut()
                .extend_from_slice(&response_buffer[headers_len..bytes_read]);
            return Ok(response);
        }
    }
}

/// This function reads the body for a response from the stream. If the Content-Length header is
/// present, it reads that many bytes; otherwise, it reads bytes until the connection is closed.
///
/// You will need to modify this function in Milestone 2.
fn read_body(stream: &mut TcpStream, response: &mut http::Response<Vec<u8>>) -> Result<(), Error> {
    // The response may or may not supply a Content-Length header. If it provides the header, then
    // we want to read that number of bytes; if it does not, we want to keep reading bytes until
    // the connection is closed.
    let content_length = get_content_length(response)?;

    while content_length.is_none() || response.body().len() < content_length.unwrap() {
        let mut buffer = [0_u8; 512];
        let bytes_read = stream
            .read(&mut buffer)
            .or_else(|err| Err(Error::ConnectionError(err)))?;
        if bytes_read == 0 {
            // The server has hung up!
            if content_length.is_none() {
                // We've reached the end of the response
                break;
            } else {
                // Content-Length was set, but the server hung up before we managed to read that
                // number of bytes
                return Err(Error::ContentLengthMismatch);
            }
        }

        // Make sure the server doesn't send more bytes than it promised to send
        if content_length.is_some() && response.body().len() + bytes_read > content_length.unwrap()
        {
            return Err(Error::ContentLengthMismatch);
        }

        // Make sure server doesn't send more bytes than we allow
        if response.body().len() + bytes_read > MAX_BODY_SIZE {
            return Err(Error::ResponseBodyTooLarge);
        }

        // Append received bytes to the response body
        response.body_mut().extend_from_slice(&buffer[..bytes_read]);
    }
    Ok(())
}

/// This function reads and returns an HTTP response from a stream, returning an Error if the server
/// closes the connection prematurely or sends an invalid response.
///
/// You will need to modify this function in Milestone 2.
pub fn read_from_stream(
    stream: &mut TcpStream,
    request_method: &http::Method,
) -> Result<http::Response<Vec<u8>>, Error> {
    let mut response = read_headers(stream)?;
    // A response may have a body as long as it is not responding to a HEAD request and as long as
    // the response status code is not 1xx, 204 (no content), or 304 (not modified).
    if !(request_method == http::Method::HEAD
        || response.status().as_u16() < 200
        || response.status() == http::StatusCode::NO_CONTENT
        || response.status() == http::StatusCode::NOT_MODIFIED)
    {
        read_body(stream, &mut response)?;
    }
    Ok(response)
}

/// This function serializes a response to bytes and writes those bytes to the provided stream.
///
/// You will need to modify this function in Milestone 2.
pub fn write_to_stream(
    response: &http::Response<Vec<u8>>,
    stream: &mut TcpStream,
) -> Result<(), std::io::Error> {
    stream.write(&format_response_line(response).into_bytes())?;
    stream.write(&['\r' as u8, '\n' as u8])?; // \r\n
    for (header_name, header_value) in response.headers() {
        stream.write(&format!("{}: ", header_name).as_bytes())?;
        stream.write(header_value.as_bytes())?;
        stream.write(&['\r' as u8, '\n' as u8])?; // \r\n
    }
    stream.write(&['\r' as u8, '\n' as u8])?;
    if response.body().len() > 0 {
        stream.write(response.body())?;
    }
    Ok(())
}

pub fn format_response_line(response: &http::Response<Vec<u8>>) -> String {
    format!(
        "{:?} {} {}",
        response.version(),
        response.status().as_str(),
        response.status().canonical_reason().unwrap_or("")
    )
}

/// This is a helper function that creates an http::Response containing an HTTP error that can be
/// sent to a client.
pub fn make_http_error(status: http::StatusCode) -> http::Response<Vec<u8>> {
    let body = format!(
        "HTTP {} {}",
        status.as_u16(),
        status.canonical_reason().unwrap_or("")
    )
    .into_bytes();
    http::Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .header("Content-Length", body.len().to_string())
        .version(http::Version::HTTP_11)
        .body(body)
        .unwrap()
}
