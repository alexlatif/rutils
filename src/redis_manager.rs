use anyhow::Result;
use futures_util::stream::StreamExt;
use redis::{aio::MultiplexedConnection, aio::PubSub, AsyncCommands, Client, RedisError};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::time::{timeout, Duration};

static MAX_POOL_SIZE: usize = 100;

#[derive(Clone)]
pub struct RedisManager {
    client: Arc<Client>,
    sync_connection_pool: Arc<Mutex<VecDeque<redis::Connection>>>,
    async_connection_pool: Arc<Mutex<VecDeque<MultiplexedConnection>>>,
    pubsub_connection_pool: Arc<Mutex<VecDeque<PubSub>>>,
}

impl RedisManager {
    pub fn new(redis_url: &str) -> Result<Self, RedisError> {
        let client = Arc::new(Client::open(redis_url)?);
        let sync_connection_pool = Arc::new(Mutex::new(VecDeque::new()));
        let async_connection_pool = Arc::new(Mutex::new(VecDeque::new()));
        let pubsub_connection_pool = Arc::new(Mutex::new(VecDeque::new()));

        Ok(Self {
            client,
            sync_connection_pool,
            async_connection_pool,
            pubsub_connection_pool,
        })
    }

    pub fn get_sync_connection(&self) -> Result<redis::Connection, RedisError> {
        let conn = {
            let mut pool = self.sync_connection_pool.lock().unwrap();
            pool.pop_front()
        };

        if let Some(conn) = conn {
            Ok(conn)
        } else {
            self.client.get_connection()
        }
    }

    async fn get_async_connection(&self) -> Result<MultiplexedConnection, RedisError> {
        let conn = {
            let mut pool = self.async_connection_pool.lock().unwrap();
            pool.pop_front()
        };

        if let Some(conn) = conn {
            Ok(conn)
        } else {
            self.client.get_multiplexed_async_connection().await
        }
    }

    #[allow(dead_code)]
    fn return_sync_connection(&self, conn: redis::Connection) {
        let mut pool = self.sync_connection_pool.lock().unwrap();
        if pool.len() < MAX_POOL_SIZE {
            pool.push_back(conn);
        }
    }

    async fn return_async_connection(&self, conn: MultiplexedConnection) {
        let mut pool = self.async_connection_pool.lock().unwrap();
        if pool.len() < MAX_POOL_SIZE {
            pool.push_back(conn);
        }
    }

    async fn get_pubsub_connection(&self) -> Result<PubSub, RedisError> {
        let conn = {
            let mut pool = self.pubsub_connection_pool.lock().unwrap();
            pool.pop_front()
        };

        if let Some(conn) = conn {
            Ok(conn)
        } else {
            self.client.get_async_pubsub().await
        }
    }

    async fn return_pubsub_connection(&self, conn: PubSub) {
        let mut pool = self.pubsub_connection_pool.lock().unwrap();
        if pool.len() < MAX_POOL_SIZE {
            pool.push_back(conn);
        }
    }

    pub async fn publish(&self, channel: &str, message: &str) -> Result<(), RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.publish(channel, message).await?;
        self.return_async_connection(conn).await;
        Ok(())
    }

    pub async fn subscribe_and_wait_for_response(
        &self,
        subscribe_channel: &str,
        timeout_duration: Duration,
    ) -> Result<String, RedisError> {
        let mut conn = self.get_pubsub_connection().await?;
        conn.subscribe(subscribe_channel).await?;

        let mut pubsub_stream = conn.on_message();
        let response = match timeout(timeout_duration, pubsub_stream.next()).await {
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
        };

        drop(pubsub_stream); // Explicitly drop the stream
        conn.unsubscribe(subscribe_channel).await?;
        self.return_pubsub_connection(conn).await;

        response
    }
}
