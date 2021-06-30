use zeromq::{Socket, BlockingRecv, BlockingSend};
use std::convert::TryInto;
use std::error::Error;
use shaitan_sdk::{ServerCommand, WorkerCommand, TaskCommand, ResultCommand};
use serde_json::json;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Start shaitan server");
    let mut socket = zeromq::RepSocket::new();
    std::fs::remove_file("shaitan.sock");
    socket.bind("ipc://shaitan.sock").await?;

    let mut queue = vec![
        TaskCommand {
            task_id: 0,
            plugin_name: "swap".to_string(),
            input: json!({
                "a": 1,
                "b": 2
            })
        },
        TaskCommand {
            task_id: 1,
            plugin_name: "swap".to_string(),
            input: json!({
                "a": 2,
                "b": 3
            })
        }
    ];

    let mut results: Vec<ResultCommand> = vec![];

    while results.len() < 2 {
        let raw: String = socket.recv().await?.try_into()?;
        let command = serde_json::from_str::<ServerCommand>(&raw)?;
        let mut response= "".to_string();

        dbg!(&command);
        match command {
            ServerCommand::Register(_) => {
                println!("Register new worker!");

                // here we should send tasks in loop!
                response = serde_json::to_string(&WorkerCommand::Task(queue.pop().unwrap()))?;
            }
            ServerCommand::Result(result) => {
                println!("Got result for task #{}", result.task_id);

                results.push(result);
            }
            ServerCommand::Ack => {
                println!("Ack!");

                // here we should send tasks in loop!
                if !queue.is_empty() {
                    response = serde_json::to_string(&WorkerCommand::Task(queue.pop().unwrap()))?;
                }
            }
        }

        socket.send(response.into()).await?;
    }

    Ok(())
}
