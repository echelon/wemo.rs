// Copyright (c) 2016 Brandon Thomas <bt@brand.io, echelon@gmail.com>

use device::state::WemoState;
use error::WemoError;
use get_if_addrs::IfAddr;
use get_if_addrs::get_if_addrs;
use iron::Iron;
use iron::IronError;
use iron::IronResult;
use iron::Listening;
use iron::Plugin;
use iron::Request;
use iron::Response;
use iron::status;
use parsing::parse_state;
use std::boxed::Box;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::net::TcpStream;
use std::ops::Fn;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread::JoinHandle;
use std::thread::Thread;
use std::thread;
use std::time::Duration;
use urlencoded::UrlEncodedQuery;

/// Individual subscription notifications.
pub struct Notification {
  pub notification_type: NotificationType,

  /// Original device subscribed to, in "IP:PORT" form.
  /// Note that the port may have been changed by the Wemo device, and that the
  /// IP could differ if the router changed it.
  pub subscription_key: String,
}

/// Each type of supported notification.
/// More may be added in the future.
pub enum NotificationType {
  State { state: WemoState }
}

struct Subscription {
  callback: Option<Box<Fn(Notification) + Sync + Send>>,
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
  /// The provided callback is invoked when notifications are received.
  /// This should be done after launching the server to avoid missing
  /// notifications.
  pub fn subscribe<F>(&self, host: &str, callback: F)
                      -> Result<(), WemoError>
                      where F: Fn(Notification) + Sync + Send + 'static {
    let local_ip = get_local_ip()?;

    send_subscribe(local_ip, host, self.subscription_ttl_sec,
        self.callback_port)?;

    let subscription = Subscription { callback: Some(Box::new(callback)) };

    self.register_subscription(host, subscription)?;
    Ok(())
  }

  /// Remove a subscription.
  pub fn unsubscribe(&self, host: &str) -> Result<(), WemoError> {
    self.subscriptions.write().map_err(|_| WemoError::LockError)?
        .remove(host);
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

    // TODO: Request headers contain a re-subscribe UUID, which should be used
    // instead of subscribing again without a subscription ID.
    let handler = move |request: &mut Request| -> IronResult<Response> {
      let mut body = String::new();
      request.body.read_to_string(&mut body)
          .map_err(|e| WemoError::IoError { cause: e })?;

      // Device is contained in a query string variable, "from".
      let host = request.get_ref::<UrlEncodedQuery>()
          .map_err(|_| WemoError::SubscriptionError)
          .and_then(|hashmap| hashmap.get("from")
              .and_then(|vec| vec.get(0))
              .ok_or(WemoError::SubscriptionError))?;

      if !body.contains("BinaryState") {
        // TODO: Handle other types of state update.
        return Ok(Response::with((status::Ok, "")));
      }

      let state = parse_state(&body)?;

      let subscriptions = subs.read()
          .map_err(|_| WemoError::SubscriptionError)?;

      let subscription = subscriptions.get(host)
          .ok_or(WemoError::SubscriptionError)?;

      if subscription.callback.is_some() {
        let callback = subscription.callback.as_ref().unwrap();
        let notification = Notification {
          notification_type: NotificationType::State {
            state: state,
          },
          subscription_key: host.to_string(),
        };
        callback(notification);
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

    self.server_handle.as_mut()
        .unwrap()
        .close()
        .map_err(|_| WemoError::IronError)?;

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

        let subs = match subscriptions.read() {
          Err(_) => continue, // TODO: LOG
          Ok(subs) => subs,
        };

        // TODO: A single failure can hold things up, causing missed events
        // from temporarily dropped subscriptions. Also, I need to mitigate
        // change of ports (and IP addresses).
        let local_ip = match get_local_ip() {
          Err(_) => continue, // TODO: LOG
          Ok(ip) => ip,
        };

        for (host, _subscription) in subs.iter() {
          let _r = send_subscribe(local_ip, host, subscription_ttl_sec,
              callback_port);
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

  fn register_subscription(&self, host: &str, subscription: Subscription)
                           -> Result<(), WemoError> {
    self.subscriptions.write().map_err(|_| WemoError::LockError)?
        .insert(host.to_string(), subscription);
    Ok(())
  }
}

// NB: Called from thread, can't reference 'self'.
pub fn send_subscribe(local_ip: IpAddr,
                      host: &str,
                      subscription_ttl_sec: u16,
                      callback_port: u16) -> Result<(), WemoError> {
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

  let mut stream = TcpStream::connect(host)?;

  stream.set_read_timeout(Some(Duration::from_secs(1)))?;
  stream.set_write_timeout(Some(Duration::from_secs(1)))?;

  stream.write(header.as_bytes())?;

  // TODO: Read response.

  Ok(())
}

/// Attempt to get the local IP address on the network.
/// Returns the first non-loopback, local Ipv4 network interface.
pub fn get_local_ip() -> Result<IpAddr, WemoError> {
  // TODO: Get rid of this dependency. Didn't realize it was GPL.
  let ips = get_if_addrs()?;

  // Only non-loopback Ipv4 addresses that aren't docker interfaces.
  let filtered = ips.iter()
      .filter(|x| match x.addr { IfAddr::V4(..) => true, _ => false } )
      .filter(|x| !x.addr.is_loopback())
      .filter(|x| !x.name.contains("docker"))
      .collect::<Vec<_>>();

  filtered.get(0)
      .ok_or(WemoError::NoLocalIp)
      .map(|x| x.addr.ip())
}

impl From<WemoError> for IronError {
  fn from(error: WemoError) -> IronError {
    let response = Response::with((status::InternalServerError, "Error"));
    IronError {
      error: Box::new(error),
      response: response,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::io::Read;
  use std::net::IpAddr;
  use std::net::Ipv4Addr;
  use std::net::SocketAddr;
  use std::net::SocketAddrV4;
  use std::net::TcpListener;
  use std::thread;
  use super::*;

  fn next_test_port() -> u16 {
    // Taken from rust-utp, since `std::net::test` not available.
    use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
    static NEXT_OFFSET: AtomicUsize = ATOMIC_USIZE_INIT;
    const BASE_PORT: u16 = 9600;
    BASE_PORT + NEXT_OFFSET.fetch_add(1, Ordering::Relaxed) as u16
  }

  fn next_test_ip4() -> SocketAddr {
    // Taken from rust standard library tests.
    let port = next_test_port();
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port))
  }

  #[test]
  fn test_send_subscribe() {
    let socket_addr = next_test_ip4();
    let listener = TcpListener::bind(&socket_addr).unwrap();
    let host = format!("localhost:{}", socket_addr.port());

    thread::spawn(move || {
      let local_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
      send_subscribe(local_ip, &host, 600, 8080).unwrap();
    });

    let mut stream = listener.accept().unwrap().0;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();

    let expected = format!("\
      SUBSCRIBE /upnp/event/basicevent1 HTTP/1.1\r\n\
      CALLBACK: <http://127.0.0.1:8080/?from=localhost:{}>\r\n\
      NT: upnp:event\r\n\
      TIMEOUT: Second-600\r\n\
      Host: localhost:{}\r\n\
      \r\n",
        socket_addr.port(),
        socket_addr.port());

    assert_eq!(buf, expected);
  }
}
