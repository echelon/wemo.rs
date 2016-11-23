// Copyright (c) 2016 Brandon Thomas <bt@brand.io, echelon@gmail.com>

use error::WemoError;
use iron::Handler;
use iron::Listening;
use iron::prelude::*;
use iron::request::Body;
use iron::status;
use net::ssdp::UPNP_PORT;
use std::collections::HashMap;
use std::io::Error as ioError;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::net::TcpStream;
use std::net::UdpSocket;
use parsing::parse_state;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread::JoinHandle;
use std::thread::Thread;
use std::thread;
use std::time::Duration;
use urlencoded::UrlEncodedQuery;

#[derive(Default)]
struct Subscription {
  notification_count: u64,
  subscribed_on: u8, // TODO
  expires_on: u8, // TODO
  enabled: bool,
}

/// Subscriptions objects manage Wemo device event notifications. You can
/// register subscriptions against multiple devices; an Iron HTTP server will
/// be started to receive callback notifications from the Wemo devices, and a
/// background thread will handle subscription management. You should only
/// ever need one of these objects.
pub struct Subscriptions {
  callback_port: u16,
  subscription_ttl_sec: u16,
  server_handle: Option<Listening>,
  polling_handle: Option<JoinHandle<Thread>>,
  continue_polling: bool,
  subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
}

/*
subs = Subscriptions::new(port, subscription_ttl);

subs.subscribe("http://...")
subs.subscribe_callback("http://...", FnOnce)

subs.register_global_callback(key, FnOnce)
subs.unregister_global_callback(key)

subs.start_server(); // In its own thread.

subs.unsubscribe("http://...")
subs.subscribe("http://...")
*/

impl Subscriptions {
  /// CTOR.
  /// Set the callback port for the HTTP server that will be launched and the
  /// subscription TTL.
  pub fn new(callback_port: u16, subscription_ttl_sec: u16) -> Self {
    Subscriptions {
      callback_port: callback_port,
      subscription_ttl_sec: subscription_ttl_sec,
      server_handle: None,
      polling_handle: None,
      continue_polling: false,
      subscriptions: Arc::new(RwLock::new(HashMap::default()))
    }
  }

  /// Subscribe to push notifications from a Wemo device.
  /// This should be done after launching the server to avoid missing
  /// notifications.
  pub fn subscribe(&self, host: &str) -> Result<(), WemoError> {
    send_subscribe(host, self.subscription_ttl_sec, self.callback_port)?;
    self.register_subscription(host)?;
    Ok(())
  }

  /// Start the HTTP server so it can begin receiving push notifications. A
  /// background thread to resubscribe will also be launched. Calling this
  /// function is nonblocking, but it returns a thread guard that will
  /// automatically join with the parent once it is dropped.
  pub fn start_server(&mut self) -> Result<(), WemoError> {
    if self.server_handle.is_some() {
      return Ok(());
    }

    let subs = self.subscriptions.clone();

    let handler = move |request: &mut Request| -> IronResult<Response> {
      let hashmap = subs.read().unwrap();

      //let arc = req.get::<Read<Log>>().unwrap();
      //let log_path = arc.as_ref();

      //let arc = request.get::<Write<P>>(); // Arc<Mutex>>.


      //match request.get_ref::<UrlEncodedQuery>() {
      //  Ok(ref hashmap) => println!("Parsed GET request query string:\n {:?}", hashmap),
      //  Err(ref e) => println!("{:?}", e)
      //}

      // (USELESS) Header - HOST: 192.168.1.4:3000
      // (USELESS) Header - SID: uuid:2c89db26-1dd2-11b2-b45b-b4fc561fc6be
      //for header in request.headers.iter() {
      //  println!("Header - {}: {}", header.name(), header.value_string());
      //}

      let mut body = String::new();
      request.body.read_to_string(&mut body);

      if !body.contains("BinaryState") {
        return Ok(Response::with((status::Ok, "")));
      }

      match parse_state(&body) {
        Err(_) => println!("Parse Error"),
        Ok(state) => println!("State updated: {}", state),
      }

      Ok(Response::with((status::Ok, "")))
    };

    let listen_address = format!("0.0.0.0:{}", self.callback_port);

    let server = try!(Iron::new(handler).http(listen_address.as_str())
        .map_err(|_| WemoError::IronError));

    self.server_handle = Some(server);

    self.start_polling();

    Ok(())
  }

  /// Stop the HTTP server from running. Also stops resubscription process.
  /// Warning: This may not work the server from listening. See the following
  /// issue on Iron/Hyper: https://github.com/hyperium/hyper/issues/338
  pub fn stop_server(&mut self) -> Result<(), WemoError> {
    if self.server_handle.is_none() {
      return Ok(());
    }

    self.stop_polling();

    self.server_handle.as_mut().unwrap().close();
    self.server_handle = None;

    Ok(())
  }

  // Not threadsafe.
  fn start_polling(&mut self) {
    if self.polling_handle.is_some() {
      return;
    }

    let subscription_ttl_sec = self.subscription_ttl_sec;
    let callback_port = self.callback_port;
    let subscriptions = self.subscriptions.clone();

    let handle = thread::spawn(move || {
      loop {
        //thread::sleep(Duration::from_secs(300)); // 60 * 5
        thread::sleep(Duration::from_secs(30));
        println!("Resubscribing...");

        let subs = match subscriptions.read() {
          Err(_) => continue, // TODO: LOG
          Ok(subs) => subs,
        };

        // TODO: A single failure can hold things up, causing missed events
        // from temporarily dropped subscriptions. Also, I need to mitigate
        // change of ports (and IP addresses).
        for (host, subscription) in subs.iter() {
          println!("Resubscribe to {}.", host);
          send_subscribe(host, subscription_ttl_sec, callback_port);
        }
      }
    });

    self.continue_polling = true;
    self.polling_handle = Some(handle);
  }

  // Consume handle. Not threadsafe.
  fn stop_polling(&mut self) {
    self.polling_handle = None; // Drops handle.
    self.continue_polling = false;
  }

  fn register_subscription(&self, host: &str) -> Result<(), WemoError> {
    let mut subscription = Subscription::default();
    subscription.enabled = true;
    self.subscriptions.write().map_err(|_| WemoError::LockError)?
        .insert(host.to_string(), subscription);
    Ok(())
  }
}

// NB: Called from thread, can't reference 'self'.
fn send_subscribe(host: &str,
                  subscription_ttl_sec: u16,
                  callback_port: u16) -> Result<(), WemoError> {
  let local_ip = "192.168.1.4"; // TODO: Must get local IP address.

  let callback_url = format!("http://{}:{}/?from={}",
    local_ip, callback_port, host);

  let header = format!("\
      SUBSCRIBE /upnp/event/basicevent1 HTTP/1.1\r\n\
      CALLBACK: <{}>\r\n\
      NT: upnp:event\r\n\
      TIMEOUT: Second-{}\r\n\
      Host: {}\r\n\
      \r\n",
    callback_url,
    subscription_ttl_sec,
    host);

  println!("Sending request...");

  let mut stream = TcpStream::connect(host)?;

  stream.set_read_timeout(Some(Duration::from_secs(1)));
  stream.set_write_timeout(Some(Duration::from_secs(1)));

  let _ = stream.write(header.as_bytes()); // TODO: Timeout

  println!("...subscribed");

  // TODO: Read response.

  Ok(())
}