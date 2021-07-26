use crate::pipelines::Pipeline;


pub struct Engine {
    pub pipelines: Vec<Pipeline>
}

impl Engine {

    pub fn new() -> Self {
        Engine {
            pipelines: vec![
                Pipeline::example1(),
                Pipeline::example2()
            ]
        }
    }

    pub fn start(&self) {

        // each pipeline has start?


    }

    pub fn stop(&self) {

    }

}