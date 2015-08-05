// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

// TODO: These enum names suck. Update them.
/// Errors updating Wemo
pub enum WemoError {
  // TODO: new

  /// Indicates that there was trouble understanding the WeMo device response.
  BadResponseError,

  /// Indicates that a networking error occurred.
  NetworkError,

  /// Indicates that a communication timeout elapsed.
  TimeoutError,

  /// Indicates that the WeMo reported a problem during the request.
  WemoError,
}

