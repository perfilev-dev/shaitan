mod engine;
mod pipelines;

use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;
use std::sync::atomic::{AtomicU32, Ordering};

use async_channel::{Sender, Receiver};
use futures::{Stream, SinkExt};
use futures::future::select_all;
use tokio::time;
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tonic::metadata::MetadataValue;
use crate::engine::Engine;
use std::fmt::Formatter;
use crate::pipelines::InnerJob::Pipeline;


pub mod proto {
    tonic::include_proto!("queue");

    pub(crate) const FILE_DESCRIPTOR_SET: &'static [u8] =
        tonic::include_file_descriptor_set!("queue_descriptor");
}

type QueueType = Mutex<HashMap<String, (Sender<proto::TaskJson>, Receiver<proto::TaskJson>)>>;
type MethodsType = Mutex<HashMap<String, Vec<String>>>;
type PipelineQueue = Mutex<HashMap<u32, Sender<proto::ResultJson>>>;

lazy_static::lazy_static! {

    // tasks
    static ref JSON_TASKS: QueueType = QueueType::default();
    static ref JSON_RESULTS: QueueType = QueueType::default();
    static ref TASKS_COUNTER: AtomicU32 = AtomicU32::new(0);

    // workers + methods
    static ref WORKERS_METHODS: MethodsType = MethodsType::default();
    static ref WORKERS_COUNTER: AtomicU32 = AtomicU32::new(0);

    // pipelines
    static ref TASK_PIPELINE_MAP: Mutex<HashMap<u32, u32>> = Mutex::<HashMap::<u32, u32>>::default();
    static ref PIPELINES_COUNTER: AtomicU32 = AtomicU32::new(0);
    static ref PIPELINES_QUEUE: PipelineQueue = PipelineQueue::default();

}

pub async fn schedule_task(method: &str, input: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let sender = JSON_TASKS.lock().await
        .get(method)
        .map(|(sender, _)| sender.clone())
        .ok_or(format!("method {} not found", method))?;

    let task_id = TASKS_COUNTER.fetch_add(1, Ordering::AcqRel);

    sender.send(proto::TaskJson {
        method: method.to_string(),
        input: input.to_string(),
        task_id
    }).await?;

    Ok(task_id)
}

pub async fn get_pipeline_queue() -> (u32, Receiver<proto::ResultJson>) {
    let pipeline_id = PIPELINES_COUNTER.fetch_add(1, Ordering::AcqRel);

    let (tx, rx) = async_channel::bounded(10);

    PIPELINES_QUEUE.lock().await.insert(pipeline_id, tx);

    (pipeline_id, rx)
}

pub async fn schedule_pipeline_task(pipeline_id: u32, method: &str, input: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let task_id = schedule_task(method, input).await?;

    // remember pipeline to send result back!
    TASK_PIPELINE_MAP.lock().await.insert(task_id, pipeline_id);

    Ok(task_id)
}

#[derive(Default)]
pub struct MyQueue {}

#[tonic::async_trait]
impl proto::queue_server::Queue for MyQueue {
    async fn register(
        &self,
        request: Request<proto::ExecutorInfo>,
    ) -> Result<Response<proto::Token>, Status> {
        println!("Registering new worker...");

        // ...
        let mut worker_methods = vec![];
        for service in &request.get_ref().services {
            for method in &service.methods {
                let full_method_name = format!("{}/{}", service.name, method.name);

                let mut tasks = JSON_TASKS.lock().await;
                if !tasks.contains_key(&full_method_name) {
                    tasks.insert(full_method_name.clone(), async_channel::bounded(10));
                }

                let mut results = JSON_RESULTS.lock().await;
                if !results.contains_key(&full_method_name) {
                    results.insert(full_method_name.clone(), async_channel::bounded(10));
                }

                worker_methods.push(full_method_name);
            }
        }   

        let reply = proto::Token {
            access_token: format!("worker_{}", WORKERS_COUNTER.fetch_add(1, Ordering::AcqRel)),
        };

        // remember list of methods for this worker!
        WORKERS_METHODS.lock().await.insert(reply.access_token.clone(), worker_methods);

        Ok(Response::new(reply))
    }

    type ProcessJsonStream =
        Pin<Box<dyn Stream<Item = Result<proto::TaskJson, Status>> + Send + Sync + 'static>>;

    async fn process_json(
        &self,
        request: Request<tonic::Streaming<proto::ResultJson>>,
    ) -> Result<Response<Self::ProcessJsonStream>, Status> {
        let token = request.metadata().get("access_token");
        if token.is_none() {
            return Err(Status::unauthenticated("no access_token in meta".to_string()));
        }

        let access_token = token.unwrap().to_str().unwrap().to_string();
        println!("Processing [JSON] from {}...", access_token);

        // stream contains results!
        let mut stream = request.into_inner();

        // build list of receivers for the worker! access_token in meta!
        let methods = WORKERS_METHODS.lock().await.get(&access_token).unwrap().clone();
        let mut task_receivers = JSON_TASKS.lock().await
            .iter()
            .filter(|(x, _)| methods.contains(x))
            .map(|(_, (_, receiver))| receiver.clone())
            .collect::<Vec<Receiver<proto::TaskJson>>>();

        println!("receivers: {}", task_receivers.len());

        // main loop, that yields tasks...
        let outbound = async_stream::stream! {
            loop {
                let mut tasks_fut = select_all(task_receivers.iter().map(|x| x.recv()));
                tokio::select! {
                    result = stream.next() => {
                        if let Some(ref res) = result {
                            match res {
                                Ok(r) => {
                                    println!("got result! {:?}", result);

                                    // is it pipeline task? is so, push it to right queue!
                                    if let Some(id) = TASK_PIPELINE_MAP.lock().await.get(&r.task_id) {
                                        let tx = PIPELINES_QUEUE.lock().await
                                            .get(id)
                                            .map(|x| x.clone())
                                            .unwrap();

                                        tx.send(r.clone()).await.unwrap();
                                    }

                                },
                                Err(err) => {
                                    println!("error: {:?}", err);
                                    break;
                                }
                            }

                        }
                    },
                    (task_result, _, _) = tasks_fut => {
                        match task_result {
                            Ok(task) => {
                                println!("new task in queue: {:?}", task);
                                yield Ok(task);
                            },
                            Err(err) => {
                                println!("error: {:?}", err);
                                continue;
                            }
                        }
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(outbound) as Self::ProcessJsonStream))
    }

    type ProcessRawStream = ReceiverStream<Result<proto::TaskRaw, Status>>;

    async fn process_raw(
        &self,
        request: Request<tonic::Streaming<proto::ResultRaw>>,
    ) -> Result<Response<Self::ProcessRawStream>, Status> {
        unimplemented!("need to be implemented")
    }

    async fn schedule_task(
        &self,
        request: Request<proto::NewTaskJson>,
    ) -> Result<Response<proto::ScheduledTask>, Status> {
        Ok(Response::new(proto::ScheduledTask {
            task_id: schedule_task(&request.get_ref().method, &request.get_ref().input).await
                .map_err(|err| Status::internal(err.to_string()))?
        }))
    }

    async fn send_input_to_pipeline(
        &self,
        request: Request<proto::PipelineInput>,
    ) -> Result<Response<proto::SendResult>, Status> {

        // just to test =)
        pipelines::test().await;

        Ok(Response::new(proto::SendResult {
            status: "ok".to_string()
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let addr = "[::1]:50052".parse().unwrap();
    let queue = MyQueue::default();

    // pipelines::test().await;

    Server::builder()
        .add_service(service)
        .add_service(proto::queue_server::QueueServer::new(queue))
        .serve(addr)
        .await?;

    Ok(())
}