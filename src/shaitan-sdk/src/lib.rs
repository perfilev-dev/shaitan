use dyn_clone::DynClone;
use std::fmt::Debug;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

pub trait Plugin : DynClone + Debug + Send {
    /// Plugin name.
    fn name(&self) -> &'static str;
    /// Process input data and return some output.
    fn process(&self, input: &Value) -> Result<Value, InvocationError>;
    /// Input data Json Schema.
    fn input_schema(&self) -> String;
    /// Output data Json Schema.
    fn output_schema(&self) -> String;
    /// Parameters for plugin.
    fn parameters_schema(&self) -> String;
}

pub enum InvocationError {
    Other { msg: String },
}

/// These commands accepts by server.
#[derive(Serialize, Deserialize, Debug)]
pub enum ServerCommand {
    Register(RegisterCommand),
    Result(ResultCommand),
    Ack
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterCommand {
    pub foo: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResultCommand {
    pub task_id: u32,
    pub output: Value,
    pub error: Option<ErrorResponse>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterResponse {
    pub bar: String
}

/// These commands accepts by any worker.
#[derive(Serialize, Deserialize)]
pub enum WorkerCommand {
    Task(TaskCommand)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskCommand {
    pub task_id: u32,
    pub plugin_name: String,
    pub input: Value
}

/// With these responses worker replies.
#[derive(Serialize, Deserialize)]
pub enum WorkerResponse {

}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponse {
    pub msg: String
}