use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::io::IoResult;
use std::str::from_utf8;
use std::collections::HashMap;

#[deriving(Show)]
pub enum HttpError {
    MethodParseError,
    ResourceParseError,
    VersionParseError,
    MalformedHeaderLineError
}

#[deriving(Show)]
pub enum RequestType {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    TRACE,
    OPTIONS,
    CONNECT,
    PATCH
}

#[deriving(PartialEq, PartialOrd, Show)]
pub enum Version {
    Http09,
    Http10,
    Http11,
    Http20
}

#[deriving(Show)]
pub struct Request {
    pub method: RequestType,
    pub version: Version,
    pub resource: String,
    pub host: String,
    pub headers: HashMap<String, String>,
    pub body: String
}

// Tokens
pub const CR: u8 = b'\r';
pub const LF: u8 = b'\n';
pub const SP: u8 = b' ';
pub const EOL: &'static [u8] = &[CR, LF];

#[deriving(Show)]
enum ParserState {
    Incomplete,
    Read_CR,
    EndComponent,
    EndLine,
    Reject
}

#[deriving(Show)]
struct Parser {
    buf: Vec<u8>,
    state: ParserState
}

impl Parser {
    fn new() -> Parser {
        Parser { buf: Vec::new(), state: ParserState::Incomplete }
    }

    fn put(&mut self, byte: u8) {
        match byte {
            SP => {
                self.state = ParserState::EndComponent;
            }

            CR => {
                self.state = ParserState::Read_CR;
            }

            LF => {
                match self.state {
                    ParserState::Read_CR => self.state = ParserState::EndLine,
                    _ => self.state = ParserState::Reject
                }
            }

            _ => {
                self.buf.push(byte);
            }
        }
    }
}

fn read_req_component(stream: &mut TcpStream) -> Vec<u8> {
    let mut parser = Parser::new();

    loop {
        let byte = stream.read_byte().unwrap();
        parser.put(byte);
        match parser.state {
            ParserState::Reject => { panic!("Failed parsing"); }
            ParserState::EndComponent => { break; }
            ParserState::EndLine => { break; }
            _ => { continue; }
        }
    }

    return parser.buf;
}

fn read_request_type(stream: &mut TcpStream) -> Option<RequestType> {
    let component = read_req_component(stream);
    let method = match component.as_slice() {
        b"GET" => Some(RequestType::GET),
        b"HEAD" => Some(RequestType::HEAD),
        b"POST" => Some(RequestType::POST),
        b"PUT" => Some(RequestType::PUT),
        b"DELETE" => Some(RequestType::DELETE),
        b"TRACE" => Some(RequestType::TRACE),
        b"OPTIONS" => Some(RequestType::OPTIONS),
        b"CONNECT" => Some(RequestType::CONNECT),
        b"PATCH" => Some(RequestType::PATCH),
        _ => None
    };

    return method;
}

fn read_resource(stream: &mut TcpStream) -> Option<String> {
    match String::from_utf8(read_req_component(stream)) {
        Ok(s) => Some(s),
        Err(e) => None
    }
}

fn read_version(stream: &mut TcpStream) -> Option<Version> {
    let component = read_req_component(stream);
    let version = match component.as_slice() {
        b"HTTP/0.9" => Some(Version::Http09),
        b"HTTP/1.0" => Some(Version::Http10),
        b"HTTP/1.1" => Some(Version::Http11),
        b"Http/2.0" => Some(Version::Http20),
        _ => None
    };

    return version;
}

fn read_req_line(stream: &mut TcpStream) -> Result<(RequestType, String, Version), HttpError> {
    let maybe_method = read_request_type(stream);
    let maybe_resource = read_resource(stream);
    let maybe_version = read_version(stream);

    if (maybe_method.is_none()) {
        return Err(HttpError::MethodParseError);
    }

    if (maybe_resource.is_none()) {
        return Err(HttpError::ResourceParseError);
    }

    if (maybe_version.is_none()) {
        return Err(HttpError::VersionParseError);
    }

    return Ok((maybe_method.unwrap(), maybe_resource.unwrap(), maybe_version.unwrap()));
}

fn read_headers(stream: &mut TcpStream) -> Result<HashMap<String, String>, HttpError> {
    let mut headers = HashMap::new();
    loop {
        let mut header_component = read_req_component(stream);
        header_component.pop(); // Remove the ':' character
        let key = String::from_utf8(header_component).unwrap_or(String::new());

        // Empty line read
        if key.len() == 0 {
            break;
        }

        let val_component = String::from_utf8(read_req_component(stream)).unwrap_or(String::new());
        if (val_component.len() == 0) {
            return Err(HttpError::MalformedHeaderLineError);
        }

        headers.insert(key, val_component);
    }

    return Ok(headers);
}

#[test]
fn it_works() {
    let tcp_listener = TcpListener::bind("127.0.0.1:3000");
    let mut acceptor = tcp_listener.listen();

    // Spawn HTTP server
    spawn(proc() {
        for mut opt_stream in acceptor.incoming() {
            match opt_stream {
                Err(e) => println!("Error: {}", e),
                Ok(mut stream) => spawn(proc() {
                    let req_line = read_req_line(&mut stream);
                    println!("Req line: {}", req_line);

                    let headers = read_headers(&mut stream);
                    println!("Headers: {}", headers);
                })
            }
        }
    });

    spawn(proc() {
        let mut stream = TcpStream::connect("127.0.0.1:3000");
        stream.write(b"GET /index.html HTTP/1.1\r\nContent-Type: text/html\r\n\r\n").unwrap();

        let mut buf = [0u8, ..4096];
        let count = stream.read(&mut buf);
        let msg = from_utf8(&buf).unwrap_or("");
        println!("Client got: {}", msg);
    });
}
