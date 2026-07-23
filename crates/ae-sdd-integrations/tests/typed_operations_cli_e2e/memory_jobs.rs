use ae_sdd_protocol::{ClientKind, RpcMethod};
use serde_json::{Value, json};

use super::support::*;

#[test]
fn memory_jobs_run_through_the_daemon_with_bound_session_lineage() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(
        &harness,
        &mut cli,
        &workspace,
        "memory-root",
        "memory-agent",
    );
    let identity = identity(&workspace, &root, "memory-agent");

    let create = submit_job(
        &harness,
        &mut cli,
        &identity,
        "memory.create",
        json!({
            "entityType":"story",
            "entityId":"STORY-TYPED-E2E",
            "context":{
                "current_series":"story",
                "next_step":"coding",
                "constraints":["daemon-owned memory sentinel"]
            }
        }),
        "memory-create-job",
    );
    assert_eq!(create["result"]["outcome"], "PASS");
    assert_eq!(create["result"]["replayed"], false);

    let read = submit_job(
        &harness,
        &mut cli,
        &identity,
        "memory.read",
        json!({"entityType":"story","entityId":"STORY-TYPED-E2E"}),
        "memory-read-job",
    );
    assert_eq!(read["result"]["found"], true);
    assert!(
        read["result"]["context"]
            .as_str()
            .is_some_and(|text| text.contains("daemon-owned memory sentinel"))
    );

    let search = submit_job(
        &harness,
        &mut cli,
        &identity,
        "memory.search",
        json!({"query":"memory sentinel","limit":10}),
        "memory-search-job",
    );
    assert_eq!(search["result"]["count"], 1);

    let replay_arguments = json!({
        "entityType":"story",
        "entityId":"STORY-TYPED-E2E",
        "context":{
            "current_series":"story",
            "next_step":"coding",
            "constraints":["daemon-owned memory sentinel"]
        }
    });
    let submitted = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::JobSubmit,
        job_params(
            &identity,
            "memory.create",
            replay_arguments.clone(),
            "memory-create-job",
        ),
    ));
    let replayed = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::JobSubmit,
        job_params(
            &identity,
            "memory.create",
            replay_arguments,
            "memory-create-job",
        ),
    ));
    assert_eq!(submitted["jobId"], replayed["jobId"]);
}

fn submit_job(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
    entrypoint: &str,
    arguments: Value,
    key: &str,
) -> Value {
    let submitted = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::JobSubmit,
        job_params(identity, entrypoint, arguments, key),
    ));
    assert_eq!(submitted["status"], "queued");
    assert!(harness.runtime.run_one_pending_job().expect("job executes"));
    let status = trusted_params(identity, json!({"jobId":submitted["jobId"]}));
    let completed = success(&call(&harness.runtime, cli, RpcMethod::JobStatus, status));
    assert_eq!(completed["status"], "pass", "{completed}");
    completed
}

fn job_params(
    identity: &CliIdentity,
    entrypoint: &str,
    arguments: Value,
    key: &str,
) -> ae_sdd_protocol::RequestParams<Value> {
    let mut request = trusted_params(
        identity,
        json!({
            "entrypoint":entrypoint,
            "arguments":arguments,
            "deadlineUnixMs":300_000,
        }),
    );
    request.idempotency_key = Some(key.to_owned());
    request
}
