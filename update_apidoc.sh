#!/usr/bin/sh
 
curl http://localhost:8080/api-docs/openapi.json | jq > openapi/bansu.json
curl http://localhost:8080/api-docs/openapi.yaml > openapi/bansu.yaml
# npx @redocly/cli build-docs  =() -o docs/redoc-static.htm

