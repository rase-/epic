use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::io::IoResult;
use std::str::from_utf8;
use std::collections::HashMap;

use http::{RequestType, HeaderVal, Version, Error, Request, Response};

// Tokens
const CR: u8 = b'\r';
const LF: u8 = b'\n';
const SP: u8 = b' ';
const COLON: u8 = b':';
const COMMA: u8 = b',';
const DQUOTE: u8 = b'"';

trait Parser {
    fn read_req_component(&mut self, stream: &mut TcpStream) -> Vec<u8>;
}

struct SPParser {
    buf: Vec<u8>,
    max_token_len: usize
}

impl SPParser {
    fn new() -> SPParser {
        SPParser { buf: Vec::new(), max_token_len: 4096us }
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

#[derive(Show,PartialEq)]
enum EOLParserState {
    Token,
    CR,
    LF
}

struct EOLParser {
    buf: Vec<u8>,
    max_token_len: usize,
    state: EOLParserState
}

impl EOLParser {
    fn new() -> EOLParser {
        EOLParser { buf: Vec::new(), max_token_len: 4096us, state: EOLParserState::Token }
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
    max_token_len: usize
}

impl HeaderKeyParser {
    fn new() -> HeaderKeyParser {
        HeaderKeyParser { buf: Vec::new(), max_token_len: 4096us }
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

#[derive(Show,PartialEq,Clone)]
enum HeaderValParserState {
    Token,
    TokenDelimeter,
    QuotedString,
    OptionalWhitespace,
    CR,
    LF
}

#[derive(Clone)]
struct HeaderValParser {
    buf: Vec<u8>,
    max_token_len: usize,
    header_val: HeaderVal,
    state: HeaderValParserState
}

impl HeaderValParser {
    fn new() -> HeaderValParser {
        HeaderValParser { buf: Vec::new(), max_token_len: 4096us, state: HeaderValParserState::OptionalWhitespace, header_val: HeaderVal::None }
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
    body_len: usize
}

impl BodyParser {
    fn new(body_len: usize) -> BodyParser {
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

fn read_status_code(stream: &mut TcpStream) -> Option<isize> {
    let mut parser = SPParser::new();
    let status_code_str = String::from_utf8(parser.read_req_component(stream)).unwrap_or(String::new());
    let status_code = status_code_str.parse::<isize>();
    match status_code {
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

fn read_status_line(stream: &mut TcpStream) -> Result<(Version, isize, String), Error> {
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

fn read_body(stream: &mut TcpStream, len: usize) -> String {
    let mut parser = BodyParser::new(len);
    String::from_utf8(parser.read_req_component(stream)).unwrap_or(String::new())
}

pub fn read_request(stream: &mut TcpStream) -> Request {
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

            Some(len_field) => {
                match len_field {
                    &HeaderVal::Val(ref len_str) => {
                        let len = len_str.to_string().as_slice().parse::<usize>();
                        match len {
                            None => None,
                            Some(len) => Some(read_body(stream, len))
                        }
                    }
                    _ => None
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

pub fn read_response(stream: &mut TcpStream) -> Response {
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

                    Some(len_field) => {
                        match len_field {
                            &HeaderVal::Val(ref len_str) => {
                                let len = len_str.to_string().as_slice().parse::<usize>();
                                match len {
                                    None => None,
                                    Some(len) => Some(read_body(stream, len))
                                }
                            }
                            _ => None
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
