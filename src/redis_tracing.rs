use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fmt::Write;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::field::Visit;
use tracing::{span, Level, Span};
use tracing_core::Field;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::registry::SpanRef;
use tracing_subscriber::registry::{LookupSpan, Registry};
use tracing_subscriber::{filter::EnvFilter, fmt};

use crate::errors::RResult;
use crate::prelude::*;
use crate::redis_manager::RedisManager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogData {
    timestamp: String,
    level: String,
    message: String,
    span_id: Option<String>,
    trace_id: String,
    span_name: Option<String>,
    job_id: Option<String>,
    service_name: Option<String>,
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
            job_id: field_visitor.job_id.clone(),
            service_name: field_visitor.service_name.clone(),
        };

        if let Some(scope) = ctx.event_scope(event) {
            if let Some(span) = scope.from_root().last() {
                let span_ref: SpanRef<S> = span;
                log_data.span_id = Some(span_ref.id().into_u64().to_string());
                log_data.trace_id = span_ref.parent().map_or_else(
                    || span_ref.id().into_u64().to_string(),
                    |parent| parent.id().into_u64().to_string(),
                );
                log_data.span_name = Some(span_ref.name().to_string());

                // let extensions = span_ref.extensions();
                // let job_id = extensions.get::<JobId>().cloned().unwrap();
                // let service_name = extensions.get::<ServiceName>().cloned().unwrap();
                // log_data.job_id = job_id;
                // log_data.service_name = service_name;
                // Check for conditional logging flag
                // println!("Logging: {:?}", field_visitor.redis_logging);
                // println!("Service name: {:?}", log_data.service_name);
                // println!("Message: {:?}", log_data.message);
                // if let Some(redis_logging) = field_visitor.redis_logging {
                //     println!("Redis logging: {}", redis_logging);
                //     if redis_logging {
                //         self.logger.log(log_data);
                //     }
                // }
                let extensions = span_ref.extensions();
                if let Some(job_id) = extensions.get::<String>() {
                    log_data.job_id = Some(job_id.clone());
                }
                if let Some(service_name) = extensions.get::<String>() {
                    log_data.service_name = Some(service_name.clone());
                }
                if let Some(redis_logging) = extensions.get::<bool>() {
                    if *redis_logging {
                        self.logger.log(log_data.clone());
                    }
                }
                println!("Logging: {:?}", field_visitor.redis_logging);
                println!("Service name: {:?}", log_data.service_name);
                println!("Message: {:?}", log_data.message);
            }
        }

        // self.logger.log(log_data);
    }
}

#[derive(Debug)]
struct FieldVisitor {
    message: String,
    job_id: Option<String>,
    service_name: Option<String>,
    redis_logging: Option<bool>,
}

impl FieldVisitor {
    fn new() -> Self {
        FieldVisitor {
            message: String::new(),
            job_id: None,
            service_name: None,
            redis_logging: None,
        }
    }
}

impl Visit for FieldVisitor {
    // fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
    //     if field.name() == "message" {
    //         self.message = format!("{:?}", value);
    //     } else if field.name() == "job_id" {
    //         self.job_id = Some(format!("{:?}", value).trim_matches('"').to_string());
    //     } else if field.name() == "service_name" {
    //         self.service_name = Some(format!("{:?}", value).trim_matches('"').to_string());
    //     } else if field.name() == "redis_logging" {
    //         if let Ok(logging) = format!("{:?}", value).parse::<bool>() {
    //             self.redis_logging = Some(logging);
    //         }
    //     }
    // }
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "job_id" {
            println!("FOOOOOO Job ID: {:?}", value);
        }
        match field.name() {
            "message" => self.message = format!("{:?}", value),
            "job_id" => self.job_id = Some(format!("{:?}", value)),
            "service_name" => self.service_name = Some(format!("{:?}", value)),
            "redis_logging" => {
                // Assuming value is printed as a boolean when using Debug
                self.redis_logging = format!("{:?}", value).parse::<bool>().ok();
            }
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "job_id" => self.job_id = Some(value.to_string()),
            "service_name" => self.service_name = Some(value.to_string()),
            "redis_logging" => {
                self.redis_logging = value.parse::<bool>().ok();
            }
            _ => {}
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "redis_logging" {
            self.redis_logging = Some(value);
        }
    }
}

fn prepare_global_logging(
    app_name: String,
    manager: Arc<RedisManager>,
) -> RResult<Arc<RedisLogger>, AnyErr> {
    let logger = Arc::new(RedisLogger::new(app_name.clone(), manager.clone()));
    let subscriber = Registry::default()
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

#[derive(Debug, Clone)]
struct JobId(String);

#[derive(Debug, Clone)]
struct ServiceName(String);

pub struct LogViewer {
    manager: Arc<RedisManager>,
}

impl LogViewer {
    pub fn new(manager: Arc<RedisManager>) -> Self {
        LogViewer { manager }
    }

    async fn fetch_logs(&self, app_name: &str) -> RResult<Vec<LogData>, AnyErr> {
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

        let log_data_list: Vec<LogData> = logs
            .iter()
            .map(|log| {
                serde_json::from_str::<LogData>(log)
                    .change_context(AnyErr)
                    .unwrap()
            })
            .collect();

        self.manager.return_async_connection(con).await;

        Ok(log_data_list)
    }

    fn print_logs(log_data_list: &[LogData]) {
        for log_data in log_data_list {
            println!(
                "{} - [{}] - {} - {}: {}",
                log_data.timestamp,
                log_data.level,
                log_data.trace_id,
                log_data.span_name.clone().unwrap_or_else(|| "".to_string()),
                log_data.message
            );
        }
    }

    pub async fn view_logs_by_app_name(&self, app_name: &str) -> RResult<Vec<LogData>, AnyErr> {
        let log_data_list = self.fetch_logs(app_name).await?;
        Self::print_logs(&log_data_list);
        Ok(log_data_list)
    }

    pub async fn view_logs_by_span_name(
        &self,
        app_name: &str,
        span_name: &str,
    ) -> RResult<Vec<LogData>, AnyErr> {
        let log_data_list = self.fetch_logs(app_name).await?;

        let span_logs: Vec<LogData> = log_data_list
            .into_iter()
            .filter(|log_data| log_data.span_name.as_deref() == Some(span_name))
            .collect();

        Self::print_logs(&span_logs);
        Ok(span_logs)
    }

    pub async fn view_logs_by_service_name(
        &self,
        app_name: &str,
        service_name: &str,
    ) -> RResult<Vec<LogData>, AnyErr> {
        let log_data_list = self.fetch_logs(app_name).await?;

        let service_logs: Vec<LogData> = log_data_list
            .into_iter()
            .filter(|log_data| log_data.service_name.as_deref() == Some(service_name))
            .collect();

        Self::print_logs(&service_logs);
        Ok(service_logs)
    }

    pub async fn view_logs_by_job_id(
        &self,
        app_name: &str,
        job_id: &str,
    ) -> RResult<Vec<LogData>, AnyErr> {
        let log_data_list = self.fetch_logs(app_name).await?;

        let job_logs: Vec<LogData> = log_data_list
            .into_iter()
            .filter(|log_data| log_data.job_id.as_deref() == Some(job_id))
            .collect();

        Self::print_logs(&job_logs);
        Ok(job_logs)
    }
}

// fn create_service_span(span_name: &str, job_id: &str, service_name: &str) -> Span {
//     span!(Level::INFO, span_name, job_id = %JobId(job_id.to_string()), service_name = %ServiceName(service_name.to_string()))
// }

// use tokio::time::{sleep, Duration};
#[tokio::test]
async fn test_tracing_and_logging() -> RResult<(), AnyErr> {
    // use tracing::instrument;
    use tracing::{info, span, Level};

    let manager = Arc::new(RedisManager::new("redis://127.0.0.1/").change_context(AnyErr)?);

    manager.flushdb().await.change_context(AnyErr)?;

    let logger = prepare_global_logging("test".to_string(), manager.clone())?;

    // #[instrument]
    async fn handle_request(job_id: &str, service_name: &str) {
        let span = span!(
            Level::INFO,
            "handle_request",
            job_id = job_id,
            service_name = service_name,
            redis_logging = true
        );
        let _enter = span.enter();

        // Span::current().with_subscriber(|(current_span, _)| {
        //     let mut extensions = current_span.extensions_mut();
        //     extensions.insert(JobId(job_id.to_string()));
        //     extensions.insert(ServiceName(service_name.to_string()));
        //     extensions.insert(true);
        // });

        info!("Handling request");
        apalis_job(job_id, service_name).await;
    }

    async fn apalis_job(job_id: &str, service_name: &str) {
        let span = span!(
            Level::INFO,
            "apalis_job",
            job_id = job_id,
            service_name = service_name,
            redis_logging = true
        );
        let _enter = span.enter();
        info!("Processing job");
    }

    info!("Starting test");
    handle_request("job123", "serviceA").await;

    // logger.flush().await;

    let viewer = LogViewer::new(manager.clone());

    let logs = viewer.view_logs_by_app_name("test").await?;
    dbg!(&logs);
    println!("Log length: {}", logs.len());
    assert_eq!(logs.len(), 2, "There should be exactly 2 logs");

    println!("---");

    let span_logs = viewer.view_logs_by_span_name("test", "apalis_job").await?;
    assert_eq!(
        span_logs.len(),
        1,
        "There should be exactly 1 log for the span 'apalis_job'"
    );

    println!("---");

    let service_logs = viewer.view_logs_by_service_name("test", "serviceA").await?;
    // assert_eq!(
    //     service_logs.len(),
    //     3,
    //     "There should be exactly 3 logs for the service 'serviceA'"
    // );

    // let job_logs = viewer.view_logs_by_job_id("test", "job123").await?;
    // assert_eq!(
    //     job_logs.len(),
    //     3,
    //     "There should be exactly 3 logs for the job 'job123'"
    // );

    Ok(())
}
