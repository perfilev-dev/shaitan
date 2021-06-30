use shaitan_sdk::{Plugin, InvocationError, ServerCommand, RegisterCommand, WorkerCommand, ResultCommand, ErrorResponse};
use std::collections::HashMap;
use dyn_clone::DynClone;
use threadpool::ThreadPool;
use serde_json::{Value, json, Map};
use schemars::{JsonSchema, schema_for};
use std::sync::{Arc, Mutex};
use zeromq::{Socket, BlockingRecv, BlockingSend, ReqSocket};
use std::convert::TryInto;
use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

/// EXAMPLE PLUGIN BELOW

#[derive(Default, Debug, Clone)]
pub struct Swap;

#[derive(JsonSchema)]
pub struct Input(Map<String, Value>);

#[derive(JsonSchema)]
pub struct Output(Map<String, Value>);

#[derive(JsonSchema)]
pub struct Parameters {
    pub field_a_path: String,
    pub field_b_path: String
}

impl Plugin for Swap {
    fn name(&self) -> &'static str {
        "swap"
    }

    fn process(&self, input: &Value) -> Result<Value, InvocationError> {
        println!("[Swap] Process...");

        // do minimal, swap .a with .b
        if let Value::Object(map) = input {
            let mut map_output = map.clone();
            if let Some(a) = map.get("a") {
                if let Some(b) = map.get("b") {
                    map_output.insert("a".to_string(), b.clone());
                    map_output.insert("b".to_string(), a.clone());
                }
            }
            Ok(Value::Object(map_output))
        } else {
            Err(InvocationError::Other { msg: "not a map".to_string() })
        }
    }

    fn input_schema(&self) -> String {
        serde_json::to_string_pretty(&schema_for!(Input)).unwrap()
    }

    fn output_schema(&self) -> String {
        serde_json::to_string_pretty(&schema_for!(Output)).unwrap()
    }

    fn parameters_schema(&self) -> String {
        serde_json::to_string_pretty(&schema_for!(Parameters)).unwrap()
    }
}

unsafe impl Send for Swap {}


/// PLUGINS CONTAINER!

#[derive(Default, Debug)]
pub struct Plugins {
    plugins: HashMap<String, Box<dyn Plugin>>
}

impl Clone for Plugins {
    fn clone(&self) -> Self {
        let mut new = Self {
            plugins: HashMap::new()
        };

        for (name, p) in self.plugins.iter() {
            new.plugins.insert(name.to_string(), dyn_clone::clone_box(&**p));
        }

        new
    }
}

impl Plugins {

    pub fn load(&mut self) {
        let p = Box::new(Swap::default()) as Box<dyn Plugin>;
        self.plugins.insert(p.name().to_string(), p);
    }

}


async fn send_result(sock: &Mutex<ReqSocket>, message: &str) -> Result<(), Box<dyn Error>> {
    if let Ok(mut socket) = sock.lock() {
        socket.send(message.into()).await?;
        let repl: String = socket.recv().await?.try_into()?;
        dbg!(repl); // here should be ok!
    }

    Ok(())
}


#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let mut plugins = Plugins::default();

    // load plugins...
    plugins.load();

    // create connections...
    let mut socket1 = zeromq::ReqSocket::new();
    socket1
        .connect("ipc://shaitan.sock")
        .await
        .expect("Failed to connect");

    // and send info about plugins loaded.
    socket1.send(serde_json::to_string(&ServerCommand::Register(RegisterCommand {
        foo: "111".to_string()
    }))?.into()).await?;

    // also create connection for feedback.
    let mut socket2 = Arc::new(Mutex::new(zeromq::ReqSocket::new()));
    socket2
        .lock().unwrap()
        .connect("ipc://shaitan.sock")
        .await
        .expect("Failed to connect");

    let pool = ThreadPool::new(2);
    loop {
        let raw: String = socket1.recv().await?.try_into()?;
        let command: WorkerCommand = serde_json::from_str(&raw)?;

        match command {
            WorkerCommand::Task(task) => {
                println!("Got new task! {:?}", task);

                let plugins2 = plugins.clone();
                let sock = socket2.clone();

                pool.execute(move || {
                    println!("do task now!!!");

                    let mut result = ResultCommand {
                        task_id: task.task_id,
                        output: Default::default(),
                        error: None
                    };

                    match plugins2.plugins.get(&task.plugin_name) {
                        Some(plugin) => {
                            match plugin.process(&task.input) {
                                Ok(output) => {
                                    result.output = output;
                                },
                                Err(e) => {
                                    result.error = Some(ErrorResponse {
                                        msg: format!("some invocation error")
                                    });
                                }
                            }
                        },
                        None => {
                            result.error = Some(ErrorResponse {
                                msg: format!("no plugin named {}", task.plugin_name)
                            });
                        }
                    }

                    // and make response! =)
                    let response = serde_json::to_string(&ServerCommand::Result(
                        result
                    )).unwrap();

                    async_std::task::block_on(send_result(&sock, &response)).unwrap();
                });
            }
        }

        socket1.send(serde_json::to_string(&ServerCommand::Ack)?.into()).await?;
        sleep(Duration::from_secs(1));
    }

    pool.join();
    println!("...");

    Ok(())
}
