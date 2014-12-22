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
    MalformedHeaderLineError,
    BodyParsingError,
    StatusCodeParseError,
    StatusReasonParseError
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

    fn read_req_component(&mut self, stream: &mut TcpStream) -> Vec<u8> {
        // Reset parser state
        self.buf.clear();
        self.state = ParserState::Incomplete;

        loop {
            let byte = stream.read_byte().unwrap();
            self.put(byte);
            match self.state {
                ParserState::Reject => { panic!("Failed parsing"); }
                ParserState::EndComponent => { break; }
                ParserState::EndLine => { break; }
                _ => { continue; }
            }
        }

        return self.buf.clone();
    }
    
    fn read_request_type(&mut self, stream: &mut TcpStream) -> Option<RequestType> {
        let component = self.read_req_component(stream);
        return match component.as_slice() {
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
    }
    
    fn read_str(&mut self, stream: &mut TcpStream) -> Option<String> {
        match String::from_utf8(self.read_req_component(stream)) {
            Ok(s) => Some(s),
            Err(e) => None
        }
    }
    
    fn read_version(&mut self, stream: &mut TcpStream) -> Option<Version> {
        let component = self.read_req_component(stream);
        return match component.as_slice() {
            b"HTTP/0.9" => Some(Version::Http09),
            b"HTTP/1.0" => Some(Version::Http10),
            b"HTTP/1.1" => Some(Version::Http11),
            b"Http/2.0" => Some(Version::Http20),
            _ => None
        };
    }
    
    fn read_status_code(&mut self, stream: &mut TcpStream) -> Option<int> {
        return from_str::<int>(String::from_utf8(self.read_req_component(stream)).unwrap_or(String::new()).as_slice());
    }
    
    fn read_req_line(&mut self, stream: &mut TcpStream) -> Result<(RequestType, String, Version), HttpError> {
        let maybe_method = self.read_request_type(stream);
        let maybe_resource = self.read_str(stream);
        let maybe_version = self.read_version(stream);
    
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
    
    fn read_status_line(&mut self, stream: &mut TcpStream) -> Result<(Version, int, String), HttpError> {
        let maybe_version = self.read_version(stream);
        let maybe_code = self.read_status_code(stream);
        let maybe_reason = self.read_str(stream);
    
        if (maybe_version.is_none()) {
            return Err(HttpError::VersionParseError);
        }
    
        if (maybe_code.is_none()) {
            return Err(HttpError::StatusCodeParseError);
        }
    
        if (maybe_reason.is_none()) {
            return Err(HttpError::StatusReasonParseError);
        }
    
        return Ok((maybe_version.unwrap(), maybe_code.unwrap(), maybe_reason.unwrap()));
    }
    
    fn read_headers(&mut self, stream: &mut TcpStream) -> Result<HashMap<String, String>, HttpError> {
        let mut headers = HashMap::new();
        loop {
            let mut header_component = self.read_req_component(stream);
            header_component.pop(); // Remove the ':' character
            let key = String::from_utf8(header_component).unwrap_or(String::new());
    
            // Empty line read
            if key.len() == 0 {
                break;
            }
    
            let val_component = String::from_utf8(self.read_req_component(stream)).unwrap_or(String::new());
            if (val_component.len() == 0) {
                return Err(HttpError::MalformedHeaderLineError);
            }
    
            headers.insert(key, val_component);
        }
    
        return Ok(headers);
    }
    
    fn read_body(&mut self, stream: &mut TcpStream) -> String {
        String::from_utf8(self.read_req_component(stream)).unwrap_or(String::new())
    }
    
    fn read_request(&mut self, stream: &mut TcpStream) -> Request {
        let (method, resource, version) = self.read_req_line(stream).unwrap();
        let headers = self.read_headers(stream).unwrap();
        let body = self.read_body(stream);
    
        return Request {
            method: method,
            version: version,
            resource: resource,
            headers: headers,
            body: body
        };
    }
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
                    let mut parser = Parser::new();
                    let req = parser.read_request(&mut stream);
                    println!("Req: {}", req);

                    // Write something back
                    stream.write(b"HTTP/1.1 200 VERY OK\r\n");
                })
            }
        }
    });

    spawn(proc() {
        let mut stream = TcpStream::connect("127.0.0.1:3000").unwrap();
        stream.write(b"GET /index.html HTTP/1.1\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nHello\r\n").unwrap();
        let mut parser = Parser::new();
        println!("Client got: {}", parser.read_status_line(&mut stream));
    });
}
