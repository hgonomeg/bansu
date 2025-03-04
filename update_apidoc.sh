#!/usr/bin/sh
 
curl http://localhost:8080/api-docs/openapi.json | jq > openapi/bansu.json
curl http://localhost:8080/api-docs/openapi.yaml > openapi/bansu.yaml
npx openapi-to-md openapi/bansu.json API_DOCUMENTATION.md
# npx @redocly/cli build-docs openapi/bansu.json -o docs/redoc-static.html

