use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fmt::Debug;
use std::fmt::Write;
use std::sync::Arc;
use std::thread::sleep;
use tokio::sync::Notify;
use tracing::field::Visit;
use tracing::instrument;
use tracing::instrument::WithSubscriber;
use tracing::{info, span, Level};
use tracing_core::Field;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::registry::SpanRef;
use tracing_subscriber::registry::{LookupSpan, Registry};
use tracing_subscriber::Layer as _;
use tracing_subscriber::{filter::EnvFilter, fmt};

use crate::errors::RResult;
use crate::prelude::*;
use crate::redis_manager::RedisManager;

#[derive(Serialize, Deserialize, Debug)]
struct LogData {
    timestamp: String,
    level: String,
    message: String,
    span_id: Option<String>,
    trace_id: String,
    span_name: Option<String>,
}

struct RedisLogger {
    app_name: String,
    manager: Arc<RedisManager>,
    notify: Arc<Notify>,
}

impl RedisLogger {
    fn new(app_name: String, manager: Arc<RedisManager>) -> Self {
        RedisLogger {
            app_name,
            manager,
            notify: Arc::new(Notify::new()),
        }
    }

    // fn log(&self, log_data: LogData) {
    //     let manager = self.manager.clone();
    //     let app_name = self.app_name.clone();
    //     let notify = self.notify.clone();
    //     tokio::spawn(async move {
    //         let mut con = manager
    //             .get_async_connection()
    //             .await
    //             .expect("Failed to get Redis connection");

    //         let key = match (&log_data.span_id, &log_data.span_name) {
    //             (Some(span_id), Some(span_name)) => format!(
    //                 "traces:{}:{}:{}:{}",
    //                 app_name, log_data.trace_id, span_name, span_id
    //             ),
    //             _ => format!("traces:{}:{}", app_name, log_data.trace_id),
    //         };

    //         let timestamp = chrono::DateTime::parse_from_rfc3339(&log_data.timestamp)
    //             .unwrap()
    //             .timestamp_millis();

    //         let _: () = con
    //             // .rpush(key, serde_json::to_string(&log_data).unwrap())
    //             .zadd(key, serde_json::to_string(&log_data).unwrap(), timestamp)
    //             .await
    //             .unwrap();

    //         manager.return_async_connection(con).await;
    //         notify.notify_one();
    //     });
    // }
    fn log(&self, log_data: LogData) {
        let manager = self.manager.clone();
        let app_name = self.app_name.clone();
        let notify = self.notify.clone();
        tokio::spawn(async move {
            let mut con = manager
                .get_async_connection()
                .await
                .expect("Failed to get Redis connection");

            let key = format!("traces:{}", app_name);
            let timestamp = DateTime::parse_from_rfc3339(&log_data.timestamp)
                .unwrap()
                .timestamp_millis();

            let _: () = con
                .zadd(key, serde_json::to_string(&log_data).unwrap(), timestamp)
                .await
                .unwrap();

            manager.return_async_connection(con).await;
            notify.notify_one();
        });
    }

    async fn flush(&self) {
        self.notify.notified().await;
    }
}

struct RedisLogLayer {
    logger: Arc<RedisLogger>,
}

impl<S> Layer<S> for RedisLogLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event, ctx: Context<S>) {
        let mut field_visitor = FieldVisitor::new();
        event.record(&mut field_visitor);

        let mut log_data = LogData {
            timestamp: Utc::now().to_rfc3339(),
            level: event.metadata().level().to_string(),
            message: field_visitor.message,
            span_id: None,
            trace_id: "default_trace_id".to_string(),
            span_name: None,
        };

        // if let Some(scope) = ctx.event_scope(event) {
        //     if let Some(span) = scope.from_root().last() {
        //         if let Some(span_builder) =
        //             span.extensions().get::<opentelemetry::trace::SpanBuilder>()
        //         {
        //             log_data.span_id = Some(span_builder.span_id.unwrap().to_string());
        //             log_data.trace_id = span_builder.trace_id.unwrap().to_string();
        //             log_data.span_name = Some(span.name().to_string());
        //         }
        //     }
        // }
        if let Some(scope) = ctx.event_scope(event) {
            if let Some(span) = scope.from_root().last() {
                let span_ref: SpanRef<S> = span;
                log_data.span_id = Some(span_ref.id().into_u64().to_string());
                log_data.trace_id = span_ref.parent().map_or_else(
                    || span_ref.id().into_u64().to_string(),
                    |parent| parent.id().into_u64().to_string(),
                );
                log_data.span_name = Some(span_ref.name().to_string());
            }
        }

        self.logger.log(log_data);
    }
}

struct FieldVisitor {
    message: String,
}

impl FieldVisitor {
    fn new() -> Self {
        FieldVisitor {
            message: String::new(),
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            write!(&mut self.message, "{:?}", value).unwrap();
        }
    }
}

fn prepare_global_logging(
    app_name: String,
    manager: Arc<RedisManager>,
) -> RResult<Arc<RedisLogger>, AnyErr> {
    // let exporter = RedisExporter::new(app_name.clone(), manager.clone());
    // let logger = RedisLogger::new(app_name.clone(), manager.clone());
    let logger = Arc::new(RedisLogger::new(app_name.clone(), manager.clone()));

    // let tracer = init_tracer(exporter);
    // let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = Registry::default()
        // .with(telemetry)
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(EnvFilter::new(
            std::env::var("LOG_LEVEL").unwrap_or_else(|_| "debug".into()),
        ))
        .with(RedisLogLayer {
            logger: logger.clone(),
        });

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");

    Ok(logger)
}

pub struct LogViewer {
    manager: Arc<RedisManager>,
}

impl LogViewer {
    pub fn new(manager: Arc<RedisManager>) -> Self {
        LogViewer { manager }
    }

    pub async fn view_logs_by_app_name(&self, app_name: &str) -> RResult<(), AnyErr> {
        let mut con = self
            .manager
            .get_async_connection()
            .await
            .change_context(AnyErr)?;

        let key = format!("traces:{}", app_name);

        let logs: Vec<String> = con
            .zrangebyscore(key, "-inf", "+inf")
            .await
            .change_context(AnyErr)?;

        for log in logs {
            let log_data: LogData = serde_json::from_str(&log).change_context(AnyErr)?;
            println!(
                "{} - [{}] - {} - {}: {}",
                log_data.timestamp,
                log_data.level,
                log_data.trace_id,
                log_data.span_name.clone().unwrap_or("".to_string()),
                log_data.message
            );
        }

        self.manager.return_async_connection(con).await;

        Ok(())
    }

    pub async fn view_logs_by_span_name(
        &self,
        app_name: &str,
        span_name: &str,
    ) -> RResult<(), AnyErr> {
        let mut con = self
            .manager
            .get_async_connection()
            .await
            .change_context(AnyErr)?;

        let key = format!("traces:{}", app_name);

        let logs: Vec<String> = con
            .zrangebyscore(key, "-inf", "+inf")
            .await
            .change_context(AnyErr)?;

        for log in logs {
            let log_data: LogData = serde_json::from_str(&log).change_context(AnyErr)?;
            if log_data.span_name.as_deref() == Some(span_name) {
                println!(
                    "{} - [{}] - {} - {}: {}",
                    log_data.timestamp,
                    log_data.level,
                    log_data.trace_id,
                    log_data.span_name.clone().unwrap_or("".to_string()),
                    log_data.message
                );
            }
        }

        self.manager.return_async_connection(con).await;

        Ok(())
    }
}

// use tokio::time::{sleep, Duration};
#[tokio::test]
async fn test_tracing_and_logging() -> RResult<(), AnyErr> {
    let manager = Arc::new(RedisManager::new("redis://127.0.0.1/").change_context(AnyErr)?);

    manager.flushdb().await.change_context(AnyErr)?;

    let logger = prepare_global_logging("test".to_string(), manager.clone())?;

    #[instrument]
    async fn handle_request() {
        info!("Handling request");
        // Simulate work or calling other services
        apalis_job().await;
    }

    #[instrument]
    async fn apalis_job() {
        info!("Processing job");
        // Simulate work
    }

    info!("Starting test");
    handle_request().await;

    logger.flush().await;

    let viewer = LogViewer::new(manager.clone());

    viewer.view_logs_by_app_name("test").await?;

    println!("---");

    viewer.view_logs_by_span_name("test", "apalis_job").await?;

    // viewer.view_logs_by_span_name("test", "apalis_job").await?;

    // viewer
    //     .view_logs_by_trace_id("test", "default_trace_id")
    //     .await?;

    // // Query logs for the default trace
    // let mut con = manager
    //     .get_async_connection()
    //     .await
    //     .change_context(AnyErr)?;
    // let default_logs: Vec<String> = con
    //     .lrange("test:log:default_trace_id", 0, -1)
    //     .await
    //     .change_context(AnyErr)?;
    // dbg!(&default_logs);
    // assert!(!default_logs.is_empty(), "Default logs should not be empty");

    // // Query logs for a specific span
    // if let Some(log) = default_logs.get(0) {
    //     let log_data: LogData = serde_json::from_str(log).unwrap();
    //     if let Some(span_id) = &log_data.span_id {
    //         let span_logs: Vec<String> = con
    //             .lrange(format!("test:log:default_trace_id:{}", span_id), 0, -1)
    //             .await
    //             .change_context(AnyErr)?;

    //         dbg!(&span_logs);
    //         assert!(!span_logs.is_empty(), "Span logs should not be empty");
    //     }
    // }

    // // // Query spans
    // let spans: Vec<String> = con.keys("test:span:*").await.change_context(AnyErr)?;
    // dbg!(&spans);
    // assert!(!spans.is_empty(), "Spans should not be empty");

    Ok(())
}

// use futures::future::BoxFuture;
// use futures::stream::Any;
// use futures::FutureExt;
// use opentelemetry::trace::TraceError;
// use opentelemetry::trace::TracerProvider as TP;
// use opentelemetry_sdk::export::trace::ExportResult;
// use opentelemetry_sdk::export::trace::{SpanData, SpanExporter};
// use opentelemetry_sdk::metrics::exporter;
// use opentelemetry_sdk::trace::{
//     BatchConfig, BatchConfigBuilder, BatchSpanProcessor, Config, SimpleSpanProcessor, Tracer,
//     TracerProvider,
// };
// use opentelemetry_sdk::{runtime, trace};

// fn init_tracer(exporter: RedisExporter) -> Tracer {
//     // let exporter = RedisExporter::new(manager.clone());

//     let conf = BatchConfigBuilder::default()
//         .with_max_queue_size(4096)
//         .build();

//     let processor = BatchSpanProcessor::builder(exporter, runtime::Tokio)
//         .with_batch_config(conf)
//         .build();

//     let provider = TracerProvider::builder()
//         .with_config(Config::default())
//         // .with_subscriber()
//         .with_span_processor(processor)
//         .build();

//     let tracer = provider.tracer("example-tracer");
//     let _ = opentelemetry::global::set_tracer_provider(provider);

//     tracer
// }

// struct RedisExporter {
//     app_name: String,
//     manager: Arc<RedisManager>,
// }

// impl RedisExporter {
//     fn new(app_name: String, manager: Arc<RedisManager>) -> Self {
//         RedisExporter { app_name, manager }
//     }
// }

// impl Debug for RedisExporter {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("RedisExporter").finish()
//     }
// }

// impl SpanExporter for RedisExporter {
//     fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
//         let manager = self.manager.clone();
//         let app_name = self.app_name.clone();
//         async move {
//             let mut con = manager
//                 .get_async_connection()
//                 .await
//                 .map_err(|e| TraceError::from(e.to_string()))?;

//             // eprint!("Exporting spans: {:?}", batch);

//             for span in batch {
//                 let start_time: DateTime<Utc> = span.start_time.into();
//                 let end_time: DateTime<Utc> = span.end_time.into();

//                 let span_json = json!({
//                     "trace_id": span.span_context.trace_id().to_string(),
//                     "span_id": span.span_context.span_id().to_string(),
//                     "parent_span_id": span.parent_span_id.to_string(),
//                     "name": span.name,
//                     "start_time": start_time.to_rfc3339(),
//                     "end_time": end_time.to_rfc3339(),
//                     "attributes": span.attributes.iter().map(|kv| {
//                         (kv.key.as_str().to_string(), kv.value.to_string())
//                     }).collect::<Vec<(String, String)>>(),
//                 });

//                 // let key = format!("span:{}", span.span_context.span_id().to_string());
//                 let key = format!(
//                     "traces:{}:{}:{}:{}",
//                     app_name,
//                     span.span_context.trace_id(),
//                     span.name,
//                     span.span_context.span_id()
//                 );
//                 // format!(
//                 //     "{}:span:{}",
//                 //     app_name,
//                 //     span.span_context.span_id().to_string()
//                 // );

//                 if let Err(err) = con
//                     .set::<String, String, ()>(key, span_json.to_string())
//                     .await
//                 {
//                     eprintln!("Failed to set span data in Redis: {:?}", err);
//                     return Err(TraceError::from("Failed to set span data in Redis"));
//                 }
//             }

//             manager.return_async_connection(con).await;
//             Ok(())
//         }
//         .boxed()
//     }
// }
