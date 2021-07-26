use async_channel::{Sender, Receiver};
use std::sync::Arc;
use futures::future::select_all;
use std::collections::HashMap;
use std::thread;
use std::thread::JoinHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{self, Duration};
use crate::{schedule_task, get_pipeline_queue, schedule_pipeline_task, proto::result_json};


#[derive(Clone)]
pub struct Pipeline {
    pub name: String,
    pub jobs: Vec<Job>,
    pub input_rx: async_broadcast::Receiver<String>,
    pub input_tx: async_broadcast::Sender<String>,
    pub output_rx: async_broadcast::Receiver<String>,
    pub output_tx: async_broadcast::Sender<String>,
    pub is_stopped: Arc<AtomicBool>
}

impl Pipeline {
    pub fn new(name: &str) -> Self {
        let (tx1, rx1) = async_broadcast::broadcast(10);
        let (tx2, rx2) = async_broadcast::broadcast(10);

        Pipeline {
            name: name.to_string(),
            jobs: vec![],
            input_rx: rx1,
            input_tx: tx1,
            output_rx: rx2,
            output_tx: tx2,
            is_stopped: Arc::new(AtomicBool::new(true))
        }
    }

    // pipeline: 3x + 2
    pub fn example1() -> Self {
        let mut pipeline = Pipeline::new("3x + 2");

        // job 3x
        let mut job1 = Job::new("math.SimpleMath/Mul3");
        job1.accepts.push(pipeline.input_rx.clone());

        // job y+2
        let mut job2 = Job::new("math.SimpleMath/Add2");
        job2.accepts.push(job1.output_rx.clone());

        // output job!
        let mut output = Job::new("output");
        output.accepts.push(job2.output_rx.clone());

        // push jobs to pipeline
        pipeline.jobs.push(job1);
        pipeline.jobs.push(job2);
        pipeline.jobs.push(output);

        pipeline
    }

    // pipeline: 3(x + 2)
    pub fn example2() -> Self {
        let mut pipeline = Pipeline::new("3(x + 2)");

        // job 3x
        let mut job1 = Job::new("y+2");
        job1.accepts.push(pipeline.input_rx.clone());

        // job y+2
        let mut job2 = Job::new("3*x");
        job2.accepts.push(job1.output_rx.clone());

        // output job!
        let mut output = Job::new("output");
        output.accepts.push(job2.output_rx.clone());

        // push jobs to pipeline
        pipeline.jobs.push(job1);
        pipeline.jobs.push(job2);
        pipeline.jobs.push(output);

        pipeline
    }

    // starts pipeline thread...
    pub fn start(&mut self) {
        self.is_stopped.store(false, Ordering::SeqCst);

        let mut pipeline = self.clone();
        tokio::spawn(async move {
            println!("pipeline {} is started!", pipeline.name);

            // prepare pipeline receiver!
            let (pipeline_id, results_queue) = get_pipeline_queue().await;

            // task_id -> job index
            let mut task_job_map = HashMap::<u32, usize>::new();

            while !pipeline.is_stopped.load(Ordering::SeqCst) {
                println!("waiting...");

                let mut recv = vec![];
                let mut accept_job = HashMap::new();

                let mut i = 0;
                for j in &mut pipeline.jobs {
                    for mut a in &mut j.accepts {
                        accept_job.insert(recv.len(), i as usize);
                        recv.push(a.recv());
                    }
                    i += 1;
                }

                let sleep = time::sleep(Duration::from_millis(50));
                tokio::pin!(sleep);

                let job_triggers = select_all(recv);
                tokio::select! {
                    (job_accept, i, _) = job_triggers => {
                        let job_index = *accept_job.get(&i).unwrap();
                        let job = &pipeline.jobs[job_index];
                        println!("job {} got {:?}", job.name, job_accept);

                        // if job.name == "3*x" {
                        //     job.output_tx.broadcast(3*(job_accept.unwrap())).await.unwrap();
                        // }
                        //
                        // if job.name == "y+2" {
                        //     job.output_tx.broadcast(2+job_accept.unwrap()).await.unwrap();
                        // }

                        // output is specific job!
                        if job.name == "output" {
                            pipeline.output_tx.broadcast(job_accept.unwrap()).await.unwrap();
                        } else {
                            let task_id = schedule_pipeline_task(pipeline_id, &job.name, &job_accept.ok().unwrap()).await.unwrap();
                            task_job_map.insert(task_id, job_index);
                        }
                    },
                    result = results_queue.recv() => {
                        println!("got result in pipeline!!!");

                        if let Ok(res) = result {
                            let job_index = task_job_map.get(&res.task_id).unwrap();
                            let job = &pipeline.jobs[*job_index];

                            if let result_json::R::Output(out) = res.r.unwrap() {
                                job.output_tx.broadcast(out).await.unwrap();
                            }

                        }

                    },
                    _ = &mut sleep => {
                        println!("timeout");

                        // fixme: know when stop!
                        break;
                    }
                }
            }

            println!("pipeline {} is stopped!", pipeline.name);
        });
    }

    // stops pipeline thread...
    pub fn stop(&self) {
        self.is_stopped.store(true, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct Job {
    pub name: String,
    pub accepts: Vec<async_broadcast::Receiver<String>>,
    pub output_rx: async_broadcast::Receiver<String>,
    pub output_tx: async_broadcast::Sender<String>,
}

impl Job {
    pub fn new(name: &str) -> Self {
        let (tx, rx) = async_broadcast::broadcast(10);

        Job {
            name: name.to_string(),
            accepts: vec![],
            output_rx: rx,
            output_tx: tx
        }
    }
}

pub enum InnerJob {
    Method(Method),
    Pipeline(Pipeline)
}

pub struct Method {

}

pub struct Output {
    pub accepts: Vec<async_broadcast::Receiver<String>>,
    pub output_rx: async_broadcast::Receiver<String>,
    pub output_tx: async_broadcast::Sender<String>,
}

impl Output {
    pub fn new(accepts: Vec<async_broadcast::Receiver<String>>) -> Self {
        let (tx, rx) = async_broadcast::broadcast(10);

        Output {
            accepts,
            output_rx: rx,
            output_tx: tx
        }
    }
}


pub async fn test() {

    // pipeline: 3x + 2
    let mut pipeline = Pipeline::example1();

    pipeline.start();

    // send example input to pipeline
    pipeline.input_tx.broadcast(r#"{"value": 2}"#.to_string()).await.unwrap();

    // and fetch result back from output queue.
    let result = pipeline.output_rx.recv().await.unwrap();

    pipeline.stop();

    // and print result =)
    println!("3*2+2={}", result);
    println!("done!");
}