use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;
use std::time::Duration;

/// Consume the whole request before replying, including form bodies that may
/// arrive after the headers. Closing with unread bytes can reset the connection.
pub(crate) fn read_http_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    read_request(stream)
}

fn read_request(reader: impl Read) -> String {
    let mut reader = BufReader::new(reader);
    let mut request = String::new();
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        assert!(
            reader.read_line(&mut line).unwrap() > 0,
            "incomplete HTTP headers"
        );
        request.push_str(&line);
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().unwrap();
            }
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    request.push_str(&String::from_utf8(body).unwrap());
    request
}

#[test]
fn http_request_reads_headers_and_body_from_separate_reads() {
    let headers = b"POST /token HTTP/1.1\r\ncOnTeNt-LeNgTh: 7\r\n\r\n";
    let reader = headers
        .as_slice()
        .chain(b"a=1&".as_slice())
        .chain(b"b=2".as_slice());
    assert_eq!(
        read_request(reader),
        format!("{}a=1&b=2", String::from_utf8_lossy(headers))
    );
}

#[test]
fn http_request_without_a_body_finishes_after_headers() {
    let request = "GET /photo HTTP/1.1\r\nAuthorization: Bearer graph\r\n\r\n";
    assert_eq!(read_request(request.as_bytes()), request);
}
