// Copyright (c) 2015-2016 Brandon Thomas <bt@brand.io>

use mio::tcp::{Shutdown, TcpStream};
use mio::{EventLoop, Handler, EventSet, PollOpt, Token};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr};

const CLIENT: Token = Token(0);
const TIMEOUT: Token = Token(1);

/// Represents a SOAP request to a WeMo device.
#[derive(Clone)]
pub struct SoapRequest {
  pub request_path: String,
  pub soap_action: String,
  pub http_post_payload: String,
}

/// An HTTP client for making SOAP requests.
pub struct SoapClient {
  stream_socket: TcpStream,
  soap_request: Option<SoapRequest>,
  soap_response: Option<String>,
}

impl SoapClient {
  pub fn connect(remote_ip_addr: IpAddr, port: u16) -> Option<SoapClient> {
    let socket = SocketAddr::new(remote_ip_addr, port);

    match TcpStream::connect(&socket) {
      Err(_) => { None },
      Ok(stream_socket) => {
        stream_socket.set_keepalive(None).unwrap();

        Some(SoapClient {
          stream_socket: stream_socket,
          soap_request: None,
          soap_response: None,
        })
      }
    }
  }

  /// Make a synchronous SOAP HTTP request and return the raw response.
  pub fn post(&mut self, soap_request: SoapRequest, timeout_ms: u64)
      -> Option<String> {
    self.soap_request = Some(soap_request);

    let mut event_loop = EventLoop::new().unwrap();

    event_loop.timeout_ms(TIMEOUT, timeout_ms).unwrap();

    event_loop.register(&self.stream_socket, CLIENT, EventSet::writable(),
                        PollOpt::edge()).unwrap();

    event_loop.run(self).unwrap();

    self.soap_response.take()
  }

  /// Perform the SOAP HTTP request.
  fn write_request(&mut self, event_loop: &mut EventLoop<SoapClient>) {
    let header = {
      let request = match self.soap_request.as_ref() {
        Some(req) => { req },
        None => { return; },
      };

      format!("\
          POST {} HTTP/1.1\r\n\
          Content-Type: text/xml; charset=\"utf-8\"\r\n\
          Accept:\r\n\
          SOAPACTION: \"{}\"\r\n\
          Content-Length: {}\r\n\
          \r\n\
          {}",
          &request.request_path,
          &request.soap_action,
          &request.http_post_payload.len(),
          &request.http_post_payload)
    };

    match self.stream_socket.write_all(&mut header.as_bytes()) {
      Err(_) => {
        debug!(target: "wemo", "error writing socket");
      },
      Ok(_) => {
        event_loop.deregister(&self.stream_socket).unwrap();
        event_loop.register(&self.stream_socket, CLIENT, EventSet::readable(),
                                PollOpt::edge()).unwrap();

        self.soap_request = None;
      },
    }
  }

  /// Read and save the HTTP response.
  fn read_response(&mut self, event_loop: &mut EventLoop<SoapClient>) {
    let mut buf = String::new();
    let result = self.stream_socket.read_to_string(&mut buf);

    match result {
      Err(e) => {
        debug!(target: "wemo", "error reading socket: {:?}", e);
      },
      Ok(_) => {
        self.soap_response = Some(buf.clone());
        event_loop.shutdown();
      },
    }
  }
}

impl Handler for SoapClient {
  type Timeout = Token;
  type Message = ();

  /// Handle events on the socket.
  fn ready(&mut self, event_loop: &mut EventLoop<SoapClient>, _token: Token,
           events: EventSet) {
    if events.is_readable() {
      self.read_response(event_loop);
    } else if events.is_writable() {
      self.write_request(event_loop);
    }
  }

  /// Timeout the SOAP HTTP request.
  fn timeout(&mut self, event_loop: &mut EventLoop<SoapClient>,
             _token: Token) {
    debug!(target: "wemo", "SoapClient received timeout");
    self.stream_socket.shutdown(Shutdown::Both).unwrap();
    event_loop.shutdown();
  }
}
