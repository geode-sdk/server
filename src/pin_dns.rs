use std::net::{SocketAddr, ToSocketAddrs};

use reqwest::dns::{Name, Resolve, Resolving};

tokio::task_local! {
    /// Used to pin IP addresses for the resolver.
    ///
    /// # Example (assumes you have an http client that uses PinDnsResolver)
    ///
    /// ```rust
    /// // Let's assume addrs are addresses we confirmed are safe to call.
    /// let addrs = vec![std::net::SocketAddr::new("127.0.0.1", port)];
    ///
    /// let response = PINNED_ADDRS.scope(addrs, async {
    ///     http_client.get(url).send().await
    /// }).await?;
    /// ```
    ///
    /// Make sure to only run **the one request you need** inside the callback.
    /// This design is a little brittle, but works for our purposes.
    pub static PINNED_ADDRS: Vec<SocketAddr>;
}

#[derive(Clone, Default)]
pub struct PinDnsResolver;

impl Resolve for PinDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            tracing::debug!("resolving {} with PinDnsResolver", name.as_str());

            if let Ok(addrs) = PINNED_ADDRS.try_with(|c| c.clone()) {
                tracing::debug!("pinned {} to {:?} successfully", name.as_str(), addrs);
                return Ok(
                    Box::new(addrs.into_iter()) as Box<dyn Iterator<Item = SocketAddr> + Send>
                );
            }

            tracing::debug!("didn't have a PINNED_ADDRS, falling back to OS resolver");
            // Fallback to the usual OS resolver otherwise
            (name.as_str(), 0)
                .to_socket_addrs()
                .map(|it| Box::new(it) as Box<dyn Iterator<Item = SocketAddr> + Send>)
                .map_err(|e| e.into())
        })
    }
}
