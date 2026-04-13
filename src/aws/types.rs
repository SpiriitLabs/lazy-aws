use serde::Deserialize;

/// AWS caller identity from `sts get-caller-identity`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct CallerIdentity {
    pub account: String,
    pub arn: String,
    pub user_id: String,
}

/// ECS Cluster.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Cluster {
    pub cluster_arn: String,
    pub cluster_name: String,
    pub status: String,
    pub running_tasks_count: i32,
    pub pending_tasks_count: i32,
    pub active_services_count: i32,
    pub registered_container_instances_count: i32,
}

/// ECS Service.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Service {
    pub service_arn: String,
    pub service_name: String,
    pub cluster_arn: String,
    pub status: String,
    pub desired_count: i32,
    pub running_count: i32,
    pub pending_count: i32,
    pub launch_type: String,
    pub task_definition: String,
    pub enable_execute_command: bool,
    pub deployments: Vec<Deployment>,
    pub load_balancers: Vec<LoadBalancer>,
}

/// ECS Deployment.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Deployment {
    pub id: String,
    pub status: String,
    pub task_definition: String,
    pub desired_count: i32,
    pub running_count: i32,
    pub rollout_state: String,
    pub created_at: String,
}

/// ECS Load Balancer.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoadBalancer {
    pub target_group_arn: String,
    pub container_name: String,
    pub container_port: i32,
}

/// ECS Task.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Task {
    pub task_arn: String,
    pub task_definition_arn: String,
    pub cluster_arn: String,
    pub last_status: String,
    pub desired_status: String,
    pub started_at: Option<String>,
    pub connectivity: Option<String>,
    pub health_status: Option<String>,
    pub launch_type: String,
    pub containers: Vec<Container>,
    pub enable_execute_command: bool,
}

/// ECS Container.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Container {
    pub container_arn: Option<String>,
    pub name: String,
    pub image: String,
    pub last_status: String,
    pub health_status: Option<String>,
    pub runtime_id: Option<String>,
}

/// SSM-managed EC2 instance.
#[derive(Debug, Clone, Default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub state: String,
    pub instance_type: String,
    pub platform: String,
    pub private_ip: String,
    pub public_ip: Option<String>,
    pub availability_zone: String,
    pub ssm_ping_status: String,
    pub ssm_agent_version: Option<String>,
}

/// CloudWatch Log Group.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LogGroup {
    pub log_group_name: String,
    pub arn: String,
    pub retention_in_days: Option<i32>,
    pub stored_bytes: Option<i64>,
    pub creation_time: Option<i64>,
}

/// CloudWatch Log Stream.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LogStream {
    pub log_stream_name: String,
    pub arn: String,
    pub first_event_timestamp: Option<i64>,
    pub last_event_timestamp: Option<i64>,
    pub last_ingestion_time: Option<i64>,
}

/// CloudWatch Log Event.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LogEvent {
    pub timestamp: i64,
    pub message: String,
    #[serde(rename = "ingestionTime")]
    pub ingestion_time: Option<i64>,
}
