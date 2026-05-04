use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("placeholder")]
    Todo,
}
