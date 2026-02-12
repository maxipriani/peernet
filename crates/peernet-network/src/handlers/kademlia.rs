use crate::state::{NetworkState, PendingQuery};
use libp2p::kad;
use peernet_core::{DhtValue, NetworkEvent, PeerId};

pub struct KademliaHandler;

impl KademliaHandler {
    pub async fn handle(state: &mut NetworkState, event: kad::Event) {
        match event {
            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                Self::handle_query_result(state, id, result).await;
            }
            kad::Event::RoutingUpdated { peer, .. } => {
                state
                    .emit(NetworkEvent::RoutingUpdated { peer_id: peer })
                    .await;
            }
            _ => {}
        }
    }

    async fn handle_query_result(
        state: &mut NetworkState,
        id: kad::QueryId,
        result: kad::QueryResult,
    ) {
        match result {
            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))) => {
                if let Some(PendingQuery::GetRecord(key)) = state.complete_query(&id) {
                    let value =
                        DhtValue::new(record.record.value).unwrap_or_else(|_| DhtValue::empty());
                    state.emit(NetworkEvent::RecordFound { key, value }).await;
                }
            }
            kad::QueryResult::GetRecord(Err(_)) => {
                if let Some(PendingQuery::GetRecord(key)) = state.complete_query(&id) {
                    state.emit(NetworkEvent::RecordNotFound { key }).await;
                }
            }
            kad::QueryResult::PutRecord(Ok(kad::PutRecordOk { key: _ })) => {
                if let Some(PendingQuery::PutRecord(dht_key)) = state.complete_query(&id) {
                    state
                        .emit(NetworkEvent::RecordStored { key: dht_key })
                        .await;
                }
            }
            kad::QueryResult::PutRecord(Err(err)) => {
                if let Some(PendingQuery::PutRecord(key)) = state.complete_query(&id) {
                    state
                        .emit(NetworkEvent::RecordStoreFailed {
                            key,
                            reason: format!("{err:?}"),
                        })
                        .await;
                }
            }
            kad::QueryResult::StartProviding(Ok(kad::AddProviderOk { key: _ })) => {
                if let Some(PendingQuery::StartProviding(key)) = state.complete_query(&id) {
                    state.emit(NetworkEvent::ProviderRecordStored { key }).await;
                }
            }
            kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                providers,
                ..
            })) => {
                if let Some(PendingQuery::GetProviders(key)) = state.complete_query(&id) {
                    let providers: Vec<PeerId> = providers.into_iter().collect();
                    state
                        .emit(NetworkEvent::ProvidersFound { key, providers })
                        .await;
                }
            }
            _ => {}
        }
    }
}
