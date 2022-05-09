#[derive(Debug)]
pub enum Error {
    BadConnection(tungstenite::Error),

    BadData(serde_json::Error)
}

impl From<tungstenite::Error> for Error {
    fn from(e: tungstenite::Error) -> Self {
        Self::BadConnection(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::BadData(e)
    }
}