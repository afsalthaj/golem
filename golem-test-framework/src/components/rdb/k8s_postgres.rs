// Copyright 2024 Golem Cloud
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::components::k8s::{
    K8sNamespace, K8sPod, K8sRouting, K8sRoutingType, K8sService, ManagedPod, ManagedService,
    Routing,
};
use crate::components::rdb::{assert_connection, DbInfo, PostgresInfo, Rdb};
use async_dropper_simple::{AsyncDrop, AsyncDropper};
use async_scoped::TokioScope;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::api::PostParams;
use kube::{Api, Client};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub struct K8sPostgresRdb {
    _namespace: K8sNamespace,
    pod: Arc<Mutex<K8sPod>>,
    service: Arc<Mutex<K8sService>>,
    routing: Arc<Mutex<K8sRouting>>,
    host: String,
    port: u16,
}

impl K8sPostgresRdb {
    pub async fn new(namespace: &K8sNamespace, routing_type: &K8sRoutingType) -> Self {
        info!("Creating Postgres pod");

        let pods: Api<Pod> = Api::namespaced(
            Client::try_default()
                .await
                .expect("Failed to create K8s client"),
            &namespace.0,
        );
        let services: Api<Service> = Api::namespaced(
            Client::try_default()
                .await
                .expect("Failed to create K8s client"),
            &namespace.0,
        );

        let pod: Pod = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": "golem-postgres",
                "labels": {
                    "app": "golem-postgres",
                    "app-group": "golem"
                },
            },
            "spec": {
                "ports": [{
                    "port": 5432,
                    "protocol": "TCP"
                }],
                "containers": [{
                    "name": "postgres",
                    "image": "postgres:12",
                    "env": [
                        {"name": "POSTGRES_DB", "value": "postgres"},
                        {"name": "POSTGRES_USER", "value": "postgres"},
                        {"name": "POSTGRES_PASSWORD", "value": "postgres"}
                    ]
                }]
            }
        }))
        .expect("Failed to deserialize pod definition");

        let pp = PostParams::default();

        let _res_pod = pods.create(&pp, &pod).await.expect("Failed to create pod");
        let managed_pod = AsyncDropper::new(ManagedPod::new("golem-postgres", namespace));

        let service: Service = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": "golem-postgres",
                "labels": {
                    "app": "golem-postgres",
                    "app-group": "golem"
                },
            },
            "spec": {
                "ports": [{
                    "port": 5432,
                    "protocol": "TCP"
                }],
                "selector": { "app": "golem-postgres" },
                "type": "LoadBalancer"
            }
        }))
        .expect("Failed to deserialize service description");

        let _res_srv = services
            .create(&pp, &service)
            .await
            .expect("Failed to create service");
        let managed_service = AsyncDropper::new(ManagedService::new("golem-postgres", namespace));

        let Routing {
            hostname: local_host,
            port: local_port,
            routing: managed_routing,
        } = Routing::create("golem-postgres", 5432, namespace, routing_type).await;

        let host = format!("golem-postgres.{}.svc.cluster.local", &namespace.0);
        let port = 5432;

        assert_connection(&local_host, local_port).await;

        info!("Test Postgres started on private host {host}:{port}, accessible from localhost as {local_host}:{local_port}");

        Self {
            _namespace: namespace.clone(),
            host,
            port,
            pod: Arc::new(Mutex::new(managed_pod)),
            service: Arc::new(Mutex::new(managed_service)),
            routing: Arc::new(Mutex::new(managed_routing)),
        }
    }
}

impl Rdb for K8sPostgresRdb {
    fn info(&self) -> DbInfo {
        DbInfo::Postgres(PostgresInfo {
            host: self.host.clone(),
            port: self.port,
            host_port: self.port,
            database_name: "postgres".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string(),
        })
    }

    fn kill(&self) {
        TokioScope::scope_and_block(|s| {
            s.spawn(async move {
                let mut pod = self.pod.lock().await;
                pod.inner_mut().async_drop().await;
                let mut service = self.service.lock().await;
                service.inner_mut().async_drop().await;
                let mut routing = self.routing.lock().await;
                routing.inner_mut().async_drop().await;
            })
        });
    }
}
