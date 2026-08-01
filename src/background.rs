use crate::search;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::mpsc;

pub struct QueryRequest {
    pub id: u64,
    pub path: PathBuf,
    pub criteria: search::SearchCriteria,
    pub columns: Vec<String>,
    pub offset: u32,
}

pub struct QueryResponse {
    pub id: u64,
    pub result: Result<search::SearchResult>,
}

pub struct QueryWorker {
    pub requests: mpsc::Sender<QueryRequest>,
    pub responses: mpsc::Receiver<QueryResponse>,
}

impl QueryWorker {
    pub fn new(page_size: u32) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<QueryRequest>();
        let (response_tx, response_rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = request_rx.recv() {
                while let Ok(newer) = request_rx.try_recv() {
                    request = newer;
                }
                let result = search::query(
                    &request.path,
                    &request.criteria,
                    &request.columns,
                    page_size,
                    request.offset,
                );
                if response_tx
                    .send(QueryResponse {
                        id: request.id,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            requests: request_tx,
            responses: response_rx,
        }
    }
}
