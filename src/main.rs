extern crate epic;

use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::str::from_utf8;
use std::collections::HashMap;
use std::thread::Thread;

fn main() {
    let mut acceptor = TcpListener::bind("127.0.0.1:8482").listen().unwrap();

    Thread::spawn(move|| {
        for socket in acceptor.incoming() {
            match socket {
                Ok(mut stream) => {
                    let req = epic::http::parser::read_request(&mut stream);
                    println!("Req: {:?}", req);

                    // Write something back
                    stream.write(b"HTTP/1.1 200 VERY OK\r\nContent-Type: text/plain\r\nContent-Length:12\r\n\r\nHello");
                    stream.write(b" world!");
                }
                // Err(ref e) if e.kind == EndOfFile => break, // closed
                Err(e) => panic!("unexpected error: {}", e),
            }
        }
    });


    let mut stream = TcpStream::connect("127.0.0.1:8482").unwrap();
    stream.write(b"GET /index.html HTTP/1.1\r\nContent-Type: text/plain\r\nContent-Length:12\r\nTransfer-Encoding: gzip, chunked\r\n\r\nHello").unwrap();
    stream.write(b" world!").unwrap();
    println!("Client got: {:?}", epic::http::parser::read_response(&mut stream));
}
