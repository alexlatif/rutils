use anyhow::Result;
use futures::StreamExt;
use redis::{aio::MultiplexedConnection, AsyncCommands, Client, Connection, RedisError};
use tokio::time::{timeout, Duration};
// use tracing::{error, info, Level};

// #[derive(Clone)]
pub struct RedisManager {
    client: Client,
    pub connection: Connection,
    pub async_connection: Option<MultiplexedConnection>,
}

impl RedisManager {
    pub fn new(redis_url: &str, use_async: bool) -> Result<Self, RedisError> {
        let client = Client::open(redis_url)?;

        let connection = client.get_connection()?;

        let mut async_connection = None;
        if use_async {
            async_connection = Some(
                tokio::runtime::Runtime::new()?
                    .block_on(client.get_multiplexed_async_connection())?,
            );
        }

        Ok(Self {
            client,
            connection,
            async_connection,
        })
    }

    pub async fn publish(&self, channel: &str, message: &str) -> Result<(), RedisError> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        conn.publish(channel, message).await?;
        Ok(())
    }

    pub async fn subscribe_and_wait_for_response(
        &self,
        subscribe_channel: &str,
        timeout_duration: Duration,
    ) -> Result<String, RedisError> {
        let mut conn = self.client.get_async_pubsub().await.unwrap();

        let _: () = conn.subscribe(subscribe_channel).await.unwrap();
        let mut pubsub_stream = conn.on_message();

        match timeout(timeout_duration, pubsub_stream.next()).await {
            Ok(Some(msg)) => {
                let payload: String = msg.get_payload()?;
                Ok(payload)
            }
            Ok(None) => Err(RedisError::from((
                redis::ErrorKind::IoError,
                "No message received",
            ))),
            Err(_) => Err(RedisError::from((
                redis::ErrorKind::IoError,
                "Timeout waiting for response",
            ))),
        }
    }
}
