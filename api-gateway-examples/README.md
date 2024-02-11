## Integrating Golem with existing API Gateways (Document in progress)

Once we are able to deploy our Golem service, we can integrate it with existing API Gateways. This is a common use case for Golem, as it allows us to leverage the existing infrastructure and security features of the API Gateway, while still being able to use Golem for the actual processing of the requests.

In order for this to work, we introduce a worker-bridge, that act as a bridge between your preferred API Gateway and the actual
worker instance that's running in Golem. 

We define the endpoint definitions and send it to worker bridge. This definition is merely the set of endpoints,
and the actual function that needs to be executed by the particular worker instance to serve the endpoint


Registration of this endpoint definition is pretty simple. The details of how much you can configure can be discussed later.
Currently, we are just focusing on the basic registration of the endpoint definition.

## Integration with Tyk API Gateway

Once you register this Endpoint definitions that relates to a specific worker and function, you can now use API Gateway
to forward request to the worker bridge. Let's say we choose Tyk as the API Gateway. A typical API definition required by Tyk is

Below given are the step by step instructions to follow to try all of this in Local.

### Step 1: Spin up Golem

```bash
// Choose any docker-comppose file in api-gateway-examples folder
docker-compose up
#golem-serviec, workerbride, worker-executor, shard-manager, redis, sqlite
```

### Step 2: Deploy shopping cart example

```bash
# Note down the template id, say "c467b83d-cb27-4296-b48a-ee85114cdb7"
golem-cli template add --template-name mytemplate test-templates/shopping-cart.wasm

# Note down the worker-name, here it is myworker
golem-cli worker invoke-and-await  --template-name mytemplate --worker-name myworker --function golem:it/api/add-item --parameters '[{"product-id" : "hmm", "name" : "hmm" , "price" : 10, "quantity" : 2}]'
```

### Step 3: Register the endpoint definition

Please make sure to use the correct template-id based on the output from `template add` command.

```bash
{
  "id": "shopping-cart",
  "version": "0.0.1",
  "routes": [
    {
      "method": "Get",
      "path": "/getcartcontents",
      "binding": {
        "type": "wit-worker",
        "template": "c467b83d-cb27-4296-b48a-ee85114cdb71",
        "workerId": "myworker2",
        "functionName": "golem:it/api/get-cart-contents",
        "functionParams": []
      }
    }
  ]
}

```

```bash

# register with worker bridge
curl -X PUT http://localhost:9005/v1/api/definitions -H "Content-Type: application/json"  -d @endpoint_definition.json

```

Step 4: Install Tyk API gateway

```bash
git clone https://github.com/TykTechnologies/tyk-gateway-docker
cd tyk-gateway-docker
docker-compose up
```

Register the API definition with Tyk. We are OAS API Definition of Tyk. You can read more about it in Tyk documentation


```json
curl --location --request POST 'http://localhost:8080/tyk/apis' \
--header 'x-tyk-authorization: foo' \
--header 'Content-Type: text/plain' \
--data-raw \
'{
    "name": "API to showcase integration with Golem",
    "api_id": "shopping-cart-tyk",
    "org_id": "default",
    "definition": {
        "location": "header",
        "key": "version"
    },
    "use_keyless": true, 
     "cache_options": {
       "enable_cache": true,
       "cache_timeout": 1,
       "cache_all_safe_requests": true,
       "cache_response_codes": [200]
    },    
    "version_data": {
        "not_versioned": true,
        "versions": {
            "Default": {
                "name": "Default",
                "global_headers": {
                    "X-API-Definition-Id":"shopping-cart"
                }
            }
        }
    },
    "driver": "otto",
    "proxy": {
        "listen_path": "/v10",
        "target_url": "http://192.168.18.202:9006/",
        "strip_listen_path": true
    }
}'

```
Reload the gateway, otherwise the API is not deployed with Tyk yet, so this is an important step.

```bash
curl -H "x-tyk-authorization: foo" -s http://localhost:8080/tyk/reload/group

```


### Important aspects
* Anything with listen_path /v5 will be forwarded to the worker bridge.
* Tyk injects X-API-Definition-Id header to the request, which is the id of the endpoint definition that we registered with the worker bridge
* With docker set up, we have 2 different docker networks running. Therefore the IP of the worker-bridge is the IP address of the machine (and not localhost) http://192.168.18.101:9006/.
* The target URL is url of the worker bridge that is ready to serve your custom requests. 
* Worker bridge is already registered with the API definition ID shopping-cart. If the worker bridge is not registered with the correct API definition, it will return something like the following
* Caching is enabled in Tyk API Gateway, just to show case, we get these features for free. 

```
 API request definition id shfddfopping-cart not found%
```

With all this in place, you can now make requests to the API Gateway and see the worker bridge forwarding the requests to the actual worker instance.


## Let's try out
```bash


curl -X GET http://localhost:8080/v1/getcartcontents
 
[[{"name":"hmm","price":10.0,"product-id":"hmm","quantity":2}]]%```

```
## Generation of Open API spec

Currently the requests to the gateway is forwarded as it is to worker bridge. 
This can work in various places. However, in some cases, it is important to generate Open API spec for the endpoints that are registered with the worker bridge,
and upload it to the gateway so that these endpoints can be further configured for authentication , authorisation, rate limits,
caching etc using Gateway console individually. This is not a mandatory step, but can be super useful in some cases.


## API Definition in external API Gateway vs API definition in worker-bridge

Consider worker-bridge, to a significant extent, as a backend service for free, that can interact with Golem's worker instances and 
the functions. It is therefore easier to integrate worker-bridge to any popular API gateways
similar to integrating any backend service to any API Gateways (Tyk, AWS Gateway etc). 

As we know, usually backend service _may_ have their own API definition (most often, it is an OpenAPI Spec). 
Similarly, we _need_ an API definition (that has its own expressive language supports) 
for worker-bridge to work. This is the only way a user/developer can let worker-bridge know which worker instance and function to call for a given request.

## Why is separate definition document required in worker-bridge as well as Tyk ?
As mentioned above, if we consider worker-bridge as a backend service, this question can be answered easily.
Here is an excerpt from Tyk's documentation on a similar aspect:

> Crucially, the user’s API service remains unaware of the Tyk Gateway’s processing layer, responding to incoming requests as in direct client-to-service communication. It implements the API endpoints, resources and methods providing the service’s functionality. It can also have its own OpenAPI document to describe and document itself (which is essentially also another name for API definition)

A backend service can have its own API definition or documentation. And at the same time, the reverse proxy configurations
may have its own documentation. Therefore it is not by surprise, we also have a 2 layer documentation for services
backed by API Gateway. We are also working on emitting OpenAPI spec from worker-bridge API Definition, which can be used to configure API Gateways.


## How does worker bridge know which API definition to pick for a given endpoint?

*X-API-Definition-Id*

This is by making using of API-ID header in the request. Given that the worker bridge is aware of various API definitions, it can pick the right
API definition for a given request, if the request consist of the knowledge of API-ID as a header. 
It is the responsibility of whoever managing the API Gateway to make sure that the API-ID is configured to be present in the request.
This can be achieved by using Tyk's middleware injection or transformations, and this is the case with almost all API Gateways.


## What next?

### Document Generation aspects

* Users can use their Open API spec that can be imported to worker bridge, with an obvious interface to specify the worker name and function name.. Sometimes there may not be any transformations that everything else is mere defaults
* If they want (not mandatory) They can make use of the same Open API spec to upload to API Gateway if they want to configure per-endpoint. Example: We allow 10000 requests per second for Get cart contents, but 1000 for posting. Otherwise (my draft PR) all requests to API Gateway is forwarded as is to worker bridge

We can flip this thinking too

* Users write the worker bridge API definition that is even more powerful with respect to a backend service, especially with transformations using Expr language
* They can generate Open API spec (probably with some challenging part in details) from this, and if they want (not mandatory) can make use of it to upload to their preferred API gateways and achieve the same advantages mentioned above in the second point. Otherwise all requests to API Gateway is forwarded to worker bridge
Given we already achieved what we discussed on Wednesday in my draft PR, may be its not a bad idea to discuss/validate some of these points (
