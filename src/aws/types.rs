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

/// RDS DB Instance.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct DbInstance {
    #[serde(rename = "DBInstanceIdentifier")]
    pub db_instance_identifier: String,
    #[serde(rename = "DBInstanceClass")]
    pub db_instance_class: String,
    pub engine: String,
    pub engine_version: String,
    #[serde(rename = "DBInstanceStatus")]
    pub db_instance_status: String,
    pub master_username: String,
    #[serde(rename = "DBName")]
    pub db_name: Option<String>,
    pub endpoint: Option<DbEndpoint>,
    pub allocated_storage: i32,
    pub instance_create_time: Option<String>,
    #[serde(rename = "MultiAZ")]
    pub multi_az: bool,
    pub publicly_accessible: bool,
    pub storage_type: String,
    #[serde(rename = "DBInstanceArn")]
    pub db_instance_arn: String,
    pub availability_zone: String,
    pub storage_encrypted: bool,
    #[serde(rename = "IAMDatabaseAuthenticationEnabled")]
    pub iam_database_authentication_enabled: bool,
    #[serde(rename = "VpcSecurityGroups")]
    pub vpc_security_groups: Vec<VpcSecurityGroup>,
    #[serde(rename = "DBSubnetGroup")]
    pub db_subnet_group: Option<DbSubnetGroup>,
}

/// RDS DB Endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct DbEndpoint {
    pub address: String,
    pub port: i32,
    pub hosted_zone_id: String,
}

/// VPC Security Group membership.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct VpcSecurityGroup {
    pub vpc_security_group_id: String,
    pub status: String,
}

/// RDS DB Subnet Group.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct DbSubnetGroup {
    #[serde(rename = "DBSubnetGroupName")]
    pub db_subnet_group_name: String,
    #[serde(rename = "VpcId")]
    pub vpc_id: String,
    pub subnet_group_status: String,
}

/// S3 Bucket.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Bucket {
    pub name: String,
    pub creation_date: String,
}

/// S3 Object metadata.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct S3Object {
    pub key: String,
    pub size: i64,
    pub last_modified: String,
    pub storage_class: String,
    #[serde(rename = "ETag")]
    pub e_tag: String,
}

/// S3 CommonPrefix (simulated folder from delimiter-based listing).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct S3CommonPrefix {
    pub prefix: String,
}

/// S3 list-objects-v2 response.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct S3ListResult {
    pub contents: Vec<S3Object>,
    pub common_prefixes: Vec<S3CommonPrefix>,
    pub is_truncated: bool,
    pub key_count: i64,
    pub next_continuation_token: Option<String>,
}
