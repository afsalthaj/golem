use std::collections::HashMap;
use std::result::Result;
use std::sync::Arc;

use golem_common::model::TemplateId;
use poem_openapi::param::Query;
use poem_openapi::payload::Json;
use poem_openapi::*;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::api::common::{ApiEndpointError, ApiTags};
use crate::api_definition;
use crate::api_definition::{ApiDefinitionId, MethodPattern, Version};
use crate::expr::Expr;
use crate::register::RegisterApiDefinition;

pub struct ApiDefinitionEndpoints {
    pub definition_service: Arc<dyn RegisterApiDefinition + Sync + Send>,
}

#[OpenApi(prefix_path = "/v1/api/definitions", tag = ApiTags::ApiDefinition)]
impl ApiDefinitionEndpoints {
    pub fn new(definition_service: Arc<dyn RegisterApiDefinition + Sync + Send>) -> Self {
        Self { definition_service }
    }

    #[oai(path = "/", method = "put")]
    async fn create_or_update(
        &self,
        payload: Json<ApiDefinition>,
    ) -> Result<Json<ApiDefinition>, ApiEndpointError> {
        let api_definition_id = &payload.id;

        info!("Save API definition - id: {}", api_definition_id);

        let definition: api_definition::ApiDefinition = payload
            .0
            .clone()
            .try_into()
            .map_err(ApiEndpointError::bad_request)?;

        self.definition_service
            .register(&definition)
            .await
            .map_err(|e| {
                error!(
                    "API definition id: {} - register error: {}",
                    api_definition_id, e
                );
                ApiEndpointError::internal(e)
            })?;

        let data = self
            .definition_service
            .get(api_definition_id)
            .await
            .map_err(ApiEndpointError::internal)?;

        let definition = data.ok_or(ApiEndpointError::not_found("API Definition not found"))?;

        let definition: ApiDefinition =
            definition.try_into().map_err(ApiEndpointError::internal)?;

        Ok(Json(definition))
    }

    #[oai(path = "/", method = "get")]
    async fn get(
        &self,
        #[oai(name = "api-definition-id")] api_definition_id_query: Query<Option<ApiDefinitionId>>,
    ) -> Result<Json<Vec<ApiDefinition>>, ApiEndpointError> {
        let api_definition_id_optional = api_definition_id_query.0;

        if let Some(api_definition_id) = api_definition_id_optional {
            info!("Get API definition - id: {}", api_definition_id);

            let data = self
                .definition_service
                .get(&api_definition_id)
                .await
                .map_err(ApiEndpointError::internal)?;

            let values: Vec<ApiDefinition> = match data {
                Some(d) => {
                    let definition: ApiDefinition =
                        d.try_into().map_err(ApiEndpointError::internal)?;
                    vec![definition]
                }
                None => vec![],
            };

            Ok(Json(values))
        } else {
            info!("Get all API definitions");

            let data = self
                .definition_service
                .get_all()
                .await
                .map_err(ApiEndpointError::internal)?;

            let mut values: Vec<ApiDefinition> = vec![];

            for d in data {
                let definition: ApiDefinition = d.try_into().map_err(ApiEndpointError::internal)?;
                values.push(definition);
            }

            Ok(Json(values))
        }
    }

    #[oai(path = "/", method = "delete")]
    async fn delete(
        &self,
        #[oai(name = "api-definition-id")] api_definition_id_query: Query<ApiDefinitionId>,
    ) -> Result<Json<String>, ApiEndpointError> {
        let api_definition_id = api_definition_id_query.0;

        info!("Delete API definition - id: {}", api_definition_id);

        let data = self
            .definition_service
            .get(&api_definition_id)
            .await
            .map_err(ApiEndpointError::internal)?;

        if data.is_some() {
            self.definition_service
                .delete(&api_definition_id)
                .await
                .map_err(ApiEndpointError::internal)?;

            return Ok(Json("API definition deleted".to_string()));
        }

        Err(ApiEndpointError::not_found("API definition not found"))
    }
}

// Mostly this data structures that represents the actual incoming request
// exist due to the presence of complicated Expr data type in api_definition::ApiDefinition.
// Consider them to be otherwise same
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Object)]
#[serde(rename_all = "camelCase")]
#[oai(rename_all = "camelCase")]
struct ApiDefinition {
    pub id: ApiDefinitionId,
    pub version: Version,
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Object)]
struct Route {
    pub method: MethodPattern,
    pub path: String,
    pub binding: GolemWorkerBinding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Object)]
#[serde(rename_all = "camelCase")]
#[oai(rename_all = "camelCase")]
struct GolemWorkerBinding {
    pub template: TemplateId,
    pub worker_id: serde_json::value::Value,
    pub function_name: String,
    pub function_params: Vec<serde_json::value::Value>,
    pub response: Option<ResponseMapping>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Object)]
struct ResponseMapping {
    pub body: serde_json::value::Value,
    // ${function.return}
    pub status: serde_json::value::Value,
    // "200" or if ${response.body.id == 1} "200" else "400"
    pub headers: HashMap<String, serde_json::value::Value>,
}

impl TryFrom<api_definition::ApiDefinition> for ApiDefinition {
    type Error = String;

    fn try_from(value: api_definition::ApiDefinition) -> Result<Self, Self::Error> {
        let mut routes = Vec::new();
        for route in value.routes {
            let v = Route::try_from(route)?;
            routes.push(v);
        }

        Ok(Self {
            id: value.id,
            version: value.version,
            routes,
        })
    }
}

impl TryInto<api_definition::ApiDefinition> for ApiDefinition {
    type Error = String;

    fn try_into(self) -> Result<api_definition::ApiDefinition, Self::Error> {
        let mut routes = Vec::new();

        for route in self.routes {
            let v = route.try_into()?;
            routes.push(v);
        }

        Ok(api_definition::ApiDefinition {
            id: self.id,
            version: self.version,
            routes,
        })
    }
}

impl TryFrom<api_definition::Route> for Route {
    type Error = String;

    fn try_from(value: api_definition::Route) -> Result<Self, Self::Error> {
        let path = value.path.to_string();
        let binding = GolemWorkerBinding::try_from(value.binding)?;

        Ok(Self {
            method: value.method,
            path,
            binding,
        })
    }
}

impl TryInto<api_definition::Route> for Route {
    type Error = String;

    fn try_into(self) -> Result<api_definition::Route, Self::Error> {
        let path =
            api_definition::PathPattern::from(self.path.as_str()).map_err(|e| e.to_string())?;
        let binding = self.binding.try_into()?;

        Ok(api_definition::Route {
            method: self.method,
            path,
            binding,
        })
    }
}

impl TryFrom<api_definition::ResponseMapping> for ResponseMapping {
    type Error = String;

    fn try_from(value: api_definition::ResponseMapping) -> Result<Self, Self::Error> {
        let body = serde_json::to_value(value.body).map_err(|e| e.to_string())?;
        let status = serde_json::to_value(value.status).map_err(|e| e.to_string())?;
        let mut headers = HashMap::new();
        for (key, value) in value.headers {
            let v = serde_json::to_value(value).map_err(|e| e.to_string())?;
            headers.insert(key.to_string(), v);
        }
        Ok(Self {
            body,
            status,
            headers,
        })
    }
}

impl TryInto<api_definition::ResponseMapping> for ResponseMapping {
    type Error = String;

    fn try_into(self) -> Result<api_definition::ResponseMapping, Self::Error> {
        let body: Expr = serde_json::from_value(self.body).map_err(|e| e.to_string())?;
        let status: Expr = serde_json::from_value(self.status).map_err(|e| e.to_string())?;
        let mut headers = HashMap::new();
        for (key, value) in self.headers {
            let v: Expr = serde_json::from_value(value).map_err(|e| e.to_string())?;
            headers.insert(key.to_string(), v);
        }

        Ok(api_definition::ResponseMapping {
            body,
            status,
            headers,
        })
    }
}

impl TryFrom<api_definition::GolemWorkerBinding> for GolemWorkerBinding {
    type Error = String;

    fn try_from(value: api_definition::GolemWorkerBinding) -> Result<Self, Self::Error> {
        let response: Option<ResponseMapping> = match value.response {
            Some(v) => {
                let r = ResponseMapping::try_from(v)?;
                Some(r)
            }
            None => None,
        };
        let worker_id = serde_json::to_value(value.worker_id).map_err(|e| e.to_string())?;
        let mut function_params = Vec::new();
        for param in value.function_params {
            let v = serde_json::to_value(param).map_err(|e| e.to_string())?;
            function_params.push(v);
        }

        Ok(Self {
            template: value.template,
            worker_id,
            function_name: value.function_name,
            function_params,
            response,
        })
    }
}

impl TryInto<api_definition::GolemWorkerBinding> for GolemWorkerBinding {
    type Error = String;

    fn try_into(self) -> Result<api_definition::GolemWorkerBinding, Self::Error> {
        let response: Option<api_definition::ResponseMapping> = match self.response {
            Some(v) => {
                let r: api_definition::ResponseMapping = v.try_into()?;
                Some(r)
            }
            None => None,
        };

        let worker_id: Expr = serde_json::from_value(self.worker_id).map_err(|e| e.to_string())?;
        let mut function_params = Vec::new();

        for param in self.function_params {
            let v: Expr = serde_json::from_value(param).map_err(|e| e.to_string())?;
            function_params.push(v);
        }

        Ok(api_definition::GolemWorkerBinding {
            template: self.template,
            worker_id,
            function_name: self.function_name,
            function_params,
            response,
        })
    }
}