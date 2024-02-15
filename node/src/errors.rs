#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error writing to stdout")]
    WriteError,
	#[error("File Not Found")]
	BadFile,
    #[error("Error reading from stdout")]
    ReadError
}
