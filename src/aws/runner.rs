use crate::aws::exec::{Executor, RunResult, StreamHandle};
use crate::aws::types::*;

/// Runner provides typed methods over the AWS CLI.
pub struct Runner {
    exec: Box<dyn Executor>,
}

impl Runner {
    pub fn new(exec: Box<dyn Executor>) -> Self {
        Runner { exec }
    }

    pub fn is_available(&self) -> bool {
        self.exec.run(&["--version"]).is_ok()
    }

    pub fn version(&self) -> Result<String, String> {
        let result = self.exec.run(&["--version"])?;
        Ok(String::from_utf8_lossy(&result.stdout).trim().to_string())
    }

    pub fn bin_path(&self) -> String {
        self.exec.look_path()
    }

    pub fn profile(&self) -> String {
        self.exec.profile()
    }

    pub fn region(&self) -> String {
        self.exec.region()
    }

    pub fn get_caller_identity(&self) -> Result<CallerIdentity, String> {
        let result = self.exec.run(&["sts", "get-caller-identity"])?;
        check_exit(&result)?;
        serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))
    }

    pub fn list_clusters(&self) -> Result<Vec<Cluster>, String> {
        let result = self.exec.run(&["ecs", "list-clusters"])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let arns: Vec<String> = parsed["clusterArns"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if arns.is_empty() {
            return Ok(vec![]);
        }

        let mut args = vec!["ecs", "describe-clusters", "--clusters"];
        let arn_refs: Vec<&str> = arns.iter().map(|s| s.as_str()).collect();
        args.extend(arn_refs.iter());

        let result = self.exec.run(&args)?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let clusters: Vec<Cluster> = parsed["clusters"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(clusters)
    }

    pub fn list_services(&self, cluster: &str) -> Result<Vec<Service>, String> {
        let result = self
            .exec
            .run(&["ecs", "list-services", "--cluster", cluster])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let arns: Vec<String> = parsed["serviceArns"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if arns.is_empty() {
            return Ok(vec![]);
        }

        let mut args = vec![
            "ecs",
            "describe-services",
            "--cluster",
            cluster,
            "--services",
        ];
        let arn_refs: Vec<&str> = arns.iter().map(|s| s.as_str()).collect();
        args.extend(arn_refs.iter());

        let result = self.exec.run(&args)?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let services: Vec<Service> = parsed["services"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(services)
    }

    pub fn list_tasks(&self, cluster: &str, service: &str) -> Result<Vec<Task>, String> {
        let result = self.exec.run(&[
            "ecs",
            "list-tasks",
            "--cluster",
            cluster,
            "--service-name",
            service,
        ])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let arns: Vec<String> = parsed["taskArns"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if arns.is_empty() {
            return Ok(vec![]);
        }

        let mut args = vec!["ecs", "describe-tasks", "--cluster", cluster, "--tasks"];
        let arn_refs: Vec<&str> = arns.iter().map(|s| s.as_str()).collect();
        args.extend(arn_refs.iter());

        let result = self.exec.run(&args)?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let tasks: Vec<Task> = parsed["tasks"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(tasks)
    }

    pub fn force_new_deployment(
        &self,
        cluster: &str,
        service: &str,
    ) -> Result<StreamHandle, String> {
        self.exec.stream(&[
            "ecs",
            "update-service",
            "--cluster",
            cluster,
            "--service",
            service,
            "--force-new-deployment",
        ])
    }

    pub fn list_log_groups(&self, prefix: Option<&str>) -> Result<Vec<LogGroup>, String> {
        let mut args = vec!["logs", "describe-log-groups"];
        let prefix_str;
        if let Some(p) = prefix {
            prefix_str = p.to_string();
            args.push("--log-group-name-prefix");
            args.push(&prefix_str);
        }
        let result = self.exec.run(&args)?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let groups: Vec<LogGroup> = parsed["logGroups"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(groups)
    }

    pub fn list_log_streams(&self, group: &str) -> Result<Vec<LogStream>, String> {
        let result = self.exec.run(&[
            "logs",
            "describe-log-streams",
            "--log-group-name",
            group,
            "--order-by",
            "LastEventTime",
            "--descending",
        ])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let streams: Vec<LogStream> = parsed["logStreams"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(streams)
    }

    pub fn tail_logs(&self, group: &str, since: &str) -> Result<StreamHandle, String> {
        self.exec
            .stream(&["logs", "tail", group, "--follow", "--since", since])
    }

    pub fn list_instances(&self) -> Result<Vec<Instance>, String> {
        // Get SSM-managed instances
        let result = self.exec.run(&["ssm", "describe-instance-information"])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let ssm_instances = parsed["InstanceInformationList"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        if ssm_instances.is_empty() {
            return Ok(vec![]);
        }

        // Collect instance IDs for EC2 describe
        let instance_ids: Vec<String> = ssm_instances
            .iter()
            .filter_map(|i| i["InstanceId"].as_str().map(|s| s.to_string()))
            .collect();

        // Get EC2 metadata
        let mut args = vec!["ec2", "describe-instances", "--instance-ids"];
        let id_refs: Vec<&str> = instance_ids.iter().map(|s| s.as_str()).collect();
        args.extend(id_refs.iter());

        let ec2_result = self.exec.run(&args);
        let ec2_map: std::collections::HashMap<String, serde_json::Value> = if let Ok(result) =
            ec2_result
        {
            if result.exit_code == 0 {
                let parsed: serde_json::Value =
                    serde_json::from_slice(&result.stdout).unwrap_or_default();
                parsed["Reservations"]
                    .as_array()
                    .map(|reservations| {
                        reservations
                            .iter()
                            .flat_map(|r| r["Instances"].as_array().cloned().unwrap_or_default())
                            .filter_map(|inst| {
                                let id = inst["InstanceId"].as_str()?.to_string();
                                Some((id, inst))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            }
        } else {
            std::collections::HashMap::new()
        };

        // Merge SSM + EC2 data
        let instances = ssm_instances
            .iter()
            .map(|ssm| {
                let id = ssm["InstanceId"].as_str().unwrap_or("").to_string();
                let ec2 = ec2_map.get(&id);

                // Get Name tag from EC2
                let name = ec2
                    .and_then(|e| {
                        e["Tags"].as_array().and_then(|tags| {
                            tags.iter().find_map(|t| {
                                if t["Key"].as_str() == Some("Name") {
                                    t["Value"].as_str().map(|s| s.to_string())
                                } else {
                                    None
                                }
                            })
                        })
                    })
                    .unwrap_or_default();

                Instance {
                    id: id.clone(),
                    name,
                    state: ec2
                        .and_then(|e| e["State"]["Name"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    instance_type: ec2
                        .and_then(|e| e["InstanceType"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    platform: ssm["PlatformName"].as_str().unwrap_or("").to_string(),
                    private_ip: ec2
                        .and_then(|e| e["PrivateIpAddress"].as_str())
                        .or_else(|| ssm["IPAddress"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    public_ip: ec2
                        .and_then(|e| e["PublicIpAddress"].as_str().map(|s| s.to_string())),
                    availability_zone: ec2
                        .and_then(|e| e["Placement"]["AvailabilityZone"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    ssm_ping_status: ssm["PingStatus"].as_str().unwrap_or("").to_string(),
                    ssm_agent_version: ssm["AgentVersion"].as_str().map(|s| s.to_string()),
                }
            })
            .collect();

        Ok(instances)
    }

    pub fn stop_task(&self, cluster: &str, task: &str) -> Result<(), String> {
        let result = self
            .exec
            .run(&["ecs", "stop-task", "--cluster", cluster, "--task", task])?;
        check_exit(&result)?;
        Ok(())
    }

    pub fn start_insights_query(
        &self,
        group: &str,
        query: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<String, String> {
        let start = start_time.to_string();
        let end = end_time.to_string();
        let result = self.exec.run(&[
            "logs",
            "start-query",
            "--log-group-name",
            group,
            "--start-time",
            &start,
            "--end-time",
            &end,
            "--query-string",
            query,
        ])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        parsed["queryId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no queryId in response".to_string())
    }

    #[allow(clippy::type_complexity)]
    pub fn get_insights_results(
        &self,
        query_id: &str,
    ) -> Result<(String, Vec<Vec<(String, String)>>), String> {
        let result = self
            .exec
            .run(&["logs", "get-query-results", "--query-id", query_id])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let status = parsed["status"].as_str().unwrap_or("Unknown").to_string();

        let results: Vec<Vec<(String, String)>> = parsed["results"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        row.as_array()
                            .map(|fields| {
                                fields
                                    .iter()
                                    .filter_map(|f| {
                                        let field = f["field"].as_str()?.to_string();
                                        let value = f["value"].as_str()?.to_string();
                                        Some((field, value))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok((status, results))
    }

    /// Lists configured AWS profiles via `aws configure list-profiles`.
    pub fn list_profiles(&self) -> Result<Vec<String>, String> {
        let result = self.exec.run(&["configure", "list-profiles"])?;
        // This command returns plain text (one profile per line), not JSON.
        // It ignores --output json, so we parse stdout as text.
        let text = String::from_utf8_lossy(&result.stdout);
        let profiles: Vec<String> = text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        Ok(profiles)
    }

    pub fn list_db_instances(&self) -> Result<Vec<DbInstance>, String> {
        let result = self.exec.run(&["rds", "describe-db-instances"])?;
        check_exit(&result)?;

        let parsed: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| format!("parse error: {e}"))?;

        let instances: Vec<DbInstance> = parsed["DBInstances"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(instances)
    }

    /// Returns the aws CLI binary path and args needed to run `aws sso login --profile <profile>`.
    /// The caller is responsible for running this command interactively (suspend TUI).
    pub fn sso_login_command(&self, profile: &str) -> (String, Vec<String>) {
        let bin = self.exec.bin();
        let parts: Vec<&str> = bin.split_whitespace().collect();
        let cmd = parts[0].to_string();
        let mut args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        args.extend([
            "sso".to_string(),
            "login".to_string(),
            "--profile".to_string(),
            profile.to_string(),
        ]);
        (cmd, args)
    }
}

fn check_exit(result: &RunResult) -> Result<(), String> {
    if result.exit_code != 0 {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!(
            "aws CLI error (exit {}): {}",
            result.exit_code,
            stderr.trim()
        ));
    }
    Ok(())
}
