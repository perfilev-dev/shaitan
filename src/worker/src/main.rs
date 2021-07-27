use reflection::server_reflection_client::ServerReflectionClient;
use reflection::server_reflection_request::MessageRequest;
use reflection::ServerReflectionRequest;
use reflection::server_reflection_response::MessageResponse;
use protobuf::descriptor::{FileDescriptorProto, MethodDescriptorProto};
use protobuf::Message;
use protobuf::reflect::{FileDescriptor, MessageDescriptor};
use protobuf::json::{parse_dynamic_from_str, print_to_string};
use tonic::codegen::http;
use tonic::{IntoRequest, Request, Streaming};
use prost::bytes::{Buf, BufMut};
use prost::DecodeError;
use prost::encoding::{WireType, DecodeContext};
use tonic::codec::ProstCodec;
use futures::channel::mpsc;
use futures::{SinkExt, Stream};
use std::error::Error;
use std::fmt::Formatter;
use std::collections::HashMap;
use protobuf::descriptor::field_descriptor_proto::Type;
use serde_json::Value;
use regex::Regex;
use std::str::FromStr;
use tonic::transport::Channel;
use tonic::metadata::MetadataValue;
use futures::FutureExt;
use futures::stream::FuturesUnordered;
use futures::StreamExt;

lazy_static::lazy_static! {
    // captures this: /helloworld.Greeter/SayHello
    static ref PATH_RE: Regex = Regex::new(r"^/(?P<service>.+?)/(?P<method>.+?)$").unwrap();
}


pub mod reflection {
    tonic::include_proto!("grpc.reflection.v1alpha");
}

pub mod queue {
    tonic::include_proto!("queue");
}

#[derive(Debug, Default)]
pub struct RawBytes {
    pub bytes: Vec<u8>
}

impl prost::Message for RawBytes {
    fn encode_raw<B>(&self, buf: &mut B) where
        B: BufMut,
        Self: Sized {
        buf.put(&*self.bytes);
    }

    fn merge_field<B>(&mut self, _: u32, _: WireType, buf: &mut B, _: DecodeContext) -> Result<(), DecodeError> where
        B: Buf,
        Self: Sized {
        while buf.has_remaining() {
            self.bytes.push(buf.get_u8());
        }
        Ok(())
    }

    fn encoded_len(&self) -> usize {
        self.bytes.len()
    }

    fn clear(&mut self) {
        self.bytes.clear();
    }
}

#[derive(Debug)]
enum MyError {
    Other (String)
}

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self))
    }
}

impl std::error::Error for MyError {

}


/// Below code for GRPC

#[derive(Debug)]
struct Executor {
    address: String,
    protos: HashMap<String, FileDescriptorProto>
}

impl Executor {

    async fn connect(address: String) -> Result<Self, Box<dyn Error>> {
        let mut executor = Self {
            address: address.to_string(),
            protos: Default::default()
        };

        // load protos!
        let mut client = ServerReflectionClient::connect(address).await?;
        let (mut tx, rx) = mpsc::unbounded::<ServerReflectionRequest>();

        // send list services
        tx.send(ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::ListServices(String::new()))
        }).await?;

        let response = client.server_reflection_info(tonic::Request::new(rx)).await?;
        let mut inbound = response.into_inner();

        let mut services: Vec<String> = vec![];
        loop {
            match inbound.message().await? {
                Some(response) => {
                    if let Some(message_response) = &response.message_response {
                        match message_response {
                            MessageResponse::ListServicesResponse(service_response) => {
                                for service in &service_response.service {
                                    if !service.name.ends_with("ServerReflection") {
                                        services.push(service.name.to_string());
                                    }
                                }
                            },
                            MessageResponse::FileDescriptorResponse(file_descriptor_response) => {
                                let bytes = &file_descriptor_response.file_descriptor_proto.get(0).unwrap();
                                let proto = FileDescriptorProto::parse_from_bytes(bytes).unwrap();

                                // ...
                                executor.protos.insert(services.pop().unwrap(), proto);
                            },
                            _ => {}
                        }

                        if services.is_empty() {
                            break;
                        } else {
                            tx.send(ServerReflectionRequest {
                                host: String::new(),
                                message_request: Some(MessageRequest::FileContainingSymbol(services.first().unwrap().to_string()))
                            }).await?;
                            continue;
                        }
                    }
                    return Err(MyError::Other(format!("wrong response: {:?}", response)).into());
                },
                None => {
                    return Err(MyError::Other("no response".to_string()).into());
                }
            }
        }

        Ok(executor)
    }

    fn services(&self) -> Vec<String> {
        self.protos.keys().map(|x| x.to_string()).collect()
    }

    fn service_methods(&self, service_name: &str) -> Result<Vec<String>, MyError> {
        let proto = self.protos
            .get(service_name)
            .ok_or(MyError::Other("proto not found".into()))?;

        let mut methods = vec![];
        for srv in &proto.service {
            if service_name.ends_with(srv.get_name()) {
                for method in &srv.method {
                    methods.push(method.get_name().to_string());
                }
                return Ok(methods);
            }
        }

        Err(MyError::Other("service not found".into()))
    }

    fn method_info(&self, service_name: &str, method_name: &str) -> Result<queue::MethodInfo, MyError> {
        let proto = self.protos
            .get(service_name)
            .ok_or(MyError::Other("proto not found".into()))?;

        for srv in &proto.service {
            if service_name.ends_with(srv.get_name()) {
                for method in &srv.method {
                    if method.get_name() == method_name {
                        return Ok(queue::MethodInfo {
                            name: method.get_name().to_string(),
                            input_type_name: method.get_input_type().to_string(),
                            output_type_name: method.get_output_type().to_string(),
                            server_streaming: method.has_server_streaming()
                        });
                    }
                }
            }
        }

        Err(MyError::Other("service not found".into()))
    }

    fn enum_types(&self) -> Vec<queue::EnumTypeInfo> {
        let mut enum_types = vec![];

        for proto in self.protos.values() {
            for enum_type in &proto.enum_type {
                let mut info = queue::EnumTypeInfo {
                    name: enum_type.get_name().to_string(),
                    values: vec![]
                };

                for value in &enum_type.value {
                    info.values.push(queue::EnumValueInfo {
                        name: value.get_name().to_string(),
                        number: value.get_number()
                    })
                }

                enum_types.push(info);
            }
        }

        enum_types
    }

    fn message_types(&self) -> Vec<queue::MessageTypeInfo> {
        let mut message_types = vec![];

        for proto in self.protos.values() {
            for message_type in &proto.message_type {
                let mut info = queue::MessageTypeInfo {
                    name: message_type.get_name().to_string(),
                    fields: vec![]
                };

                for field in &message_type.field {
                    info.fields.push(queue::FieldTypeInfo {
                        name: field.get_json_name().to_string(),
                        optional: field.get_proto3_optional(),
                        r#type: (match field.get_field_type() {
                            Type::TYPE_DOUBLE => "double",
                            Type::TYPE_FLOAT => "float",
                            Type::TYPE_INT64 => "int64",
                            Type::TYPE_UINT64 => "uint64",
                            Type::TYPE_INT32 => "int32",
                            Type::TYPE_FIXED64 => "fixed64",
                            Type::TYPE_FIXED32 => "fixed32",
                            Type::TYPE_BOOL => "bool",
                            Type::TYPE_STRING => "string",
                            Type::TYPE_GROUP => "group",
                            Type::TYPE_MESSAGE => "message",
                            Type::TYPE_BYTES => "bytes",
                            Type::TYPE_UINT32 => "uint32",
                            Type::TYPE_ENUM => "enum",
                            Type::TYPE_SFIXED32 => "sfixed32",
                            Type::TYPE_SFIXED64 => "sfixed64",
                            Type::TYPE_SINT32 => "sint32",
                            Type::TYPE_SINT64 => "sint64"
                        }).to_string()
                    });
                }

                message_types.push(info);
            }
        }

        message_types
    }

    fn info(&self) -> Result<queue::ExecutorInfo, MyError> {
        let mut info = queue::ExecutorInfo {
            services: vec![],
            enum_types: self.enum_types(),
            message_types: self.message_types()
        };

        for service_name in &self.services() {
            let mut service_info = queue::ServiceInfo {
                name: service_name.to_string(),
                methods: vec![]
            };
            for method_name in &self.service_methods(service_name)? {
                service_info.methods.push(self.method_info(service_name, method_name)?);
            }
            info.services.push(service_info);
        };

        Ok(info)
    }

    fn get_method(&self, service_name: &str, method_name: &str) -> Result<&MethodDescriptorProto, MyError> {
        let proto = self.protos.get(service_name)
            .ok_or(MyError::Other("service not found".to_string()))?;

        for srv in &proto.service {
            if service_name.ends_with(srv.get_name()) {
                for method in &srv.method {
                    if method.get_name() == method_name {
                        return Ok(method)
                    }
                }
            }
        }

        Err(MyError::Other("method not found".to_string()))
    }

    fn get_message_by_type_name(&self, name: &str) -> Option<MessageDescriptor> {
        for proto in self.protos.values() {
            let fd = FileDescriptor::new_dynamic(proto.clone(), vec![]);

            if let Some(message) = fd.message_by_full_name(name) {
                return Some(message);
            }
        }

        None
    }

    // async fn server_streaming(&self, path: String, json_str: String, task_id: u32) -> (Result<Streaming<RawBytes>, Box<dyn Error>>, u32) {
    //     (self._server_streaming(path, json_str).await, task_id)
    // }

    async fn unary_json(&self, path: String, json_str: String, task_id: u32) -> (Result<Value, Box<dyn Error>>, u32) {
        (self._unary_json(path, json_str).await, task_id)
    }

    async fn _server_streaming(&self, path: String, json_str: String) -> Result<impl Stream<Item = Result<Value, Box<dyn Error>>>, Box<dyn Error>> {
        let caps = PATH_RE.captures(&path)
            .ok_or(MyError::Other("wrong service path".to_string()))?;

        // find method and types (input, output)
        let method = self.get_method(&caps["service"], &caps["method"])?;
        let input_type = self.get_message_by_type_name(method.get_input_type())
            .ok_or(MyError::Other("input type not found!".to_string()))?;
        let output_type = self.get_message_by_type_name(method.get_output_type())
            .ok_or(MyError::Other("output type not found!".to_string()))?;

        // create input message
        let input = parse_dynamic_from_str(&input_type, &json_str)?;

        let request: Request<RawBytes> = tonic::Request::new(RawBytes {
            bytes: input.write_to_bytes_dyn()?
        });

        // connect!
        let conn = tonic::transport::Endpoint::new(self.address.to_string())?.connect().await?;
        let mut grpc = tonic::client::Grpc::new(conn);
        grpc.ready().await?;

        let codec: ProstCodec<RawBytes, RawBytes> = tonic::codec::ProstCodec::default();
        let path = http::uri::PathAndQuery::from_str(&path).unwrap();

        // make request and got response...
        let response: tonic::Response<Streaming<RawBytes>> = grpc.server_streaming(request.into_request(), path, codec).await?;
        let mut inbound = response.into_inner();

        Ok(async_stream::stream! {

            while let Some(raw) = inbound.message().await? {
                let mut output = raw.bytes.clone();

                // wtf is this? but it work!
                output.insert(0, 10);

                let mut out = output_type.new_instance();
                out.merge_from_bytes_dyn(&output)?;

                yield Ok(serde_json::from_str(&print_to_string(&*out).unwrap()).unwrap());
            }

        })
    }

    async fn _unary_json(&self, path: String, json_str: String) -> Result<Value, Box<dyn Error>> {
        let caps = PATH_RE.captures(&path)
            .ok_or(MyError::Other("wrong service path".to_string()))?;

        // find method and types (input, output)
        let method = self.get_method(&caps["service"], &caps["method"])?;
        let input_type = self.get_message_by_type_name(method.get_input_type())
            .ok_or(MyError::Other("input type not found!".to_string()))?;
        let output_type = self.get_message_by_type_name(method.get_output_type())
            .ok_or(MyError::Other("output type not found!".to_string()))?;

        // create input message
        let input = parse_dynamic_from_str(&input_type, &json_str)?;

        let request: Request<RawBytes> = tonic::Request::new(RawBytes {
            bytes: input.write_to_bytes_dyn()?
        });

        // connect!
        let conn = tonic::transport::Endpoint::new(self.address.to_string())?.connect().await?;
        let mut grpc = tonic::client::Grpc::new(conn);
        grpc.ready().await?;

        let codec: ProstCodec<RawBytes, RawBytes> = tonic::codec::ProstCodec::default();
        let path = http::uri::PathAndQuery::from_str(&path).unwrap();

        // make request and got response...
        let result: tonic::Response<RawBytes> = grpc.unary(request.into_request(), path, codec).await?;
        let mut output = result.into_inner().bytes.clone();

        // wtf is this? but it work!
        output.insert(0, 10);

        let mut out = output_type.new_instance();
        out.merge_from_bytes_dyn(&output)?;

        Ok(serde_json::from_str(&print_to_string(&*out).unwrap())?)
    }

}


/// ----

async fn queue_forever(client: &mut queue::queue_client::QueueClient<Channel>, access_token: &str, executor: &Executor) -> Result<(), Box<dyn Error>> {
    let (mut tx, rx) = mpsc::channel(10);

    let mut request = Request::new(rx);
    request.metadata_mut().insert("access_token", MetadataValue::from_str(access_token)?);

    let response = client.process_json(request).await?;
    let mut inbound = response.into_inner();

    let mut future_results = FuturesUnordered::new();
    loop {
        if future_results.is_empty() {
            tokio::select! {
                task_result = inbound.message() => {
                    if let Ok(task) = task_result {
                        match task {
                            Some(t) => {
                                println!("new task! {:?}", t);
                                let fut = executor.unary_json(format!("/{}", t.method), t.input, t.task_id);
                                future_results.push(fut.boxed());
                            },
                            None => {
                                println!("no task");
                            }
                        }
                    }
                }
            }
        }
        else {
            tokio::select! {
                task_result = inbound.message() => {
                    if let Ok(task) = task_result {
                        match task {
                            Some(t) => {
                                println!("new task! {:?}", t);
                                let fut = executor.unary_json(format!("/{}", t.method), t.input, t.task_id);
                                future_results.push(fut.boxed());
                            },
                            None => {
                                println!("no task");
                            }
                        }
                    }
                },
                result = future_results.next() => {
                    println!("result: {:?}", result);

                    match result {
                        Some((r, task_id)) => {
                            let mut result = queue::ResultJson::default();
                            result.task_id = task_id;

                            match r {
                                Ok(output) => {
                                    result.r = Some(queue::result_json::R::Output(serde_json::to_string(&output)?));
                                },
                                Err(err) => {
                                    result.r = Some(queue::result_json::R::Error(err.to_string()));
                                }
                            }

                            tx.send(result).await?;
                        }
                        None => {
                            println!("no result?");
                        }
                    }


                }
            }
        }
    }
}



#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let executor = Executor::connect("http://[::1]:50051".to_string()).await?;

    // get executor info in grpc format!
    let info = executor.info()?;

    // print summary info about rpc services, enums, types...
    println!("INFO:\n{:?}", &executor.info()?);
    //
    // let result = executor.unary_json("/helloworld.Greeter/SayHello", &json!({
    //     "name": "Sergey"
    // })).await?;
    //
    // println!("{:?}", result);

    // let mut result = executor._server_streaming("/math.SimpleMath/RandomEverySecond".to_string(), serde_json::json!({
    //
    // }).to_string()).await?;
    //
    // futures::pin_mut!(result);
    //
    // while let Some(value) = result.next().await {
    //     println!("got {:?}", value);
    // }

    // ------------ SERVER -------------

    let mut client = queue::queue_client::QueueClient::connect("http://[::1]:50052").await?;

    // register client and get token!
    let response = client.register(info).await?;
    let access_token = response.get_ref().access_token.to_string();

    // then fetch tasks
    queue_forever(&mut client, &access_token, &executor).await?;

    Ok(())
}
