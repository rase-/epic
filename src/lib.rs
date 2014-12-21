use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::io::IoResult;
use std::str::from_utf8;
use std::collections::HashMap;

#[deriving(Show)]
pub enum Errors {
    VersionParseError
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
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
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

    println!("Parser result: {}", parser.buf);
    return parser.buf;
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
                    loop {
                        let res = read_request(&mut stream);
                        let slice = res.as_slice();
                        println!("Parsed: {}", from_utf8(slice).unwrap_or(""));

                        // Emitting to client
                        stream.write(slice);
                    }
                })
            }
        }
    });

    spawn(proc() {
        let mut stream = TcpStream::connect("127.0.0.1:3000");
        stream.write(b"Content-Type: text/html\r\n").unwrap();

        let mut buf = [0u8, ..4096];
        let count = stream.read(&mut buf);
        let msg = from_utf8(&buf).unwrap_or("");
        println!("Client got: {}", msg);
    });
}
