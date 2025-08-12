pub use de::{from_reader, from_str};
pub use error::{Error, Result};
pub use se::{to_string, to_writer, to_writer_pretty};

mod de;
mod error;
mod se;
#[cfg(test)]
mod test;
