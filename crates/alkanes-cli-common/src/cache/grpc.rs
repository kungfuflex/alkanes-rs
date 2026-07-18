//! gRPC-backed [`AlkanesCache`] shim.
//!
//! This is a *client-side* adapter: an [`AlkanesCache`] implementation that
//! forwards every call to a remote cache server over gRPC. The intended
//! consumer is `subfrost-mobile`, which already runs a Redis-backed cache
//! inside its API pod — the mobile app and now the CLI can share that
//! cache via a thin gRPC tunnel, so multiple clients see consistent
//! state.
//!
//! The proto schema is intentionally minimal — three RPCs (Get / Put /
//! OnReorg) — so the server side stays trivial to implement. We define
//! the schema and types inline (no `.proto` file in this commit) to keep
//! this stand-alone; consumers can wire in their own proto file later
//! and the trait surface won't change.
//!
//! Available behind the `cache-grpc` feature.

use core::time::Duration;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint};
use tonic::{IntoRequest, Request, Status};

use super::{AlkanesCache, BlockHash, Bytes, CacheError, CacheKey, CacheResult, CacheScope};

/// Wire types — intentionally hand-rolled (not protoc-generated) so this
/// module doesn't require build.rs / tonic-build at compile time. A real
/// deployment will replace these with prost-generated types.
pub mod proto {
    use prost::Message;

    #[derive(Clone, PartialEq, Message)]
    pub struct CacheGetRequest {
        #[prost(string, tag = "1")]
        pub namespace: prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub network: prost::alloc::string::String,
        #[prost(string, tag = "3")]
        pub scope_tag: prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "4")]
        pub key_suffix: prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", optional, tag = "5")]
        pub scope_key: ::core::option::Option<prost::alloc::vec::Vec<u8>>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct CacheGetResponse {
        #[prost(bool, tag = "1")]
        pub hit: bool,
        #[prost(bytes = "vec", tag = "2")]
        pub value: prost::alloc::vec::Vec<u8>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct CachePutRequest {
        #[prost(string, tag = "1")]
        pub namespace: prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub network: prost::alloc::string::String,
        #[prost(string, tag = "3")]
        pub scope_tag: prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "4")]
        pub key_suffix: prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", optional, tag = "5")]
        pub scope_key: ::core::option::Option<prost::alloc::vec::Vec<u8>>,
        #[prost(bytes = "vec", tag = "6")]
        pub value: prost::alloc::vec::Vec<u8>,
        #[prost(int64, optional, tag = "7")]
        pub ttl_secs: ::core::option::Option<i64>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct CachePutResponse {}

    #[derive(Clone, PartialEq, Message)]
    pub struct CacheOnReorgRequest {
        #[prost(bytes = "vec", tag = "1")]
        pub new_tip: prost::alloc::vec::Vec<u8>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct CacheOnReorgResponse {}
}

/// Trait the gRPC client (real tonic-generated or test mock) must satisfy.
///
/// Splitting this out lets unit tests drive [`GrpcCache`] with an in-process
/// implementation rather than spinning up a real gRPC server.
#[async_trait]
pub trait GrpcCacheTransport: Send + Sync {
    async fn get(
        &self,
        req: proto::CacheGetRequest,
    ) -> Result<proto::CacheGetResponse, Status>;
    async fn put(
        &self,
        req: proto::CachePutRequest,
    ) -> Result<proto::CachePutResponse, Status>;
    async fn on_reorg(
        &self,
        req: proto::CacheOnReorgRequest,
    ) -> Result<proto::CacheOnReorgResponse, Status>;
}

/// gRPC-backed cache. Generic over the transport so the same surface is
/// usable both with a real `tonic::Channel`-based client and with an
/// in-memory test double.
#[derive(Clone)]
pub struct GrpcCache<T: GrpcCacheTransport> {
    transport: Arc<T>,
}

impl<T: GrpcCacheTransport> core::fmt::Debug for GrpcCache<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GrpcCache").finish()
    }
}

impl<T: GrpcCacheTransport> GrpcCache<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(transport),
        }
    }
}

#[async_trait]
impl<T: GrpcCacheTransport + 'static> AlkanesCache for GrpcCache<T> {
    async fn get(&self, key: &CacheKey, scope: &CacheScope) -> CacheResult<Option<Bytes>> {
        let req = proto::CacheGetRequest {
            namespace: key.namespace.to_string(),
            network: key.network.clone(),
            scope_tag: scope.tag().to_string(),
            key_suffix: key.suffix.clone(),
            scope_key: scope.scope_bytes().map(|b| b.to_vec()),
        };
        let resp = self
            .transport
            .get(req)
            .await
            .map_err(|s| CacheError::Backend(format!("grpc get: {s}")))?;
        Ok(if resp.hit { Some(resp.value) } else { None })
    }

    async fn put(
        &self,
        key: &CacheKey,
        scope: &CacheScope,
        value: Bytes,
        ttl: Option<Duration>,
    ) -> CacheResult<()> {
        let req = proto::CachePutRequest {
            namespace: key.namespace.to_string(),
            network: key.network.clone(),
            scope_tag: scope.tag().to_string(),
            key_suffix: key.suffix.clone(),
            scope_key: scope.scope_bytes().map(|b| b.to_vec()),
            value,
            ttl_secs: ttl.map(|d| d.as_secs() as i64),
        };
        self.transport
            .put(req)
            .await
            .map_err(|s| CacheError::Backend(format!("grpc put: {s}")))?;
        Ok(())
    }

    async fn on_reorg(&self, new_tip: BlockHash) -> CacheResult<()> {
        let req = proto::CacheOnReorgRequest {
            new_tip: new_tip.to_vec(),
        };
        self.transport
            .on_reorg(req)
            .await
            .map_err(|s| CacheError::Backend(format!("grpc on_reorg: {s}")))?;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "grpc"
    }
}

/// Standard tonic-based transport for production use. Pointed at any
/// gRPC server that implements the `AlkanesCacheService` proto contract
/// described in [`proto`]. The actual `.proto`-generated client lives in
/// the consuming crate (e.g. `subfrost-mobile`); here we expose just the
/// endpoint plumbing.
///
/// Wrapped in `Mutex` so multiple async tasks can share one channel.
pub struct TonicChannelTransport {
    _channel: Mutex<Channel>,
}

impl TonicChannelTransport {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, Status> {
        let url: String = endpoint.into();
        let ep = Endpoint::from_shared(url.clone())
            .map_err(|e| Status::invalid_argument(format!("bad endpoint {url}: {e}")))?
            .connect_timeout(Duration::from_secs(5));
        let channel = ep
            .connect()
            .await
            .map_err(|e| Status::unavailable(format!("connect {url}: {e}")))?;
        Ok(Self {
            _channel: Mutex::new(channel),
        })
    }
}

#[async_trait]
impl GrpcCacheTransport for TonicChannelTransport {
    async fn get(
        &self,
        _req: proto::CacheGetRequest,
    ) -> Result<proto::CacheGetResponse, Status> {
        // Concrete RPC dispatch is provided by the consumer's tonic-generated
        // client (e.g. `AlkanesCacheServiceClient::get(channel, req)`). This
        // method exists for the trait surface and to make wiring obvious;
        // production callers should implement `GrpcCacheTransport` directly
        // on their generated client.
        Err(Status::unimplemented(
            "TonicChannelTransport is a placeholder — implement GrpcCacheTransport \
             on your tonic-generated AlkanesCacheServiceClient instead",
        ))
    }

    async fn put(
        &self,
        _req: proto::CachePutRequest,
    ) -> Result<proto::CachePutResponse, Status> {
        Err(Status::unimplemented(
            "TonicChannelTransport is a placeholder",
        ))
    }

    async fn on_reorg(
        &self,
        _req: proto::CacheOnReorgRequest,
    ) -> Result<proto::CacheOnReorgResponse, Status> {
        Err(Status::unimplemented(
            "TonicChannelTransport is a placeholder",
        ))
    }
}

// Silence unused-import warnings for `IntoRequest`/`Request` — they're part
// of the public surface a real client implementation will use.
#[allow(dead_code)]
fn _unused_imports() {
    fn _f<T>(_r: T)
    where
        T: IntoRequest<u32>,
    {
    }
    fn _g(_r: Request<u32>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// In-process transport that mimics a remote server. Lets us drive the
    /// full GrpcCache surface without spinning up a real gRPC stack.
    #[derive(Default)]
    struct InProcTransport {
        // (namespace, network, scope_tag, suffix, scope_key) -> (value, inserted_secs, ttl_secs)
        store: StdMutex<
            std::collections::HashMap<
                (String, String, String, Vec<u8>, Option<Vec<u8>>),
                (Vec<u8>, i64, Option<i64>),
            >,
        >,
    }

    #[async_trait]
    impl GrpcCacheTransport for InProcTransport {
        async fn get(
            &self,
            req: proto::CacheGetRequest,
        ) -> Result<proto::CacheGetResponse, Status> {
            let store = self.store.lock().unwrap();
            let key = (
                req.namespace,
                req.network,
                req.scope_tag,
                req.key_suffix,
                req.scope_key,
            );
            Ok(match store.get(&key) {
                Some((v, _, _)) => proto::CacheGetResponse {
                    hit: true,
                    value: v.clone(),
                },
                None => proto::CacheGetResponse::default(),
            })
        }

        async fn put(
            &self,
            req: proto::CachePutRequest,
        ) -> Result<proto::CachePutResponse, Status> {
            let mut store = self.store.lock().unwrap();
            store.insert(
                (
                    req.namespace,
                    req.network,
                    req.scope_tag,
                    req.key_suffix,
                    req.scope_key,
                ),
                (req.value, 0, req.ttl_secs),
            );
            Ok(proto::CachePutResponse::default())
        }

        async fn on_reorg(
            &self,
            req: proto::CacheOnReorgRequest,
        ) -> Result<proto::CacheOnReorgResponse, Status> {
            let mut store = self.store.lock().unwrap();
            store.retain(|(_, _, scope, _, sk), _| {
                scope != "tip" || sk.as_deref() == Some(req.new_tip.as_slice())
            });
            Ok(proto::CacheOnReorgResponse::default())
        }
    }

    #[tokio::test]
    async fn grpc_cache_roundtrips_through_transport() {
        let cache = GrpcCache::new(InProcTransport::default());
        let key = CacheKey::new("getbytecode", "mainnet", b"4:512".to_vec());
        cache
            .put(&key, &CacheScope::Immutable, b"wasmbody".to_vec(), None)
            .await
            .unwrap();
        let got = cache.get(&key, &CacheScope::Immutable).await.unwrap();
        assert_eq!(got.as_deref(), Some(b"wasmbody".as_ref()));
    }

    #[tokio::test]
    async fn grpc_cache_honors_reorg() {
        let cache = GrpcCache::new(InProcTransport::default());
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        cache
            .put(
                &CacheKey::new("sim", "mainnet", b"a".to_vec()),
                &CacheScope::Tip(h1),
                b"old".to_vec(),
                None,
            )
            .await
            .unwrap();
        cache
            .put(
                &CacheKey::new("sim", "mainnet", b"b".to_vec()),
                &CacheScope::Tip(h2),
                b"new".to_vec(),
                None,
            )
            .await
            .unwrap();
        cache.on_reorg(h2).await.unwrap();
        let a = cache
            .get(
                &CacheKey::new("sim", "mainnet", b"a".to_vec()),
                &CacheScope::Tip(h1),
            )
            .await
            .unwrap();
        let b = cache
            .get(
                &CacheKey::new("sim", "mainnet", b"b".to_vec()),
                &CacheScope::Tip(h2),
            )
            .await
            .unwrap();
        assert!(a.is_none());
        assert_eq!(b.as_deref(), Some(b"new".as_ref()));
    }
}
