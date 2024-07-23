use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::batch::v1::JobSpec;
use k8s_openapi::api::core::v1::{Container, PodSpec, PodTemplateSpec};
use kube::api::{ObjectMeta, PostParams};
use kube::{Api, Client};
use serde_json::json;

async fn create_k8s_job(client: Client, job_name: &str, image_uri: &str) -> kube::Result<Job> {
    let jobs: Api<Job> = Api::namespaced(client, "default");

    let job = Job {
        metadata: ObjectMeta {
            name: Some(job_name.to_string()),
            ..Default::default()
        },
        spec: Some(JobSpec {
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: job_name.to_string(),
                        image: Some(image_uri.to_string()),
                        ..Default::default()
                    }],
                    restart_policy: Some("Never".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    jobs.create(&PostParams::default(), &job).await
}

#[rstest::rstest]
fn test_create_k8s_job() {
    let job_name = "test-job";
    let image_uri = "alelat/wondera:latest";

    // let client = Client::try_default().unwrap();
    // let job = create_k8s_job(client, job_name, image_uri).block().unwrap();

    // assert_eq!(job.metadata.name.unwrap(), job_name);
    // assert_eq!(
    //     job.spec.unwrap().template.spec.unwrap().containers[0]
    //         .image
    //         .unwrap(),
    //     image_uri
    // );
}
