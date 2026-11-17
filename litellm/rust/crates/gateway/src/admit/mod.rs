use axum::extract::Request;

/// Why the gateway does not serve a request the way the Python proxy would.
#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
pub enum Refused {
    #[error("the gateway has not read the proxy's configuration")]
    #[strum(serialize = "configuration")]
    Configuration,
}

/// Admits the requests the gateway serves the way the Python proxy would, and refuses every other
/// one, giving it back as the client sent it, with why.
// TODO: this refuses every request for now, and will be expanded later to admit what the native app serves
pub async fn admit(request: Request) -> Result<Request, (Box<Request>, Refused)> {
    Err((Box::new(request), Refused::Configuration))
}
