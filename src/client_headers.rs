use reqwest::header::USER_AGENT;
use reqwest::RequestBuilder;

pub const CLIENT_NAME: &str = env!("CARGO_PKG_NAME");
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const USER_AGENT_VALUE: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
pub const CLIENT_NAME_HEADER: &str = "X-AppSignal-Client";
pub const CLIENT_VERSION_HEADER: &str = "X-AppSignal-Client-Version";

pub fn with_user_agent(request: RequestBuilder) -> RequestBuilder {
    request.header(USER_AGENT, USER_AGENT_VALUE)
}

pub fn with_appsignal_headers(request: RequestBuilder) -> RequestBuilder {
    with_user_agent(request)
        .header(CLIENT_NAME_HEADER, CLIENT_NAME)
        .header(CLIENT_VERSION_HEADER, CLIENT_VERSION)
}
