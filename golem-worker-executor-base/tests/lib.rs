use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ctor::{ctor, dtor};
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use golem_test_framework::components::rdb::Rdb;
use golem_test_framework::components::redis::provided::ProvidedRedis;
use golem_test_framework::components::redis::spawned::SpawnedRedis;
use golem_test_framework::components::redis::Redis;
use golem_test_framework::components::redis_monitor::spawned::SpawnedRedisMonitor;
use golem_test_framework::components::redis_monitor::RedisMonitor;
use golem_test_framework::components::shard_manager::ShardManager;
use golem_test_framework::components::template_service::filesystem::FileSystemTemplateService;
use golem_test_framework::components::template_service::TemplateService;
use golem_test_framework::components::worker_executor::provided::ProvidedWorkerExecutor;
use golem_test_framework::components::worker_executor::WorkerExecutor;
use golem_test_framework::components::worker_executor_cluster::WorkerExecutorCluster;
use golem_test_framework::components::worker_service::forwarding::ForwardingWorkerService;
use golem_test_framework::components::worker_service::WorkerService;
use golem_test_framework::config::TestDependencies;

mod common;

pub mod api;
pub mod blobstore;
pub mod guest_languages;
pub mod keyvalue;
pub mod rpc;
pub mod scalability;
pub mod transactions;
pub mod wasi;

#[derive(Clone)]
pub(crate) struct WorkerExecutorPerTestDependencies {
    redis: Arc<dyn Redis + Send + Sync + 'static>,
    redis_monitor: Arc<dyn RedisMonitor + Send + Sync + 'static>,
    worker_executor: Arc<dyn WorkerExecutor + Send + Sync + 'static>,
    worker_service: Arc<dyn WorkerService + Send + Sync + 'static>,
    template_service: Arc<dyn TemplateService + Send + Sync + 'static>,
    template_directory: PathBuf,
}

impl TestDependencies for WorkerExecutorPerTestDependencies {
    fn rdb(&self) -> Arc<dyn Rdb + Send + Sync + 'static> {
        panic!("Not supported")
    }

    fn redis(&self) -> Arc<dyn Redis + Send + Sync + 'static> {
        self.redis.clone()
    }

    fn redis_monitor(&self) -> Arc<dyn RedisMonitor + Send + Sync + 'static> {
        self.redis_monitor.clone()
    }

    fn shard_manager(&self) -> Arc<dyn ShardManager + Send + Sync + 'static> {
        panic!("Not supported")
    }

    fn template_directory(&self) -> PathBuf {
        self.template_directory.clone()
    }

    fn template_service(&self) -> Arc<dyn TemplateService + Send + Sync + 'static> {
        self.template_service.clone()
    }

    fn worker_service(&self) -> Arc<dyn WorkerService + Send + Sync + 'static> {
        self.worker_service.clone()
    }

    fn worker_executor_cluster(&self) -> Arc<dyn WorkerExecutorCluster + Send + Sync + 'static> {
        panic!("Not supported")
    }
}

struct WorkerExecutorTestDependencies {
    redis: Arc<dyn Redis + Send + Sync + 'static>,
    redis_monitor: Arc<dyn RedisMonitor + Send + Sync + 'static>,
    template_service: Arc<dyn TemplateService + Send + Sync + 'static>,
    template_directory: PathBuf,
}

impl WorkerExecutorTestDependencies {
    pub fn new() -> Self {
        let redis: Arc<dyn Redis + Send + Sync + 'static> = Arc::new(SpawnedRedis::new(
            6379,
            "".to_string(),
            Level::INFO,
            Level::ERROR,
        ));
        let redis_monitor: Arc<dyn RedisMonitor + Send + Sync + 'static> = Arc::new(
            SpawnedRedisMonitor::new(redis.clone(), Level::DEBUG, Level::ERROR),
        );
        let template_directory = Path::new("../test-templates").to_path_buf();
        let template_service: Arc<dyn TemplateService + Send + Sync + 'static> =
            Arc::new(FileSystemTemplateService::new(Path::new("data/templates")));
        Self {
            redis,
            redis_monitor,
            template_directory,
            template_service,
        }
    }

    pub fn per_test(
        &self,
        redis_prefix: &str,
        http_port: u16,
        grpc_port: u16,
    ) -> WorkerExecutorPerTestDependencies {
        // Connecting to the primary Redis but using a unique prefix
        let redis: Arc<dyn Redis + Send + Sync + 'static> = Arc::new(ProvidedRedis::new(
            self.redis.public_host().to_string(),
            self.redis.public_port(),
            redis_prefix.to_string(),
        ));
        // Connecting to the worker executor started in-process
        let worker_executor: Arc<dyn WorkerExecutor + Send + Sync + 'static> = Arc::new(
            ProvidedWorkerExecutor::new("localhost".to_string(), http_port, grpc_port),
        );
        // Fake worker service forwarding all requests to the worker executor directly
        let worker_service: Arc<dyn WorkerService + Send + Sync + 'static> = Arc::new(
            ForwardingWorkerService::new(worker_executor.clone(), self.template_service()),
        );
        WorkerExecutorPerTestDependencies {
            redis,
            redis_monitor: self.redis_monitor.clone(),
            worker_executor,
            worker_service,
            template_service: self.template_service().clone(),
            template_directory: self.template_directory.clone(),
        }
    }
}

impl TestDependencies for WorkerExecutorTestDependencies {
    fn rdb(&self) -> Arc<dyn Rdb + Send + Sync + 'static> {
        panic!("Not supported")
    }

    fn redis(&self) -> Arc<dyn Redis + Send + Sync + 'static> {
        self.redis.clone()
    }

    fn redis_monitor(&self) -> Arc<dyn RedisMonitor + Send + Sync + 'static> {
        self.redis_monitor.clone()
    }

    fn shard_manager(&self) -> Arc<dyn ShardManager + Send + Sync + 'static> {
        panic!("Not supported")
    }

    fn template_directory(&self) -> PathBuf {
        self.template_directory.clone()
    }

    fn template_service(&self) -> Arc<dyn TemplateService + Send + Sync + 'static> {
        self.template_service.clone()
    }

    fn worker_service(&self) -> Arc<dyn WorkerService + Send + Sync + 'static> {
        panic!("Not supported")
    }

    fn worker_executor_cluster(&self) -> Arc<dyn WorkerExecutorCluster + Send + Sync + 'static> {
        panic!("Not supported")
    }
}

#[ctor]
pub static BASE_DEPS: WorkerExecutorTestDependencies = WorkerExecutorTestDependencies::new();

#[dtor]
unsafe fn drop_base_deps() {
    let base_deps_ptr = BASE_DEPS.deref() as *const WorkerExecutorTestDependencies;
    let base_deps_ptr = base_deps_ptr as *mut WorkerExecutorTestDependencies;
    (*base_deps_ptr).redis().kill();
    (*base_deps_ptr).redis_monitor().kill();
}

struct Tracing;

impl Tracing {
    pub fn init() -> Self {
        // let console_layer = console_subscriber::spawn().with_filter(
        //     EnvFilter::try_new("trace").unwrap()
        //);
        let ansi_layer = tracing_subscriber::fmt::layer()
            .with_ansi(true)
            .with_filter(
                EnvFilter::try_new("debug,cranelift_codegen=warn,wasmtime_cranelift=warn,wasmtime_jit=warn,h2=warn,hyper=warn,tower=warn,fred=warn").unwrap()
            );

        tracing_subscriber::registry()
            // .with(console_layer) // Uncomment this to use tokio-console. Also needs RUSTFLAGS="--cfg tokio_unstable"
            .with(ansi_layer)
            .init();

        Self
    }
}

#[ctor]
pub static TRACING: Tracing = Tracing::init();
