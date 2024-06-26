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

use std::io::Read;

use async_trait::async_trait;
use golem_client::model::Component;

use tokio::fs::File;
use tracing::info;

use crate::model::{ComponentId, ComponentName, GolemError, PathBufOrStdin};

#[async_trait]
pub trait ComponentClient {
    async fn get_metadata(
        &self,
        component_id: &ComponentId,
        version: u64,
    ) -> Result<Component, GolemError>;
    async fn get_latest_metadata(
        &self,
        component_id: &ComponentId,
    ) -> Result<Component, GolemError>;
    async fn find(&self, name: Option<ComponentName>) -> Result<Vec<Component>, GolemError>;
    async fn add(&self, name: ComponentName, file: PathBufOrStdin)
        -> Result<Component, GolemError>;
    async fn update(&self, id: ComponentId, file: PathBufOrStdin) -> Result<Component, GolemError>;
}

#[derive(Clone)]
pub struct ComponentClientLive<C: golem_client::api::ComponentClient + Sync + Send> {
    pub client: C,
}

#[async_trait]
impl<C: golem_client::api::ComponentClient + Sync + Send> ComponentClient
    for ComponentClientLive<C>
{
    async fn get_metadata(
        &self,
        component_id: &ComponentId,
        version: u64,
    ) -> Result<Component, GolemError> {
        info!("Getting component version");

        Ok(self
            .client
            .get_component_metadata(&component_id.0, &version.to_string())
            .await?)
    }

    async fn get_latest_metadata(
        &self,
        component_id: &ComponentId,
    ) -> Result<Component, GolemError> {
        info!("Getting latest component version");

        Ok(self
            .client
            .get_latest_component_metadata(&component_id.0)
            .await?)
    }

    async fn find(&self, name: Option<ComponentName>) -> Result<Vec<Component>, GolemError> {
        info!("Getting components");

        let name = name.map(|n| n.0);

        Ok(self.client.get_components(name.as_deref()).await?)
    }

    async fn add(
        &self,
        name: ComponentName,
        path: PathBufOrStdin,
    ) -> Result<Component, GolemError> {
        info!("Adding component {name:?} from {path:?}");

        let component = match path {
            PathBufOrStdin::Path(path) => {
                let file = File::open(path)
                    .await
                    .map_err(|e| GolemError(format!("Can't open component file: {e}")))?;

                self.client.create_component(&name.0, file).await?
            }
            PathBufOrStdin::Stdin => {
                let mut bytes = Vec::new();

                let _ = std::io::stdin()
                    .read_to_end(&mut bytes) // TODO: steaming request from stdin
                    .map_err(|e| GolemError(format!("Failed to read stdin: {e:?}")))?;

                self.client.create_component(&name.0, bytes).await?
            }
        };

        Ok(component)
    }

    async fn update(&self, id: ComponentId, path: PathBufOrStdin) -> Result<Component, GolemError> {
        info!("Updating component {id:?} from {path:?}");

        let component = match path {
            PathBufOrStdin::Path(path) => {
                let file = File::open(path)
                    .await
                    .map_err(|e| GolemError(format!("Can't open component file: {e}")))?;

                self.client.update_component(&id.0, file).await?
            }
            PathBufOrStdin::Stdin => {
                let mut bytes = Vec::new();

                let _ = std::io::stdin()
                    .read_to_end(&mut bytes)
                    .map_err(|e| GolemError(format!("Failed to read stdin: {e:?}")))?;

                self.client.update_component(&id.0, bytes).await?
            }
        };

        Ok(component)
    }
}
