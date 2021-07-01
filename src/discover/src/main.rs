use reflection::server_reflection_client::ServerReflectionClient;
use reflection::server_reflection_request::MessageRequest;
use reflection::{ServerReflectionRequest, ServerReflectionResponse};
use reflection::server_reflection_response::MessageResponse;
use tokio::time;
use std::time::Duration;
use protobuf::descriptor::{FileDescriptorProto, FileDescriptorSet};
use protobuf::{Message, MessageDyn};
use prost_types::{DescriptorProto, Method};
use serde_protobuf::{descriptor, de};
use protobuf::reflect::{FileDescriptor, MessageDescriptor};
use protobuf::json::{parse_dynamic_from_str, print_to_string};
use tonic::codegen::http;
use tonic::{IntoRequest, Request};
use prost::bytes::{Buf, BufMut};
use prost::DecodeError;
use prost::encoding::{WireType, DecodeContext};
use tonic::codec::ProstCodec;
use futures::channel::mpsc;
use futures::{StreamExt, SinkExt};
use std::error::Error;
use tonic::client::Grpc;
use tonic::transport::Channel;
use std::fmt::Formatter;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Keys;
use protobuf::descriptor::field_descriptor_proto::Type;
use serde::{Serialize, Deserialize};


pub mod reflection {
    tonic::include_proto!("grpc.reflection.v1alpha");
}

#[derive(Debug, Default)]
pub struct Message1 {
    pub bytes: Vec<u8>
}

impl prost::Message for Message1 {
    fn encode_raw<B>(&self, buf: &mut B) where
        B: BufMut,
        Self: Sized {
        buf.put(&*self.bytes);
    }

    fn merge_field<B>(&mut self, tag: u32, wire_type: WireType, buf: &mut B, ctx: DecodeContext) -> Result<(), DecodeError> where
        B: Buf,
        Self: Sized {
        println!("{}", buf.remaining());
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
    client: ServerReflectionClient<Channel>,
    protos: HashMap<String, FileDescriptorProto>
}

impl Executor {

    async fn connect(addr: String) -> Result<Self, Box<dyn Error>> {
        let mut executor = Self {
            client: ServerReflectionClient::connect(addr).await?,
            protos: Default::default()
        };

        // load protos!
        let (mut tx, rx) = mpsc::unbounded::<ServerReflectionRequest>();

        let response = executor.client.server_reflection_info(tonic::Request::new(rx)).await?;
        let mut inbound = response.into_inner();

        // send list services
        tx.send(ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::ListServices(String::new()))
        }).await?;

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

    fn method_info(&self, service_name: &str, method_name: &str) -> Result<MethodInfo, MyError> {
        let proto = self.protos
            .get(service_name)
            .ok_or(MyError::Other("proto not found".into()))?;

        for srv in &proto.service {
            if service_name.ends_with(srv.get_name()) {
                for method in &srv.method {
                    if method.get_name() == method_name {
                        return Ok(MethodInfo {
                            name: method.get_name().to_string(),
                            input_type_name: method.get_input_type().to_string(),
                            output_type_name: method.get_output_type().to_string()
                        });
                    }
                }
            }
        }

        Err(MyError::Other("service not found".into()))
    }

    fn enum_types(&self) -> Vec<EnumTypeInfo> {
        let mut enum_types = vec![];

        for proto in self.protos.values() {
            for enum_type in &proto.enum_type {
                let mut info = EnumTypeInfo {
                    name: enum_type.get_name().to_string(),
                    values: vec![]
                };

                for value in &enum_type.value {
                    info.values.push(EnumValueInfo {
                        name: value.get_name().to_string(),
                        number: value.get_number()
                    })
                }

                enum_types.push(info);
            }
        }

        enum_types
    }

    fn message_types(&self) -> Vec<MessageTypeInfo> {
        let mut message_types = vec![];

        for proto in self.protos.values() {
            for message_type in &proto.message_type {
                let mut info = MessageTypeInfo {
                    name: message_type.get_name().to_string(),
                    fields: vec![]
                };

                for field in &message_type.field {
                    info.fields.push(FieldTypeInfo {
                        name: field.get_json_name().to_string(),
                        optional: field.get_proto3_optional(),
                        type_: (match field.get_field_type() {
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

    fn info(&self) -> Result<ExecutorInfo, MyError> {
        let mut info = ExecutorInfo {
            services: vec![],
            enum_types: self.enum_types(),
            message_types: self.message_types()
        };

        for service_name in &self.services() {
            let mut service_info = ServiceInfo {
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

}

#[derive(Debug, Serialize)]
struct ExecutorInfo {
    services: Vec<ServiceInfo>,
    enum_types: Vec<EnumTypeInfo>,
    message_types: Vec<MessageTypeInfo>
}

#[derive(Debug, Serialize)]
struct ServiceInfo {
    name: String,
    methods: Vec<MethodInfo>
}

#[derive(Debug, Serialize)]
struct MethodInfo {
    name: String,
    input_type_name: String,
    output_type_name: String
}

#[derive(Debug, Serialize)]
struct EnumTypeInfo {
    name: String,
    values: Vec<EnumValueInfo>
}

#[derive(Debug, Serialize)]
struct EnumValueInfo {
    name: String,
    number: i32
}

#[derive(Debug, Serialize)]
struct MessageTypeInfo {
    name: String,
    fields: Vec<FieldTypeInfo>
}

#[derive(Debug, Serialize)]
struct FieldTypeInfo {
    name: String,
    optional: bool,
    type_: String
}



#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut executor = Executor::connect("http://[::1]:50052".to_string()).await?;

    // print summary info about rpc services, enums, types...
    println!("INFO:\n{}", serde_json::to_string_pretty(&executor.info()?)?);


    /*

    // parse filedescriptorproto

    let fdp_base64 = "ChBoZWxsb3dvcmxkLnByb3RvEgpoZWxsb3dvcmxkIiIKDEhlbGxvUmVxdWVzdBISCgRuYW1lGAEgASgJUgRuYW1lIiYKCkhlbGxvUmVwbHkSGAoHbWVzc2FnZRgBIAEoCVIHbWVzc2FnZTJHCgdHcmVldGVyEjwKCFNheUhlbGxvEhguaGVsbG93b3JsZC5IZWxsb1JlcXVlc3QaFi5oZWxsb3dvcmxkLkhlbGxvUmVwbHlKiQIKBhIEAAANAQoICgEMEgMAABIKCAoBAhIDAQATCgoKAgYAEgQDAAUBCgoKAwYAARIDAwgPCgsKBAYAAgASAwQCMwoMCgUGAAIAARIDBAYOCgwKBQYAAgACEgMEEBwKDAoFBgACAAMSAwQnMQoKCgIEABIEBwAJAQoKCgMEAAESAwcIFAoLCgQEAAIAEgMIAhIKDAoFBAACAAUSAwgCCAoMCgUEAAIAARIDCAkNCgwKBQQAAgADEgMIEBEKCgoCBAESBAsADQEKCgoDBAEBEgMLCBIKCwoEBAECABIDDAIVCgwKBQQBAgAFEgMMAggKDAoFBAECAAESAwwJEAoMCgUEAQIAAxIDDBMUYgZwcm90bzM=".to_string();
    let fdp = base64::decode(fdp_base64).unwrap();

    let t = FileDescriptorProto::parse_from_bytes(&fdp).unwrap();

    for service in &t.service {
        for method in &service.method {
            println!("service: {}, method: {}, input: {}, output: {}", service.get_name(), method.get_name(), method.get_input_type(), method.get_output_type());
        }
    }

    for t1 in &t.enum_type {
        println!("enum: {:?}", t1);
    }

    for t2 in &t.message_type {
        println!("messages: {:?}", t2);
    }

    // try to construct message

    let fd = FileDescriptor::new_dynamic(t, vec![]);

    // println!("{:?}", );

    let m = fd.message_by_package_relative_name("HelloRequest").unwrap();
    let mut msg = m.new_instance();

    let tt1 = parse_dynamic_from_str(&m, r#"{"name": "sergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergey"}"#).unwrap();

    let export = tt1.write_to_bytes_dyn().unwrap();

    println!("sent: {:?}", export);  // is it right???

    let m1 = fd.message_by_package_relative_name("HelloReply").unwrap();
    let mut msg1 = m.new_instance();

    let tt2 = parse_dynamic_from_str(&m1, r#"{"message": "1ello sergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergeysergey!"}"#).unwrap();

    let export2 = tt2.write_to_bytes_dyn().unwrap();

    println!("should be: {:?}", export2);  // is it right???


    // now we wan't to sent tt1 to server!

    let r: Request<Message1> = tonic::Request::new(Message1 {
        bytes: export
    });

    let mut conn = tonic::transport::Endpoint::new("http://[::1]:50052")?.connect().await?;
    let mut grpc = tonic::client::Grpc::new(conn);

    grpc.ready().await?;

    let codec: ProstCodec<Message1, Message1> = tonic::codec::ProstCodec::default();
    let path = http::uri::PathAndQuery::from_static("/helloworld.Greeter/SayHello");

    let result: tonic::Response<Message1> = grpc.unary(r.into_request(), path, codec).await?;

    //println!("result: {:?}", result.into_inner().bytes);

    let mut output = result.into_inner().bytes.clone();
    println!("actually: {:?}", output);

    // wtf is this? but it work!
    output.insert(0, 10);

    let mut out = m1.new_instance();
    let m = out.merge_from_bytes_dyn(&output);

    println!("{}", print_to_string(&*out).unwrap());



    // -----

    let start = time::Instant::now();
    let outbound = async_stream::stream! {
        let mut interval = time::interval(Duration::from_secs(1));

        while let time = interval.tick().await {
            let elapsed = time.duration_since(start);
            let note = ServerReflectionRequest {
                host: "asd".to_string(),
                message_request: Some(MessageRequest::FileByFilename("1".to_string()))
            };

            yield note;
        }
    };

    let response = client.server_reflection_info(tonic::Request::new(outbound)).await?;
    let mut inbound = response.into_inner();

    while let Some(note) = inbound.message().await? {
        println!("NOTE = {:?}", note);
    }


    let request = tonic::Request::new(ServerReflectionRequest {
        host: "asd".to_string(),
        message_request: Some(MessageRequest::ListServices("".to_string()))
    });

    let resp = client.server_reflection_info(request).await?;

     */

    Ok(())
}
