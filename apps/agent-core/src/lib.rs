pub mod agent;
pub mod api;
pub mod crypto;
pub mod domain;
pub mod jobs;
pub mod policy;
pub mod skills;
pub mod storage;

pub mod proto {
    tonic::include_proto!("teamsagent.v1");
}
