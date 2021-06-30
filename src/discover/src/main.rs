use reflection::server_reflection_client::ServerReflectionClient;
use reflection::server_reflection_request::MessageRequest;
use reflection::{ServerReflectionRequest};
use tokio::time;
use std::time::Duration;
use protobuf::descriptor::{FileDescriptorProto, FileDescriptorSet};
use protobuf::{Message, MessageDyn};
use prost_types::DescriptorProto;
use serde_protobuf::{descriptor, de};
use protobuf::reflect::{FileDescriptor, MessageDescriptor};
use protobuf::json::{parse_dynamic_from_str, print_to_string};
use tonic::codegen::http;
use tonic::{IntoRequest, Request};
use prost::bytes::{Buf, BufMut};
use prost::DecodeError;
use prost::encoding::{WireType, DecodeContext};
use tonic::codec::ProstCodec;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ServerReflectionClient::connect("http://[::1]:50052").await?;

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

    /*
    let request = tonic::Request::new(ServerReflectionRequest {
        host: "asd".to_string(),
        message_request: Some(MessageRequest::ListServices("".to_string()))
    });

    let resp = client.server_reflection_info(request).await?;

     */

    Ok(())
}
