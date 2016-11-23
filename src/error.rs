// Copyright (c) 2016 Brandon Thomas <bt@brand.io, echelon@gmail.com>

use std::io::Error as IoError;

/// Represents all of the various types of errors reported by the wemo.rs
/// library. (TODO: Work in progress unifying errors.)
#[derive(Debug)]
pub enum WemoError {
  /// Indicates that there was trouble understanding the WeMo device response.
  BadResponseError,

  /// Wraps an std::io::Error.
  IoError { cause: IoError },

  /// Couldn't parse the XML received from Wemo.
  ParsingError,

  /// Indicates that a communication timeout elapsed.
  TimeoutError,

  /// Indicates that the WeMo reported a problem during the request.
  WemoError,
}

impl From<IoError> for WemoError {
  fn from(error: IoError) -> WemoError {
    WemoError::IoError { cause: error }
  }
}
