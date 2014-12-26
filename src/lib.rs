use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::io::IoResult;
use std::str::from_utf8;
use std::collections::HashMap;

#[deriving(Show)]
pub enum Error {
    MethodParseError,
    ResourceParseError,
    VersionParseError,
    MalformedHeaderLineError,
    BodyParsingError,
    StatusCodeParseError,
    StatusReasonParseError
}

#[deriving(Show, PartialEq)]
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
    pub headers: HashMap<String, HeaderVal>,
    pub body: Option<String>
}

#[deriving(Show)]
pub struct Response {
    pub version: Version,
    pub status_code: int,
    pub reason: String,
    pub headers: HashMap<String, HeaderVal>,
    pub body: Option<String>
}

// Tokens
pub const CR: u8 = b'\r';
pub const LF: u8 = b'\n';
pub const SP: u8 = b' ';
pub const COLON: u8 = b':';
pub const COMMA: u8 = b',';
pub const DQUOTE: u8 = b'"';

trait Parser {
    fn read_req_component(&mut self, stream: &mut TcpStream) -> Vec<u8>;
}

struct SPParser {
    buf: Vec<u8>,
    max_token_len: uint
}

impl SPParser {
    fn new() -> SPParser {
        SPParser { buf: Vec::new(), max_token_len: 4096u }
    }
}

impl Parser for SPParser {
    fn read_req_component(&mut self, stream: &mut TcpStream) -> Vec<u8> {
        // Reset parser state
        self.buf.clear();

        loop {
            let byte = stream.read_byte().unwrap();
            if self.buf.len() >= self.max_token_len { break; }
            match byte {
                SP =>{ break; }
                _ => { self.buf.push(byte); }
            }
        }

        return self.buf.clone();
    }
}

#[deriving(Show,PartialEq)]
enum EOLParserState {
    Token,
    CR,
    LF
}

struct EOLParser {
    buf: Vec<u8>,
    max_token_len: uint,
    state: EOLParserState
}

impl EOLParser {
    fn new() -> EOLParser {
        EOLParser { buf: Vec::new(), max_token_len: 4096u, state: EOLParserState::Token }
    }
}

impl Parser for EOLParser {
    fn read_req_component(&mut self, stream: &mut TcpStream) -> Vec<u8> {
        // Reset parser state
        self.buf.clear();

        loop {
            let byte = stream.read_byte().unwrap();
            if self.buf.len() >= self.max_token_len { break; }

            match byte {
                CR => {
                    if self.state != EOLParserState::Token { panic!("Parse error!"); }
                    self.state = EOLParserState::CR;
                }
                LF => {
                    if self.state != EOLParserState::CR { panic!("Parse error!"); }
                    break;
                }
                _ => {
                    self.buf.push(byte);
                }
            }
        }

        return self.buf.clone();
    }
}

struct HeaderKeyParser {
    buf: Vec<u8>,
    max_token_len: uint
}

impl HeaderKeyParser {
    fn new() -> HeaderKeyParser {
        HeaderKeyParser { buf: Vec::new(), max_token_len: 4096u }
    }
}

impl Parser for HeaderKeyParser {
    fn read_req_component(&mut self, stream: &mut TcpStream) -> Vec<u8> {
        // Reset parser state
        self.buf.clear();

        loop {
            let byte = stream.read_byte().unwrap();
            if self.buf.len() >= self.max_token_len { break; }
            match byte {
                COLON => { break; }
                CR => {
                    match stream.read_byte().unwrap() {
                        LF => { break; }
                        _ => { panic!("Parse error!"); }
                    }
                }
                _ => { self.buf.push(byte); }
            }
        }

        return self.buf.clone();
    }
}

#[deriving(Show,PartialEq,Clone)]
pub enum HeaderVal {
    List(Vec<String>),
    Val(String),
    None
}

impl HeaderVal {
    fn to_string(self) -> String {
        match self {
            HeaderVal::List(list) => list.iter().fold(String::new(), |mut acc, x| { acc.push_str(x.to_string().as_slice()); return acc; } ),
            HeaderVal::Val(s) => s,
            HeaderVal::None => String::new()
        }
    }
}

#[deriving(Show,PartialEq)]
enum HeaderValParserState {
    Token,
    TokenDelimeter,
    QuotedString,
    OptionalWhitespace,
    CR,
    LF
}

struct HeaderValParser {
    buf: Vec<u8>,
    max_token_len: uint,
    header_val: HeaderVal,
    state: HeaderValParserState
}

impl HeaderValParser {
    fn new() -> HeaderValParser {
        HeaderValParser { buf: Vec::new(), max_token_len: 4096u, state: HeaderValParserState::OptionalWhitespace, header_val: HeaderVal::None }
    }

    fn read_req_component(&mut self, stream: &mut TcpStream) -> HeaderVal {
        // Reset parser state
        self.buf.clear();
        self.header_val = HeaderVal::None;
        self.state = HeaderValParserState::OptionalWhitespace;

        loop {
            let byte = stream.read_byte().unwrap();
            if self.buf.len() >= self.max_token_len { panic!("Parse error!"); }

            match byte {
                CR => {
                    if self.state != HeaderValParserState::Token { panic!("Parse error!") }
                    self.state = HeaderValParserState::CR;
                }
                LF => {
                    if self.state != HeaderValParserState::CR { panic!("Parse error!"); }
                    break;
                }
                SP => {
                    match self.state {
                        HeaderValParserState::OptionalWhitespace => { continue; }
                        HeaderValParserState::Token => { self.state = HeaderValParserState::TokenDelimeter; }
                        HeaderValParserState::TokenDelimeter => { continue; }
                        HeaderValParserState::CR => { panic!("Parse error!"); }
                        _ => { self.buf.push(byte); }
                    }
                }
                COMMA => {
                    // TODO: consider other "standard" delimeters
                    self.state = HeaderValParserState::TokenDelimeter;
                    let str = String::from_utf8(self.buf.clone()).unwrap_or(String::new()).as_slice().trim().into_string();
                    self.buf.clear();
                    let new_val = match &self.header_val {
                        &HeaderVal::None => HeaderVal::Val(str),
                        &HeaderVal::Val(ref v) => HeaderVal::List(vec!(v.clone(), str)),
                        &HeaderVal::List(ref list) => { let mut new_list = list.clone(); new_list.push(str); HeaderVal::List(new_list) }
                    };
                    self.header_val = new_val;
                }
                DQUOTE => {
                    match self.state {
                        HeaderValParserState::QuotedString => { self.state = HeaderValParserState::Token }
                        HeaderValParserState::CR => { panic!("Parse error!") }
                        _ => { self.state = HeaderValParserState::QuotedString }
                    };
                }
                _ => {
                    self.state = HeaderValParserState::Token;
                    self.buf.push(byte);
                }
            }
        }

        if self.buf.len() > 0 {
            let val = HeaderVal::Val(String::from_utf8(self.buf.clone()).unwrap_or(String::new()).as_slice().trim().into_string());
            let str = String::from_utf8(self.buf.clone()).unwrap_or(String::new()).as_slice().trim().into_string();
            self.buf.clear();
            let new_val = match &self.header_val {
                &HeaderVal::None => HeaderVal::Val(str),
                &HeaderVal::Val(ref v) => HeaderVal::List(vec!(v.clone(), str)),
                &HeaderVal::List(ref list) => { let mut new_list = list.clone(); new_list.push(str); HeaderVal::List(new_list) }
            };
            self.header_val = new_val;
        }

        return self.header_val.clone();
    }
}

struct BodyParser {
    buf: Vec<u8>,
    body_len: uint
}

impl BodyParser {
    fn new(body_len: uint) -> BodyParser {
        BodyParser { buf: Vec::new(), body_len: body_len }
    }
}

impl Parser for BodyParser {
     fn read_req_component(&mut self, stream: &mut TcpStream) -> Vec<u8> {
        // Reset parser state
        self.buf.clear();

        loop {
            let byte = stream.read_byte().unwrap();
            self.buf.push(byte);
            if self.buf.len() >= self.body_len { break; }
        }

        return self.buf.clone();
    }
}

fn read_request_type(stream: &mut TcpStream) -> Option<RequestType> {
    let mut parser = SPParser::new();
    let component = parser.read_req_component(stream);
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

fn read_reason(stream: &mut TcpStream) -> Option<String> {
    let mut parser = EOLParser::new();
    match String::from_utf8(parser.read_req_component(stream)) {
        Ok(s) => Some(s),
        Err(e) => None
    }
}

fn read_resource(stream: &mut TcpStream) -> Option<String> {
    let mut parser = SPParser::new();
    match String::from_utf8(parser.read_req_component(stream)) {
        Ok(s) => Some(s),
        Err(e) => None
    }
}

fn read_version<T: Parser>(stream: &mut TcpStream, parser: &mut T) -> Option<Version> {
    let component = parser.read_req_component(stream);
    return match component.as_slice() {
        b"HTTP/0.9" => Some(Version::Http09),
        b"HTTP/1.0" => Some(Version::Http10),
        b"HTTP/1.1" => Some(Version::Http11),
        b"Http/2.0" => Some(Version::Http20),
        _ => None
    };
}

fn read_status_code(stream: &mut TcpStream) -> Option<int> {
    let mut parser = SPParser::new();
    match from_str::<int>(String::from_utf8(parser.read_req_component(stream)).unwrap_or(String::new()).as_slice()) {
        Some(num) => {
            match num.to_string().len() {
                3 => Some(num),
                _ => None
            }
        }
        None => None
    }
}

fn read_req_line(stream: &mut TcpStream) -> Result<(RequestType, String, Version), Error> {
    let maybe_method = read_request_type(stream);
    let maybe_resource = read_resource(stream);
    let maybe_version = read_version(stream, &mut EOLParser::new());

    if maybe_method.is_none() {
        return Err(Error::MethodParseError);
    }

    if maybe_resource.is_none() {
        return Err(Error::ResourceParseError);
    }

    if maybe_version.is_none() {
        return Err(Error::VersionParseError);
    }

    return Ok((maybe_method.unwrap(), maybe_resource.unwrap(), maybe_version.unwrap()));
}

fn read_status_line(stream: &mut TcpStream) -> Result<(Version, int, String), Error> {
    let maybe_version = read_version(stream, &mut SPParser::new());
    let maybe_code = read_status_code(stream);
    let maybe_reason = read_reason(stream);

    if maybe_version.is_none() {
        return Err(Error::VersionParseError);
    }

    if maybe_code.is_none() {
        return Err(Error::StatusCodeParseError);
    }

    if maybe_reason.is_none() {
        return Err(Error::StatusReasonParseError);
    }

    return Ok((maybe_version.unwrap(), maybe_code.unwrap(), maybe_reason.unwrap()));
}

fn read_headers(stream: &mut TcpStream) -> Result<HashMap<String, HeaderVal>, Error> {
    let mut key_parser = HeaderKeyParser::new();
    let mut val_parser = HeaderValParser::new();

    let mut headers = HashMap::new();
    loop {
        let key = String::from_utf8(key_parser.read_req_component(stream)).unwrap_or(String::new());
        if key.len() == 0 { break; }
        let val_component = val_parser.read_req_component(stream);;

        headers.insert(key, val_component);
    }

    return Ok(headers);
}

fn read_body(stream: &mut TcpStream, len: uint) -> String {
    let mut parser = BodyParser::new(len);
    String::from_utf8(parser.read_req_component(stream)).unwrap_or(String::new())
}

fn read_request(stream: &mut TcpStream) -> Request {
    let (method, resource, version) = read_req_line(stream).unwrap();
    let headers = read_headers(stream).unwrap();
  
    let body = if method == RequestType::HEAD {
        None
    } else {
        match headers.get("Content-Length") {
            None => {
                match headers.get("Transfer-Encoding") {
                    None => None,
                    Some(v) => Some(read_body(stream, 4096))
                }
            }

            Some(len_str) => {
                match from_str::<uint>(len_str.to_string().as_slice()) {
                    None => None,
                    Some(len) => Some(read_body(stream, len))
                }
            }
        }
    };

    return Request {
        method: method,
        version: version,
        resource: resource,
        headers: headers,
        body: body
    };
}

fn read_response(stream: &mut TcpStream) -> Response {
    let (version, status_code, reason) = read_status_line(stream).unwrap();
    let headers = read_headers(stream).unwrap();
  
    let body = match status_code {
        204 => None,
        304 => None,
        _ => {
            if status_code >= 100 && status_code < 200 {
                None
            } else {
                match headers.get("Content-Length") {
                    None => {
                        match headers.get("Transfer-Encoding") {
                            None => None,
                            Some(v) => Some(read_body(stream, 4096))
                        }
                    }

                    Some(len_str) => {
                        match from_str::<uint>(len_str.to_string().as_slice()) {
                            None => None,
                            Some(len) => Some(read_body(stream, len))
                        }
                    }
                }
            }
        }
    };

    return Response {
        version: version,
        status_code: status_code,
        reason: reason,
        headers: headers,
        body: body
    };
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
                    let req = read_request(&mut stream);
                    println!("Req: {}", req);

                    // Write something back
                    stream.write(b"HTTP/1.1 200 VERY OK\r\nContent-Type: text/plain\r\nContent-Length:5\r\n\r\nHello");
                })
            }
        }
    });

    spawn(proc() {
        let mut stream = TcpStream::connect("127.0.0.1:3000").unwrap();
        stream.write(b"GET /index.html HTTP/1.1\r\nContent-Type: text/plain\r\nContent-Length:5\r\nTransfer-Encoding: gzip, chunked\r\n\r\nHello").unwrap();
        println!("Client got: {}", read_response(&mut stream));
    });
}
